//! Widget bodies written in the app (`pxui_body_register`). The engine's registry holds plain function pointers, so each kind has one
//! trampoline here (`tramps`): it fills the kind's C record (`pxui_types::ffi::Pxui*Config`) from the engine's config and calls the
//! app's `(fn, user)` pair that `Shared::bodies` keeps for the body id the theme names. The default path never comes here: a widget
//! with no registered body calls its built-in body directly. A call costs one load of the host pointer, two bounds-checked loads of
//! the pair, the record on the stack and one indirect call; nothing is allocated (a slot is a push on a reused stack).
use crate::state::{PxuiUi, Shared, checked, fail, fail_code, get, pair, to_abi_box};
use crate::{PXUI_ERR_ARGUMENT, PXUI_ERR_STATE, PXUI_ERR_STALE, PxuiBox, Str};
use core::ffi::c_void;
use pxui_core::{bodies::*, ui::Ui};
use pxui_types::{ffi::*, theme::{BodyId, BodyKind, Part, State}, Node};

type Body = extern "C" fn(*mut c_void, *mut c_void, *const c_void);

/// What `pxui_slot` runs: the caller's content, or a search field's editor (which answers whether the query is not empty).
#[derive(Clone, Copy)]
enum Slot { Content(*mut (dyn FnMut(&mut Ui) + 'static)), Editor(*mut (dyn FnMut(&mut Ui) -> bool + 'static)) }

/// The app's bodies and the slots of the body calls in progress, in `Shared`.
#[derive(Default)]
pub(crate) struct BodyState {
    /// `(fn, user)` of body id n + 1 of kind k at `table[k][n]`, filled by `pxui_body_register`
    table: Vec<Vec<(Body, *mut c_void)>>,
    /// the slots of the body calls in progress, innermost last, and the serial the next one takes: a slot is live while it is here
    slots: Vec<(u64, Slot)>,
    serial: u64,
    /// body calls in progress (`pxui_part` is refused outside one)
    depth: u32,
    /// the spans a dialog's, a tip's and a footer's records point at, taken while their body runs (a nested body finds them empty)
    strs: Vec<Str>,
    hints: Vec<PxuiHintDesc>,
}

fn shared(ui: &Ui) -> Option<&'static mut Shared> { unsafe { ui.body_host().cast::<Shared>().as_mut() } }

fn invoke(ui: &mut Ui, sh: &mut Shared, kind: BodyKind, id: BodyId, cfg: *const c_void) {
    let Some(&(body, user)) = sh.bodies.table.get(kind as usize).and_then(|k| k.get((id.0 as usize).wrapping_sub(1))) else { return };
    let mut handle = PxuiUi::borrowed(ui, sh);
    sh.bodies.depth += 1;
    body(user, (&mut handle as *mut PxuiUi).cast(), cfg);
    sh.bodies.depth -= 1;
}

fn push(sh: &mut Shared, slot: Slot) -> PxuiSlot {
    sh.bodies.serial += 1;
    sh.bodies.slots.push((sh.bodies.serial, slot));
    PxuiSlot { serial: sh.bodies.serial }
}
// the slot outlives the borrow it was made from only until the trampoline truncates the stack, before it returns
fn content(sh: &mut Shared, f: &mut dyn FnMut(&mut Ui)) -> PxuiSlot { push(sh, Slot::Content(unsafe { core::mem::transmute::<&mut dyn FnMut(&mut Ui), *mut (dyn FnMut(&mut Ui) + 'static)>(f) })) }
fn editor(sh: &mut Shared, f: &mut dyn FnMut(&mut Ui) -> bool) -> PxuiSlot { push(sh, Slot::Editor(unsafe { core::mem::transmute::<&mut dyn FnMut(&mut Ui) -> bool, *mut (dyn FnMut(&mut Ui) -> bool + 'static)>(f) })) }

fn st(s: State) -> PxuiStateDesc { PxuiStateDesc { hovered: s.hovered as u8, pressed: s.pressed as u8, focused: s.focused as u8, disabled: s.disabled as u8, checked: s.checked as u8, reserved: [0; 3] } }
fn bx(b: pxui_core::ui::UiBox) -> PxuiBox { to_abi_box(b) }
fn opt(s: Option<&str>) -> Str { s.map_or(Str::default(), pair) }
fn flag(b: Option<bool>) -> (u8, u8) { (b.is_some() as u8, b.unwrap_or(false) as u8) }

/// One trampoline: `$kind`'s engine body `(ui, config, extra..)` becomes a call of the app's body with the record `$cfg { .. }`;
/// `$sh` is the shared state, for the slots a field makes. The slots are dropped when the body returns.
macro_rules! tramp {
    ($kind:ident, |$ui:ident, $sh:ident, $c:ident: $ct:ty $(, $a:ident: $at:ty)*| $cfg:ident { $($f:ident: $v:expr),* $(,)? }) => {
        pub(super) fn $kind($ui: &mut Ui, $c: $ct $(, $a: $at)*) {
            let id = $ui.theme().body.$kind;
            let Some($sh) = shared($ui) else { return };
            let mark = $sh.bodies.slots.len();
            let cfg = $cfg { size: core::mem::size_of::<$cfg>() as u32, $($f: $v),* };
            invoke($ui, $sh, BodyKind::$kind, id, (&cfg as *const $cfg).cast());
            $sh.bodies.slots.truncate(mark);
        }
    };
}

/// A trampoline that needs a span of `Str` or hints for its record: taken from the shared scratch, given back after the call.
macro_rules! tramp_span {
    ($kind:ident, $scratch:ident, |$ui:ident, $sh:ident, $span:ident, $c:ident: $ct:ty $(, $a:ident: $at:ty)*| $fill:block, $cfg:ident { $($f:ident: $v:expr),* $(,)? }) => {
        pub(super) fn $kind($ui: &mut Ui, $c: $ct $(, $a: $at)*) {
            let id = $ui.theme().body.$kind;
            let Some($sh) = shared($ui) else { return };
            let mark = $sh.bodies.slots.len();
            let mut $span = core::mem::take(&mut $sh.bodies.$scratch);
            $span.clear();
            $fill
            let cfg = $cfg { size: core::mem::size_of::<$cfg>() as u32, $($f: $v),* };
            invoke($ui, $sh, BodyKind::$kind, id, (&cfg as *const $cfg).cast());
            $sh.bodies.slots.truncate(mark);
            $sh.bodies.$scratch = $span;
        }
    };
}

mod tramps {
    use super::*;
    use pxui_core::ui_text::ValueFieldOpts;
    use pxui_types::theme::States;

    tramp!(toggle, |ui, sh, c: &ToggleConfig, label: &str| PxuiToggleConfig { state: st(c.state), on: c.on as u8, row: bx(c.row), label: pair(label) });
    tramp!(slider, |ui, sh, c: &SliderConfig, label: &str| PxuiSliderConfig { state: st(c.state), fraction: c.fraction, track: bx(c.track), label: pair(label) });
    tramp!(progress, |ui, sh, c: &ProgressConfig, label: &str| PxuiProgressConfig { has_value: c.value.is_some() as u8, value: c.value.unwrap_or(0.0), row: bx(c.row), label: pair(label) });
    tramp!(button, |ui, sh, c: &ButtonConfig, states: &States, label: &str| PxuiButtonConfig {
        state: st(c.state), bare: c.bare as u8, icon: c.icon as u8, at_end: c.at_end as u8, line: bx(c.line), row: bx(c.row), states: states, label: pair(label)
    });
    tramp!(icon_button, |ui, sh, c: &IconButtonConfig, label: &str, icon: &str| PxuiIconButtonConfig { state: st(c.state), node: bx(c.node), label: pair(label), icon: pair(icon) });
    tramp!(checkbox, |ui, sh, c: &CheckConfig, label: &str| PxuiCheckConfig { state: st(c.state), on: c.on as u8, row: bx(c.row), label: pair(label) });
    tramp!(radio, |ui, sh, c: &CheckConfig, label: &str| PxuiCheckConfig { state: st(c.state), on: c.on as u8, row: bx(c.row), label: pair(label) });
    tramp!(spinner, |ui, sh, c: &SpinnerConfig, label: &str| PxuiSpinnerConfig { node: bx(c.node), label: pair(label) });
    tramp!(badge, |ui, sh, c: &BadgeConfig, label: &str, text: &str| PxuiBadgeConfig { node: bx(c.node), label: pair(label), text: pair(text) });
    tramp!(label, |ui, sh, c: &LabelConfig, text: &str| PxuiLabelConfig { first: c.first as u8, node: bx(c.node), text: pair(text) });
    tramp_span!(footer, hints, |ui, sh, span, c: &FooterConfig, right: Option<&str>, hints: &[(String, String)]| {
        span.extend(hints.iter().map(|(k, v)| PxuiHintDesc { size: core::mem::size_of::<PxuiHintDesc>() as u32, reserved: 0, key: pair(k), value: pair(v) }));
    }, PxuiFooterConfig { row: bx(c.row), has_right: right.is_some() as u8, right: opt(right), hints: Slice { ptr: span.as_ptr(), len: span.len() } });
    tramp!(grip, |ui, sh, c: &GripConfig| PxuiGripConfig { state: st(c.state), vertical: c.vertical as u8, floating: c.floating as u8, grip: bx(c.grip) });
    tramp!(panel, |ui, sh, c: &PanelConfig, label: &str, body: &mut dyn FnMut(&mut Ui)| PxuiPanelConfig { node: bx(c.node), label: pair(label), content: content(sh, body) });
    tramp!(window, |ui, sh, c: &PanelConfig, label: &str, body: &mut dyn FnMut(&mut Ui)| PxuiPanelConfig { node: bx(c.node), label: pair(label), content: content(sh, body) });
    tramp!(popup, |ui, sh, c: &PanelConfig, label: &str, body: &mut dyn FnMut(&mut Ui)| PxuiPanelConfig { node: bx(c.node), label: pair(label), content: content(sh, body) });
    tramp!(modal, |ui, sh, c: &PanelConfig, label: &str, body: &mut dyn FnMut(&mut Ui)| PxuiPanelConfig { node: bx(c.node), label: pair(label), content: content(sh, body) });
    // a dialog has no engine config: its title, buttons and content are the whole record
    tramp_span!(dialog, strs, |ui, sh, span, title: &str, buttons: &[&str], body: &mut dyn FnMut(&mut Ui)| {
        span.extend(buttons.iter().map(|b| pair(b)));
    }, PxuiDialogConfig { title: pair(title), buttons: Slice { ptr: span.as_ptr(), len: span.len() }, content: content(sh, body) });
    tramp!(inspector_section, |ui, sh, c: &SectionConfig, label: &str, body: &mut dyn FnMut(&mut Ui)| PxuiSectionConfig { open: c.open as u8, gap: c.gap, title: bx(c.title), label: pair(label), content: content(sh, body) });
    tramp!(inspector_row, |ui, sh, c: &InspectorRowConfig, label: &str| PxuiInspectorRowConfig {
        state: st(c.state), has_pin: flag(c.pin).0, pin: flag(c.pin).1, label_w: c.label_w, row: bx(c.row), label: pair(label)
    });
    tramp!(split, |ui, sh, c: &SplitConfig, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)| PxuiSplitConfig {
        axis: c.axis as u32, ratio: c.ratio, min_a: c.min_a, min_b: c.min_b, split: bx(c.split), first: content(sh, a), second: content(sh, b)
    });
    tramp!(scroll_view, |ui, sh, c: &ScrollConfig, body: &mut dyn FnMut(&mut Ui)| PxuiScrollConfig { node: bx(c.node), content: content(sh, body) });
    tramp!(toast, |ui, sh, c: &ToastConfig, text: &str| PxuiToastConfig { state: st(c.state), node: bx(c.node), text: pair(text) });
    tramp_span!(tip, strs, |ui, sh, span, c: &TipConfig, lines: &[String]| {
        span.extend(lines.iter().map(|l| pair(l)));
    }, PxuiTipConfig { width: c.width, height: c.height, tip: bx(c.tip), lines: Slice { ptr: span.as_ptr(), len: span.len() } });
    tramp!(tab, |ui, sh, c: &TabConfig, text: &str| PxuiTabConfig { state: st(c.state), index: c.index, node: bx(c.node), text: pair(text) });
    tramp!(segment, |ui, sh, c: &SegmentConfig, text: &str| PxuiSegmentConfig { state: st(c.state), index: c.index, selected: c.selected, group: c.group, node: bx(c.node), text: pair(text) });
    tramp!(dropdown, |ui, sh, c: &DropdownConfig, text: &str| PxuiDropdownConfig { state: st(c.state), open: c.open as u8, key: c.key, button: bx(c.button), text: pair(text) });
    tramp!(menu_bar, |ui, sh, c: &MenuItemConfig, text: &str| PxuiMenuItemConfig { state: st(c.state), index: c.index, node: bx(c.node), text: pair(text) });
    tramp!(menu_item, |ui, sh, c: &MenuItemConfig, text: &str| PxuiMenuItemConfig { state: st(c.state), index: c.index, node: bx(c.node), text: pair(text) });
    tramp!(context_menu, |ui, sh, c: &MenuRowConfig, text: &str, shortcut: Option<&str>| PxuiMenuRowConfig {
        state: st(c.state), selected: c.selected as u8, chevron: c.chevron as u8, danger: c.danger as u8, has_swatch: c.swatch.is_some() as u8, swatch: c.swatch.unwrap_or_default(),
        has_shortcut: shortcut.is_some() as u8, index: c.index, lead: c.lead, swatch_pad: c.swatch_pad, row: bx(c.row), text: pair(text), shortcut: opt(shortcut)
    });
    tramp!(list_view, |ui, sh, c: &RowConfig, body: &mut dyn FnMut(&mut Ui)| PxuiRowConfig { state: st(c.state), index: c.index, row: bx(c.row), content: content(sh, body) });
    tramp!(table_view, |ui, sh, c: &RowConfig, body: &mut dyn FnMut(&mut Ui)| PxuiRowConfig { state: st(c.state), index: c.index, row: bx(c.row), content: content(sh, body) });
    tramp!(tree, |ui, sh, c: &TreeRowConfig, body: &mut dyn FnMut(&mut Ui)| PxuiTreeRowConfig {
        state: st(c.state), has: c.has as u8, open: c.open as u8, index: c.index, node: c.node, depth: c.depth, row: bx(c.row), content: content(sh, body)
    });
    tramp!(table_head, |ui, sh, c: &TableHeadConfig, title: &str| PxuiTableHeadConfig {
        state: st(c.state), has_ascending: flag(c.ascending).0, ascending: flag(c.ascending).1, index: c.index, node: bx(c.node), title: pair(title)
    });
    tramp!(table, |ui, sh, c: &TableCellConfig, text: &str| PxuiTableCellConfig {
        header: c.header as u8, left: c.left, right: c.right, height: c.height, row: c.row, column: c.column, key: c.key, text: pair(text)
    });
    tramp!(picker, |ui, sh, c: &PickerRowConfig, label: &str, detail: &str, query: &str| PxuiPickerRowConfig {
        state: st(c.state), current: c.current as u8, doomed: c.doomed as u8, unavailable: c.unavailable as u8, marked: c.marked as u8, has_swatch: c.swatch.is_some() as u8,
        swatch: c.swatch.unwrap_or_default(), index: c.index, row: bx(c.row), label: pair(label), detail: pair(detail), query: pair(query)
    });
    tramp!(search_field, |ui, sh, c: &SearchFieldConfig, icon: &str, body: &mut dyn FnMut(&mut Ui) -> bool| PxuiSearchFieldConfig {
        state: st(c.state), key: c.key, well: bx(c.well), icon: pair(icon), editor: editor(sh, body)
    });
    tramp!(text_field, |ui, sh, c: &TextFieldConfig, label: &str, value: &str| PxuiTextFieldConfig { state: st(c.state), row: bx(c.row), label: pair(label), value: pair(value) });
    tramp!(line_field, |ui, sh, c: &LineFieldConfig, text: &str| PxuiLineFieldConfig { state: st(c.state), empty: c.empty as u8, field: bx(c.field), text: pair(text) });
    tramp!(value_field, |ui, sh, c: &ValueFieldConfig, opts: &ValueFieldOpts, text: &str| PxuiValueFieldConfig {
        state: st(c.state), invalid: c.invalid as u8, w: c.w, h: c.h, field: bx(c.field), opts: value_opts(opts), text: pair(text)
    });
    tramp!(picker_field, |ui, sh, c: &PickerFieldConfig, placeholder: &str| PxuiPickerFieldConfig {
        state: st(c.state), empty: c.empty as u8, slash: c.slash as u8, prefix: c.prefix, inset: c.inset, search: bx(c.search), placeholder: pair(placeholder)
    });
}

/// The caller's `ValueFieldOpts` as the descriptor `pxui_ui_value_field` takes (its strings borrowed; place, label and value unset).
fn value_opts(o: &pxui_core::ui_text::ValueFieldOpts) -> PxuiValueFieldDesc {
    let flags = [(PxuiValueFieldDesc::LEFT, o.left), (PxuiValueFieldDesc::EDIT, o.edit), (PxuiValueFieldDesc::BARE, o.bare), (PxuiValueFieldDesc::KEPT_DRAG, o.kept_drag),
        (PxuiValueFieldDesc::HAS_SIZE, o.size.is_some()), (PxuiValueFieldDesc::HAS_FRACTION, o.fraction.is_some()), (PxuiValueFieldDesc::HAS_TRACKING, o.tracking.is_some()),
        (PxuiValueFieldDesc::HAS_LINE, o.line.is_some()), (PxuiValueFieldDesc::HAS_LEAD, o.lead.is_some()), (PxuiValueFieldDesc::HAS_TRAIL, o.trail.is_some()),
        (PxuiValueFieldDesc::HAS_DISPLAY, o.display.is_some()), (PxuiValueFieldDesc::HAS_PLACEHOLDER, o.placeholder.is_some())]
        .iter().fold(0, |all, (bit, on)| all | if *on { *bit } else { 0 });
    PxuiValueFieldDesc {
        size: core::mem::size_of::<PxuiValueFieldDesc>() as u32, flags, font_size: o.size.unwrap_or(0), fraction: o.fraction.unwrap_or(0.0), tracking: o.tracking.unwrap_or(0.0),
        line: o.line.unwrap_or_default(), lead_color: o.lead.as_ref().map_or_else(Default::default, |l| l.1), trail_color: o.trail.as_ref().map_or_else(Default::default, |l| l.1), reserved: [0; 4],
        x: 0.0, y: 0.0, width: 0.0, height: 0.0, display: opt(o.display.as_deref()), placeholder: opt(o.placeholder.as_deref()),
        lead: opt(o.lead.as_ref().map(|l| l.0.as_str())), trail: opt(o.trail.as_ref().map(|l| l.0.as_str())), label: Str::default(), value: Str::default(),
    }
}

macro_rules! registrars {
    ($($kind:ident $register:ident $body:ident $config:ident),* $(,)?) => {
        /// Register kind n's trampoline under a name: the same list, in the same order, as `BodyKind`.
        static REGISTER: &[fn(&mut Ui, &str) -> BodyId] = &[$(|ui, name| ui.$register(name, tramps::$kind)),*];
    };
}
pxui_types::body_kinds!(registrars);

/// A body name a theme file can select: not `default`, no space and none of `=` or `#`.
fn name_ok(name: &str) -> bool { !name.is_empty() && name != "default" && !name.contains(|c: char| c.is_whitespace() || c.is_control() || c == '=' || c == '#') }

// Registers `body` (called as `body(user, ui, config)`, the config the record of `kind`, a `BodyKind` number) as the body of `kind`
// called `name`, for a theme's `KIND.body = NAME`; returns its id, or 0 with `pxui_last_error` set when it is refused (an unknown kind,
// a name that is empty, `default` or holds a space, `=` or `#`, a null function, or a call inside a frame). Registering a name again
// replaces its function and keeps its id. Register at start-up: `user` must outlive the `Ui`.
export!(pub unsafe fn pxui_body_register(ui: *mut PxuiUi, kind: u32, name: Str, body: PxuiBodyFn, user: *mut c_void) -> u16 {
    let Some(state) = (unsafe { get(ui) }) else { return 0 };
    let (Some(body), Some(name)) = (body, unsafe { checked(name) }) else { unsafe { fail(ui, "invalid body registration") }; return 0 };
    if state.shared().in_frame || !state.owner { unsafe { fail_code(ui, "bodies are registered outside a frame", PXUI_ERR_STATE) }; return 0 }
    let Some(register) = REGISTER.get(kind as usize) else { unsafe { fail(ui, "unknown body kind") }; return 0 };
    if !name_ok(name) { unsafe { fail(ui, "invalid body name") }; return 0 }
    let engine = unsafe { &mut *state.engine };
    let shared = state.shared;
    engine.set_body_host(shared.cast());
    let id = register(engine, name);
    let table = &mut state.shared().bodies.table;
    if table.is_empty() { table.resize_with(REGISTER.len(), Vec::new); }
    let (kind_table, at) = (&mut table[kind as usize], (id.0 as usize).wrapping_sub(1));
    if at == kind_table.len() { kind_table.push((body, user)) } else if let Some(entry) = kind_table.get_mut(at) { *entry = (body, user) } else { unsafe { fail(ui, "too many bodies of one kind") }; return 0 }
    id.0
});

// Builds a node as `part` of the widget whose body is running (the way `pxui_ui_node_keyed` does): behaviour finds it again by role.
// A part is a number of `Part`; refused outside a body call or for a number that names no part.
export!(pub unsafe fn pxui_part(ui: *mut PxuiUi, part: u32, node: *const Node, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if state.shared().bodies.depth == 0 { return unsafe { fail_code(ui, "a part is built inside a body", PXUI_ERR_STATE) } }
    if !Part::valid(part) { return unsafe { fail(ui, "invalid part number") } }
    let key = unsafe { (*state.engine).own_part_key(Part(part)) };
    unsafe { crate::pxui_ui_node_keyed(ui, key, node, out) }
});

// The box of `part` of the widget keyed `owner` as the last build left it: 0 and the box in `out`, 1 when it was not built.
export!(pub unsafe fn pxui_part_box(ui: *mut PxuiUi, owner: i64, part: u32, out: *mut PxuiBox) -> i32 {
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT };
    if out.is_null() || !Part::valid(part) { return unsafe { fail(ui, "invalid part query") } }
    match unsafe { (*state.engine).part_box(owner, Part(part)) } { Some(b) => { unsafe { *out = to_abi_box(b) }; 0 } None => 1 }
});

/// Runs a slot of the body call in progress (the caller's content, or a search field's editor, whose answer, 1 when the query is not
/// empty, goes to `out` when it is not null); a slot of a body call that has returned is refused as stale.
pub unsafe fn slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32 {
    crate::state::note_call();
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT };
    let Some(&(_, found)) = state.shared().bodies.slots.iter().rev().find(|(serial, _)| *serial == slot.serial) else { return unsafe { fail_code(ui, "stale slot", PXUI_ERR_STALE) } };
    let engine = unsafe { &mut *state.engine };
    let answer = match found { Slot::Content(f) => { unsafe { (*f)(engine) }; 0 } Slot::Editor(f) => (unsafe { (*f)(engine) }) as u8 };
    if !out.is_null() { unsafe { *out = answer } }
    0
}

export!(trusted pub unsafe fn pxui_slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32 { unsafe { crate::trusted::pxui_slot(ui, slot, out) } });
