use crate::{composite::{defer_callback_panic, deferred}, check, Ui};
#[cfg(not(feature = "static"))]
use crate::Strs;
use core::{ffi::c_void, mem::ManuallyDrop, panic::AssertUnwindSafe, ptr::NonNull};
use pxui_types::{ui::Target, Axis, Look, PxuiBox, Str, ffi::{PxuiCurveDesc, PxuiFontDesc, PxuiLookDesc, PxuiRingDesc, PxuiShadowDesc, Slice}};

#[cfg_attr(feature = "static", allow(unused_imports))]
pub(crate) use pxui_types::ffi::{PxuiFooterDesc as FooterDesc, PxuiGridListDesc as GridListDesc, PxuiHintDesc as HintDesc, PxuiInspectorRowDesc as InspectorRowDesc, PxuiInspectorSectionDesc as InspectorSectionDesc, PxuiInspectorTextDesc as InspectorTextDesc, PxuiInspectorValueDesc as InspectorValueDesc, PxuiKeyframeDesc as KeyframeDesc, PxuiMenuBarDesc as MenuBarDesc, PxuiMenuDesc as MenuDesc, PxuiMenuOut as MenuOut, PxuiSplitDesc as SplitDesc, PxuiTableDesc as TableDesc};

#[cfg(feature = "static")]
mod sys {
    pub use pxui_abi_link::{pxui_ui_animate_keys, pxui_ui_animate_keys_repeat, pxui_ui_menu_bar, pxui_ui_wants_frame};
}
#[cfg(not(feature = "static"))]
mod sys {
    use super::{FooterDesc, GridListDesc, InspectorRowDesc, InspectorSectionDesc, InspectorTextDesc, InspectorValueDesc, KeyframeDesc, MenuBarDesc, MenuOut, SplitDesc, TableDesc};
    pub use crate::sys::PxuiUi;
    use pxui_types::{PxuiBox, Str, ffi::Slice};
    #[link(name = "pxui")]
    unsafe extern "C" {
    pub fn pxui_ui_section_list(ui:*mut PxuiUi,desc:*const pxui_types::ffi::PxuiSectionListDesc,out:*mut PxuiBox)->i32;
    pub fn pxui_ui_section_list_measured(ui:*mut PxuiUi,desc:*const pxui_types::ffi::PxuiMeasuredSectionDesc,out:*mut PxuiBox)->i32;
    pub fn pxui_ui_grid_list(ui: *mut PxuiUi, desc: *const GridListDesc, out: *mut PxuiBox) -> i32;
    pub fn pxui_ui_split(ui: *mut PxuiUi, desc: *const SplitDesc, changed: *mut u8, ratio: *mut f32) -> i32;
    pub fn pxui_ui_menu_bar(ui: *mut PxuiUi, desc: *const MenuBarDesc, out: *mut MenuOut) -> i32;
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
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
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
    ///     let b = ui.node("n".into(), pxui::Node::DEFAULT);
    ///     let curve = Curve::Bezier { duration: 0.4, points: [0.0, 0.0, 1.0, 1.0] };
    ///     let keys = [Keyframe::opacity(0.0, 1.0).curve(curve), Keyframe::opacity(0.4, 0.5).curve(curve)];
    ///     ui.animate_keys(&b, &keys, &pxui::Keyframes::DEFAULT.repeat(pxui::Repeat::Count(2)));
    /// });
    /// ```
    pub fn pxui_ui_animate_keys_repeat(ui: *mut PxuiUi, box_: *const PxuiBox, keys: Slice<KeyframeDesc>, repeat: u32, autoreverse: u8, start_offset: f32, out: *mut u8) -> i32;
    pub fn pxui_ui_table(ui: *mut PxuiUi, desc: *const TableDesc, clicked_row: *mut i64) -> i32;
    }
}

#[inline]
fn as_str(value: &str) -> Str { crate::as_str(value) }

type Build = pxui_types::ffi::UiBuild;
type GridBuild = Option<extern "C" fn(*mut c_void, *mut c_void, usize)>;
type CellFn = Option<extern "C" fn(*mut c_void, i64, i64) -> Str>;

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
/// The cell closure of `table_str`: its text is the caller's own, so the library borrows it and nothing is copied.
struct CellStr<F> { f: F }
extern "C" fn call_cell_str<'a, F: FnMut(i64, i64) -> &'a str>(user: *mut c_void, row: i64, column: i64) -> Str {
    let state = unsafe { &mut *user.cast::<CellStr<F>>() };
    if deferred() { return Str::default(); }
    match std::panic::catch_unwind(AssertUnwindSafe(|| (state.f)(row, column))) {
        Ok(value) => as_str(value),
        Err(panic) => { defer_callback_panic(panic); Str::default() }
    }
}

struct CellText<F> {
    f: F,
    value: String,
}
impl<F: FnMut(i64, i64) -> String> CellText<F> {
    fn erased(&mut self) -> (CellFn, *mut c_void) { (Some(call_cell::<F>), (self as *mut Self).cast()) }
}
extern "C" fn call_cell<F: FnMut(i64, i64) -> String>(user: *mut c_void, row: i64, column: i64) -> Str {
    let state = unsafe { &mut *user.cast::<CellText<F>>() };
    if deferred() { return Str::default(); }
    match std::panic::catch_unwind(AssertUnwindSafe(|| (state.f)(row, column))) {
        Ok(value) => { state.value = value; Str { ptr: state.value.as_ptr(), len: state.value.len() } }
        Err(panic) => { defer_callback_panic(panic); Str::default() }
    }
}

fn encode_look(value: Look) -> PxuiLookDesc {
    let shadow = |s: pxui_types::Shadow| PxuiShadowDesc { color: s.color, offset: s.offset, blur: s.blur, spread: s.spread, inner: s.inner as u8, reserved: [0; 3] };
    PxuiLookDesc {
        fill: value.fill, gradient: value.gradient.get().unwrap_or_default(), has_gradient: value.gradient.is_some() as u8, reserved: [0; 3], angle: value.angle,
        border: value.border, border_w: value.border_w, sides: value.sides as u32, radius: value.radius,
        shadows: [shadow(value.shadows[0]), shadow(value.shadows[1])], text: value.text,
        font: PxuiFontDesc { family: value.font.family, weight: value.font.weight, size: value.font.size, tracking: value.font.tracking, caps: value.font.caps as u8, reserved: [0; 3] },
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

impl Ui {
    /// A scrolling lead-in, sticky heading and virtual fixed-height rows in one scroll region; row `i`'s box is keyed by the item id
    /// `key(i)`, so a row's state follows its item when the items reorder (React's row key).
    ///
    /// `build(ui, part)` builds the lead-in, the heading and each row in view. `reveal(&returned_box, row)` scrolls a row index into
    /// view (minimum scroll, sticky heading respected). Ids must be unique among the rows (a repeated id takes a `#dup` key).
    /// Allocates: never on a warm frame.
    /// Identity: `label` keys the list under its parent; row boxes are `Key::Id(key(i))` under the list's rows.
    /// Panics: when the library refuses the geometry (a row height that is not positive and finite, more than a million rows), or A key or identity label over 256 bytes panics (a programmer error).
    /// when `build` or `key` panics.
    ///
    /// ```
    /// use pxui::ui::{SectionList, SectionPart};
    /// let ids = [7i64, 3, 9];
    /// let section = SectionList { count: ids.len(), row_height: 20.0, leading_height: 0.0, header_height: 0.0, trailing_height: 0.0, overlay_scrollbar: false };
    /// pxui::doc::frames(1, &mut |ui| { ui.section_list("songs", &section, &|i| ids[i], &mut |ui, part| if let SectionPart::Row(i) = part { ui.label_fmt(format_args!("{}", ids[i]), pxui::Label::BODY); }); });
    /// ```
    #[track_caller]
    pub fn section_list(&mut self,label:&str,section:&pxui_types::ui::SectionList,key:&dyn Fn(usize)->i64,build:&mut dyn FnMut(&mut Ui,pxui_types::ui::SectionPart))->PxuiBox {
        self.keyed_section(label,section,false,key,&mut |ui,part,_| build(ui,part))
    }

    /// `section_list` with rows of their own measured heights (estimated at `row_height` until built); `build` also gets the part's
    /// logical content Y. Rows are keyed by `key(i)` as in `section_list`.
    ///
    /// Allocates: never on a warm frame (the height cache grows with the count).
    /// Identity: as `section_list`.
    /// Panics: as `section_list`. A key or identity label over 256 bytes panics (a programmer error).
    ///
    /// ```
    /// use pxui::ui::{SectionList, SectionPart};
    /// let section = SectionList { count: 3, row_height: 20.0, leading_height: 0.0, header_height: 0.0, trailing_height: 0.0, overlay_scrollbar: false };
    /// pxui::doc::frames(2, &mut |ui| { ui.section_list_measured("notes", &section, &|i| i as i64, &mut |ui, part, _y| if let SectionPart::Row(_) = part { ui.label("note", pxui::Label::BODY.wrap()); }); });
    /// ```
    #[track_caller]
    pub fn section_list_measured(&mut self,label:&str,section:&pxui_types::ui::SectionList,key:&dyn Fn(usize)->i64,build:&mut dyn FnMut(&mut Ui,pxui_types::ui::SectionPart,f32))->PxuiBox {
        self.keyed_section(label,section,true,key,build)
    }

    /// The one body of `section_list` and `section_list_measured`.
    #[track_caller]
    fn keyed_section(&mut self,label:&str,section:&pxui_types::ui::SectionList,measured:bool,key:&dyn Fn(usize)->i64,build:&mut dyn FnMut(&mut Ui,pxui_types::ui::SectionPart,f32))->PxuiBox {
        use pxui_types::ui::SectionPart;
        let part=|index| match index {usize::MAX=>SectionPart::Leading,index if index==usize::MAX-1=>SectionPart::Header,index=>SectionPart::Row(index)};
        let mut out=PxuiBox::default();
        #[cfg(feature="static")]
        {
            let mut invoke=|raw:*mut crate::sys::PxuiUi,index,y| { let mut ui=ManuallyDrop::new(unsafe { Ui(NonNull::new_unchecked(raw)) }); build(&mut ui,part(index),y) };
            check(self,unsafe { if measured {
                pxui_abi_link::trusted::section_list_measured(self.0.as_ptr(),label,section,Some(key),&mut invoke,&mut out)
            } else {
                pxui_abi_link::trusted::section_list(self.0.as_ptr(),label,section,Some(key),&mut |raw,index| invoke(raw,index,0.0),&mut out)
            } });
        }
        #[cfg(not(feature="static"))]
        {
            let mut invoke=|ui:&mut Ui,index,y| build(ui,part(index),y);
            let mut context=KeyedSection {key,build:&mut invoke};
            let user=(&mut context as *mut KeyedSection).cast();
            let (overlay_scrollbar,label,count)=(u8::from(section.overlay_scrollbar),as_str(label),section.count);
            let (row_height,leading_height,header_height,trailing_height)=(section.row_height,section.leading_height,section.header_height,section.trailing_height);
            check(self,unsafe { if measured {
                let desc=pxui_types::ffi::PxuiMeasuredSectionDesc {size:core::mem::size_of::<pxui_types::ffi::PxuiMeasuredSectionDesc>() as u32,overlay_scrollbar,reserved:[0;3],label,count,row_height,leading_height,header_height,trailing_height,build:Some(call_keyed_measured),user,key:Some(call_row_key)};
                sys::pxui_ui_section_list_measured(self.0.as_ptr(),&desc,&mut out)
            } else {
                let desc=pxui_types::ffi::PxuiSectionListDesc {size:core::mem::size_of::<pxui_types::ffi::PxuiSectionListDesc>() as u32,overlay_scrollbar,reserved:[0;3],label,count,row_height,leading_height,header_height,trailing_height,build:Some(call_keyed_grid),user,key:Some(call_row_key)};
                sys::pxui_ui_section_list(self.0.as_ptr(),&desc,&mut out)
            } });
        }
        out
    }

    #[inline]
    #[track_caller] pub fn grid_list(&mut self, label: &str, count: usize, cell_w: f32, cell_h: f32, build: &mut dyn FnMut(&mut Ui, usize)) -> PxuiBox {
        let mut out = PxuiBox::default();
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

    #[inline]
    #[track_caller] pub fn split(&mut self, label: &str, axis: Axis, ratio: f32, min_a: f32, min_b: f32, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> Option<f32> {
        if axis == Axis::Stack { panic!("pxui: a split is a row or a column") }
        #[cfg(feature = "static")]
        { let mut out = None; check(self, unsafe { pxui_abi_link::trusted::split(self.0.as_ptr(), label, axis, ratio, min_a, min_b, inner!(|ui: &mut Ui| a(ui)), inner!(|ui: &mut Ui| b(ui)), &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let (mut context_a, mut context_b) = (Callback { f: a }, Callback { f: b });
            let ((build_a, user_a), (build_b, user_b)) = (context_a.erased(), context_b.erased());
            let desc = SplitDesc { size: core::mem::size_of::<SplitDesc>() as u32, reserved: 0, label: as_str(label), axis: match axis { Axis::Row => 0, _ => 1 }, ratio, min_a, min_b, build_a, user_a, build_b, user_b };
            let (mut changed, mut out) = (0u8, 0.0f32);
            check(self, unsafe { sys::pxui_ui_split(self.0.as_ptr(), &desc, &mut changed, &mut out) });
            (changed != 0).then_some(out)
        }
    }

    #[inline]
    #[track_caller] pub fn menu_bar(&mut self, label: &str, menus: &[(&str, &[&str])]) -> Option<(usize, usize)> {
        // a bar of up to 8 menus and 64 items lives on the stack, a larger one on the heap
        let total: usize = menus.iter().map(|(_, items)| items.len()).sum();
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
        for ((title, items), desc) in menus.iter().zip(descs.iter_mut()) {
            for (out, item) in flat[at..at + items.len()].iter_mut().zip(*items) { *out = as_str(item); }
            *desc = MenuDesc { size: core::mem::size_of::<MenuDesc>() as u32, reserved: 0, title: as_str(title), items: Slice { ptr: flat[at..].as_ptr(), len: items.len() } };
            at += items.len();
        }
        let desc = MenuBarDesc { size: core::mem::size_of::<MenuBarDesc>() as u32, reserved: 0, label: as_str(label), menus: Slice { ptr: descs.as_ptr(), len: descs.len() } };
        let mut out = MenuOut { size: core::mem::size_of::<MenuOut>() as u32, reserved: 0, menu: usize::MAX, item: usize::MAX };
        check(self, unsafe { sys::pxui_ui_menu_bar(self.0.as_ptr(), &desc, &mut out) });
        (out.menu != usize::MAX && out.item != usize::MAX).then_some((out.menu, out.item))
    }

    #[inline]
    #[track_caller] pub fn inspector_row(&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (PxuiBox, f32, f32, f32) {
        #[cfg(feature = "static")]
        { let mut out = (PxuiBox::default(), 0.0, 0.0, 0.0); check(self, unsafe { pxui_abi_link::trusted::inspector_row(self.0.as_ptr(), width, pin, key, label, &mut out) }); out }
        #[cfg(not(feature = "static"))]
        {
            let mut desc = InspectorRowDesc { size: core::mem::size_of::<InspectorRowDesc>() as u32, has_width: width.is_some() as u8, has_pin: pin.is_some() as u8, pin: pin.unwrap_or(false) as u8, reserved: 0, key: as_str(key), label: as_str(label), width: width.unwrap_or_default(), reserved_tail: 0, row: PxuiBox::default(), control_x: 0.0, control_y: 0.0, control_w: 0.0 };
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
    ///     let b = ui.node("pulse".into(), pxui::Node::DEFAULT);
    ///     let spring = Curve::Spring { omega: 8.0 };
    ///     let keys = [Keyframe::opacity(0.0, 1.0).curve(spring), Keyframe::opacity(0.5, 0.4).curve(spring)];
    ///     ui.animate_keys(&b, &keys, &Keyframes::DEFAULT.repeat(Repeat::Forever).autoreverse());
    ///     ui.animate_keys(&b, &keys[..1], &Keyframes::DEFAULT);
    /// });
    /// ```
    #[track_caller]
    pub fn animate_keys(&mut self, box_: &PxuiBox, keys: &[Keyframe], play: &Keyframes) -> bool {
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

    #[inline]
    /// `table` over cells that borrow from the caller's data (valid for the whole call): nothing is copied or allocated per cell.
    #[track_caller]
    pub fn table_str<'a, F: FnMut(i64, i64) -> &'a str>(&mut self, at: (f32, f32), width: f32, height: f32, headers: &[&str], rows: i64, cell: F, label: &str) -> Option<i64> {
        #[cfg(feature = "static")]
        {
            let (mut cell, mut out) = (cell, None);
            check(self, unsafe { pxui_abi_link::trusted::table(self.0.as_ptr(), at, width, height, headers, rows, &mut |row: i64, column: i64, put: &mut dyn FnMut(&str)| put(cell(row, column)), label, &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        {
            let header_values = Strs::new(headers);
            let mut callback = CellStr { f: cell };
            let desc = TableDesc { size: core::mem::size_of::<TableDesc>() as u32, reserved: 0, label: as_str(label), at: [at.0, at.1], width, height, headers: header_values.span(), rows, cell: Some(call_cell_str::<F>), user: (&mut callback as *mut CellStr<F>).cast() };
            let mut out = -1;
            check(self, unsafe { sys::pxui_ui_table(self.0.as_ptr(), &desc, &mut out) });
            (out >= 0).then_some(out)
        }
    }

    #[track_caller]
    pub fn table<F: FnMut(i64, i64) -> String>(&mut self, at: (f32, f32), width: f32, height: f32, headers: &[&str], rows: i64, cell: F, label: &str) -> Option<i64> {
        #[cfg(feature = "static")]
        {
            let (mut cell, mut out) = (cell, None);
            check(self, unsafe { pxui_abi_link::trusted::table(self.0.as_ptr(), at, width, height, headers, rows, &mut |row: i64, column: i64, put: &mut dyn FnMut(&str)| put(&cell(row, column)), label, &mut out) });
            out
        }
        #[cfg(not(feature = "static"))]
        {
            let header_values = Strs::new(headers);
            let mut callback = CellText { f: cell, value: String::new() };
            let (cell, user) = callback.erased();
            let desc = TableDesc { size: core::mem::size_of::<TableDesc>() as u32, reserved: 0, label: as_str(label), at: [at.0, at.1], width, height, headers: header_values.span(), rows, cell, user };
            let mut out = -1;
            check(self, unsafe { sys::pxui_ui_table(self.0.as_ptr(), &desc, &mut out) });
            (out >= 0).then_some(out)
        }
    }
}
