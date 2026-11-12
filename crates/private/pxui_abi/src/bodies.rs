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
    /// the slots of the body calls in progress, innermost last, with the depth of the body call each was given to: a slot is live
    /// while it is here and runs only at that depth (not from a nested body, not inside its own content)
    slots: Vec<(u64, u32, Slot)>,
    /// body calls and running slots in progress (`pxui_part` is refused outside a body call)
    depth: u32,
    /// the spans a dialog's, a tip's and a footer's records point at: a pool, one taken while a body runs and given back after, so a body
    /// nested in another takes its own and nothing allocates once the pool has grown to the deepest nesting
    strs: Vec<Vec<Str>>,
    hints: Vec<Vec<PxuiHintDesc>>,
}

/// The shared state of the `Ui` the engine belongs to (null before a body was registered). A raw pointer, as `run_frame` keeps it: the
/// body the trampoline calls reaches the same state through its handle, so no `&mut Shared` is held across the call; each access below
/// is one short-lived place expression.
fn shared(ui: &Ui) -> *mut Shared { ui.body_host().cast() }

fn invoke(ui: &mut Ui, sh: *mut Shared, kind: BodyKind, id: BodyId, cfg: *const c_void) {
    let Some(&(body, user)) = (unsafe { &(*sh).bodies.table }).get(kind as usize).and_then(|k| k.get((id.0 as usize).wrapping_sub(1))) else { return };
    let mut handle = PxuiUi::borrowed(ui, sh);
    let _current = crate::state::Current::set(sh, &mut handle);
    let depth = unsafe { (*sh).bodies.depth };
    unsafe { (*sh).bodies.depth = depth + 1 };
    body(user, (&mut handle as *mut PxuiUi).cast(), cfg);
    // set back, not decremented: a slot whose content unwound (the static wrapper's) never lowered it
    unsafe { (*sh).bodies.depth = depth };
}

/// Slot serials, one count for every `Ui`: a slot of one `Ui` never names a live slot of another.
static SERIAL: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// A slot of the body call the trampoline is about to make (one deeper than now).
fn push(sh: *mut Shared, slot: Slot) -> PxuiSlot {
    let serial = SERIAL.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + 1;
    let bodies = unsafe { &mut (*sh).bodies };
    bodies.slots.push((serial, bodies.depth + 1, slot));
    PxuiSlot { serial }
}
// the slot outlives the borrow it was made from only until the trampoline truncates the stack, before it returns
fn content(sh: *mut Shared, f: &mut dyn FnMut(&mut Ui)) -> PxuiSlot { push(sh, Slot::Content(unsafe { core::mem::transmute::<&mut dyn FnMut(&mut Ui), *mut (dyn FnMut(&mut Ui) + 'static)>(f) })) }
fn editor(sh: *mut Shared, f: &mut dyn FnMut(&mut Ui) -> bool) -> PxuiSlot { push(sh, Slot::Editor(unsafe { core::mem::transmute::<&mut dyn FnMut(&mut Ui) -> bool, *mut (dyn FnMut(&mut Ui) -> bool + 'static)>(f) })) }

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
            let $sh = shared($ui);
            if $sh.is_null() { return }
            let mark = unsafe { (*$sh).bodies.slots.len() };
            let cfg = $cfg { size: core::mem::size_of::<$cfg>() as u32, $($f: $v),* };
            invoke($ui, $sh, BodyKind::$kind, id, (&cfg as *const $cfg).cast());
            unsafe { (*$sh).bodies.slots.truncate(mark) };
        }
    };
}

/// A trampoline that needs a span of `Str` or hints for its record: taken from the shared scratch pool, given back after the call.
macro_rules! tramp_span {
    ($kind:ident, $scratch:ident, |$ui:ident, $sh:ident, $span:ident, $c:ident: $ct:ty $(, $a:ident: $at:ty)*| $fill:block, $cfg:ident { $($f:ident: $v:expr),* $(,)? }) => {
        pub(super) fn $kind($ui: &mut Ui, $c: $ct $(, $a: $at)*) {
            let id = $ui.theme().body.$kind;
            let $sh = shared($ui);
            if $sh.is_null() { return }
            let mark = unsafe { (*$sh).bodies.slots.len() };
            let mut $span = unsafe { (*$sh).bodies.$scratch.pop() }.unwrap_or_default();
            $span.clear();
            $fill
            let cfg = $cfg { size: core::mem::size_of::<$cfg>() as u32, $($f: $v),* };
            invoke($ui, $sh, BodyKind::$kind, id, (&cfg as *const $cfg).cast());
            unsafe { (*$sh).bodies.slots.truncate(mark) };
            unsafe { (*$sh).bodies.$scratch.push($span) };
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
/// empty, goes to `out` when it is not null); a slot of a body call that has returned is refused as stale. It runs only in the body
/// call that received it: one deeper (a nested body, or the slot's own content, which runs a level down) is refused, so the closure
/// it lends is never entered while it runs.
pub unsafe fn slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32 {
    enter!(ui);
    let Some(state) = (unsafe { get(ui) }) else { return PXUI_ERR_ARGUMENT };
    let bodies = &mut state.shared().bodies;
    let Some(&(_, level, found)) = bodies.slots.iter().rev().find(|(serial, ..)| *serial == slot.serial) else { return unsafe { fail_code(ui, "stale slot", PXUI_ERR_STALE) } };
    if level != bodies.depth { return unsafe { fail_code(ui, "a slot runs in the body call that received it, not inside its own content or a nested body", PXUI_ERR_STATE) } }
    bodies.depth = level + 1;
    let engine = unsafe { &mut *state.engine };
    let answer = match found { Slot::Content(f) => { unsafe { (*f)(engine) }; 0 } Slot::Editor(f) => (unsafe { (*f)(engine) }) as u8 };
    state.shared().bodies.depth = level;
    if !out.is_null() { unsafe { *out = answer } }
    0
}

export!(trusted pub unsafe fn pxui_slot(ui: *mut PxuiUi, slot: PxuiSlot, out: *mut u8) -> i32 { unsafe { crate::trusted::pxui_slot(ui, slot, out) } });

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameIn, FrameOut, PXUI_ERR_ARGUMENT, PxuiUiDesc, pxui_frame, pxui_last_error, pxui_ui_create, pxui_ui_destroy, pxui_ui_set_theme};
    use core::cell::{Cell, RefCell};
    use pxui_types::ffi::{PxuiSizeDesc, ThemeSource};

    thread_local! {
        static TOGGLE: RefCell<Vec<(u32, String, u8, i32, i32, i32)>> = const { RefCell::new(Vec::new()) };
        static SLOT: Cell<PxuiSlot> = const { Cell::new(PxuiSlot { serial: 0 }) };
        static CONTENT: Cell<u32> = const { Cell::new(0) };
        static REGISTERED_IN_FRAME: Cell<u16> = const { Cell::new(99) };
    }

    fn text(s: &str) -> Str { Str { ptr: s.as_ptr(), len: s.len() } }
    fn error(ui: *mut PxuiUi) -> String { let e = unsafe { pxui_last_error(ui) }; String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(e.ptr, e.len) }).into_owned() }

    extern "C" fn toggle_body(_: *mut c_void, ui: *mut c_void, config: *const c_void) {
        let (ui, c) = (ui.cast::<PxuiUi>(), unsafe { &*config.cast::<PxuiToggleConfig>() });
        let label = String::from_utf8_lossy(unsafe { core::slice::from_raw_parts(c.label.ptr, c.label.len) }).into_owned();
        let (node, mut out) = (Node::DEFAULT, PxuiBox::default());
        let hit = unsafe { pxui_part(ui, Part::HIT.0, &node, &mut out) };
        let bad = unsafe { pxui_part(ui, 10, &node, &mut out) };
        // the owner's key is the row's: its part is found by it, one it never built is not
        let mut found = PxuiBox::default();
        let again = unsafe { pxui_part_box(ui, c.row.box_key, Part::HIT.0, &mut found) } + 10 * unsafe { pxui_part_box(ui, c.row.box_key, Part::TRACK.0, &mut found) };
        TOGGLE.with(|t| t.borrow_mut().push((c.size, label, c.on, hit, bad, again)));
        REGISTERED_IN_FRAME.with(|r| r.set(unsafe { pxui_body_register(ui, BodyKind::toggle as u32, text("late"), Some(toggle_body), core::ptr::null_mut()) }));
    }

    extern "C" fn toggle_frame(_: *mut c_void, ui: *mut c_void) {
        let mut out = 0u8;
        assert_eq!(unsafe { crate::pxui_toggle(ui.cast(), text("Wi-Fi"), 1, &mut out) }, 0);
    }

    fn frame(ui: *mut PxuiUi, build: extern "C" fn(*mut c_void, *mut c_void)) {
        let mut out = FrameOut::default();
        assert_eq!(unsafe { pxui_frame(ui, &FrameIn { width: 200.0, height: 100.0, ..FrameIn::default() }, Some(build), core::ptr::null_mut(), &mut out) }, 0);
    }

    fn theme(ui: *mut PxuiUi, source: &str) -> i32 {
        unsafe { pxui_ui_set_theme(ui, &ThemeSource { size: core::mem::size_of::<ThemeSource>() as u32, flags: 0, source: text(source) }) }
    }

    /// A C body gets the toggle's record (its size, label and value), builds `Part::HIT` through `pxui_part`, refuses a number that names no
    /// part, finds the part again through `pxui_part_box`, and cannot register a body from inside the frame; `pxui_part` outside a body is
    /// refused; an unknown name in a theme text, and every bad registration, are errors.
    #[test]
    fn a_c_body_gets_its_record_and_builds_a_part_and_bad_names_are_errors() {
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        let user = core::ptr::null_mut();
        let register = |kind: u32, name: &str, body: PxuiBodyFn| unsafe { pxui_body_register(ui, kind, text(name), body, user) };
        assert_eq!(register(BodyKind::toggle as u32, "probe", Some(toggle_body)), 1);
        assert_eq!(register(BodyKind::toggle as u32, "probe", Some(toggle_body)), 1, "a name again keeps its id");
        assert_eq!(register(BodyKind::toggle as u32, "other", Some(toggle_body)), 2);
        for (kind, name, body) in [(BodyKind::toggle as u32, "default", Some(toggle_body as _)), (BodyKind::toggle as u32, "", Some(toggle_body as _)), (BodyKind::toggle as u32, "a b", Some(toggle_body as _)),
            (BodyKind::toggle as u32, "a=b", Some(toggle_body as _)), (BodyKind::toggle as u32, "nobody", None), (40, "probe", Some(toggle_body as _)), (u32::MAX, "probe", Some(toggle_body as _))] {
            assert_eq!(register(kind, name, body), 0, "kind {kind} name {name:?} should be refused");
            assert!(!error(ui).is_empty());
        }
        // a theme text that names a body nobody registered is refused, with the line
        assert_eq!(theme(ui, "toggle.body = nope"), PXUI_ERR_ARGUMENT);
        assert!(error(ui).contains("nope"), "{}", error(ui));
        assert_eq!(theme(ui, "slider.body = probe"), PXUI_ERR_ARGUMENT, "a body of another kind is not this kind's");
        assert_eq!(theme(ui, "toggle.body = probe"), 0);
        frame(ui, toggle_frame);
        let seen = TOGGLE.with(|t| t.borrow().clone());
        assert_eq!(seen, vec![(core::mem::size_of::<PxuiToggleConfig>() as u32, "Wi-Fi".to_owned(), 1, 0, PXUI_ERR_ARGUMENT, 10)], "the body call: its record, HIT built and found, 10 (no part) refused, TRACK never built");
        assert_eq!(REGISTERED_IN_FRAME.with(Cell::get), 0, "a body is not registered inside a frame");
        frame(ui, toggle_frame);
        assert_eq!(TOGGLE.with(|t| t.borrow().last().map(|s| s.5)), Some(10), "the second frame finds the part it built");
        let (node, mut out) = (Node::DEFAULT, PxuiBox::default());
        assert_eq!(unsafe { pxui_part(ui, Part::HIT.0, &node, &mut out) }, PXUI_ERR_STATE, "a part outside a body");
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    extern "C" fn scroll_body(_: *mut c_void, ui: *mut c_void, config: *const c_void) {
        let c = unsafe { &*config.cast::<PxuiScrollConfig>() };
        SLOT.with(|s| s.set(c.content));
        let mut answer = 7u8;
        assert_eq!(unsafe { pxui_slot(ui.cast(), c.content, &mut answer) }, 0);
        assert_eq!(answer, 0, "a content slot answers 0");
        // a slot that was never made is stale
        assert_eq!(unsafe { pxui_slot(ui.cast(), PxuiSlot { serial: 0xdead_beef }, core::ptr::null_mut()) }, PXUI_ERR_STALE);
    }

    extern "C" fn count_content(_: *mut c_void, _: *mut c_void) { CONTENT.with(|c| c.set(c.get() + 1)); }

    extern "C" fn scroll_frame(_: *mut c_void, ui: *mut c_void) {
        let mut out = PxuiBox::default();
        let grow = PxuiSizeDesc { kind: 3, value: 0.0, relative: None };
        assert_eq!(unsafe { crate::pxui_ui_scroll_view(ui.cast(), text("list"), grow, grow, Some(count_content), core::ptr::null_mut(), &mut out) }, 0);
    }

    /// A body runs the caller's content once through its slot; the slot is stale once the body call has returned.
    #[test]
    fn a_slot_runs_the_callers_content_and_is_stale_after_the_body_returns() {
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        assert_eq!(unsafe { pxui_body_register(ui, BodyKind::scroll_view as u32, text("probe"), Some(scroll_body), core::ptr::null_mut()) }, 1);
        assert_eq!(theme(ui, "scroll_view.body = probe"), 0);
        frame(ui, scroll_frame);
        assert_eq!(CONTENT.with(Cell::get), 1, "the content ran once, through the slot");
        let mut out = 0u8;
        assert_eq!(unsafe { pxui_slot(ui, SLOT.with(Cell::get), &mut out) }, PXUI_ERR_STALE);
        assert!(error(ui).contains("stale slot"));
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    thread_local! {
        static OWN: Cell<PxuiSlot> = const { Cell::new(PxuiSlot { serial: 0 }) };
        static RUNS: Cell<u32> = const { Cell::new(0) };
        static AGAIN: Cell<i32> = const { Cell::new(99) };
    }

    extern "C" fn own_slot_body(_: *mut c_void, ui: *mut c_void, config: *const c_void) {
        let c = unsafe { &*config.cast::<PxuiScrollConfig>() };
        OWN.with(|s| s.set(c.content));
        assert_eq!(unsafe { pxui_slot(ui.cast(), c.content, core::ptr::null_mut()) }, 0);
    }

    // the caller's content runs its own slot again: it would enter the closure that is running it
    extern "C" fn reentering_content(_: *mut c_void, ui: *mut c_void) {
        RUNS.with(|r| r.set(r.get() + 1));
        if RUNS.with(Cell::get) < 3 { AGAIN.with(|a| a.set(unsafe { pxui_slot(ui.cast(), OWN.with(Cell::get), core::ptr::null_mut()) })); }
    }

    extern "C" fn reentering_frame(_: *mut c_void, ui: *mut c_void) {
        let (mut out, grow) = (PxuiBox::default(), PxuiSizeDesc { kind: 3, value: 0.0, relative: None });
        assert_eq!(unsafe { crate::pxui_ui_scroll_view(ui.cast(), text("list"), grow, grow, Some(reentering_content), core::ptr::null_mut(), &mut out) }, 0);
    }

    /// A slot is never run inside its own content (the closure it lends is running): refused, and the content ran once.
    #[test]
    fn a_slot_is_refused_inside_its_own_content() {
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        assert_eq!(unsafe { pxui_body_register(ui, BodyKind::scroll_view as u32, text("own"), Some(own_slot_body), core::ptr::null_mut()) }, 1);
        assert_eq!(theme(ui, "scroll_view.body = own"), 0);
        frame(ui, reentering_frame);
        assert_eq!((RUNS.with(Cell::get), AGAIN.with(Cell::get)), (1, PXUI_ERR_STATE), "{}", error(ui));
        // the refusal left the depth as it was: the next frame's body runs its slot again
        frame(ui, reentering_frame);
        assert_eq!(RUNS.with(Cell::get), 2);
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    thread_local! {
        static FIRST: Cell<PxuiSlot> = const { Cell::new(PxuiSlot { serial: 0 }) };
        static FOREIGN: Cell<i32> = const { Cell::new(99) };
    }

    // on the first Ui it keeps its slot; on the second it runs the first Ui's
    extern "C" fn foreign_body(_: *mut c_void, ui: *mut c_void, config: *const c_void) {
        let c = unsafe { &*config.cast::<PxuiScrollConfig>() };
        match FIRST.with(Cell::get) {
            PxuiSlot { serial: 0 } => FIRST.with(|f| f.set(c.content)),
            first => FOREIGN.with(|f| f.set(unsafe { pxui_slot(ui.cast(), first, core::ptr::null_mut()) })),
        }
    }

    /// A slot of one `Ui` names nothing in another, however many slots each has made.
    #[test]
    fn a_slot_of_another_ui_is_stale() {
        let (a, b) = unsafe { (pxui_ui_create(&PxuiUiDesc::default()), pxui_ui_create(&PxuiUiDesc::default())) };
        for ui in [a, b] {
            assert_eq!(unsafe { pxui_body_register(ui, BodyKind::scroll_view as u32, text("foreign"), Some(foreign_body), core::ptr::null_mut()) }, 1);
            assert_eq!(theme(ui, "scroll_view.body = foreign"), 0);
            frame(ui, scroll_frame);
        }
        assert_eq!(FOREIGN.with(Cell::get), PXUI_ERR_STALE);
        assert_eq!(unsafe { pxui_ui_destroy(a) } | unsafe { pxui_ui_destroy(b) }, 0);
    }

    /// A kind takes at most 65,535 names (ids are `u16`, 0 the built-in body): one more is refused instead of wrapping onto body 1,
    /// whose function and user pointer then stay its own.
    #[test]
    fn a_kind_refuses_a_name_past_the_last_id() {
        extern "C" fn other(_: *mut c_void, _: *mut c_void, _: *const c_void) {}
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        let kind = BodyKind::label as u32;
        let mut name = String::new();
        for id in 1..=u16::MAX {
            name.clear();
            name.push_str(&id.to_string());
            assert_eq!(unsafe { pxui_body_register(ui, kind, text(&name), Some(toggle_body), core::ptr::null_mut()) }, id);
        }
        assert_eq!(unsafe { pxui_body_register(ui, kind, text("one_more"), Some(other), core::ptr::null_mut()) }, 0);
        assert_eq!(unsafe { pxui_body_register(ui, kind, text("and_another"), Some(other), core::ptr::null_mut()) }, 0);
        let first = unsafe { (&(*(*ui).shared).bodies.table)[kind as usize][0] };
        assert_eq!(first.0 as *const () as usize, toggle_body as *const () as usize, "body 1 is still its own");
        assert_eq!(unsafe { pxui_body_register(ui, kind, text("7"), Some(other), core::ptr::null_mut()) }, 7, "a name again keeps its id at the limit");
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }

    thread_local! { static LAID: Cell<PxuiBox> = const { Cell::new(PxuiBox { index: 0, box_key: 0, box_slot: 0, generation: 0 }) }; }

    #[repr(C, align(16))]
    struct Raw([u8; core::mem::size_of::<Node>()]);

    extern "C" fn layout_frame(_: *mut c_void, ui: *mut c_void) {
        let ui = ui.cast::<PxuiUi>();
        let mut b = PxuiBox::default();
        assert_eq!(unsafe { crate::pxui_ui_node_keyed(ui, 7, &Node::DEFAULT, &mut b) }, 0);
        // a record shorter than the library's node is refused and not written past its size
        let mut short = Raw([0xaa; core::mem::size_of::<Node>()]);
        short.0[..4].copy_from_slice(&8u32.to_ne_bytes());
        assert_eq!(unsafe { crate::pxui_ui_node_layout(ui, &b, short.0.as_mut_ptr().cast()) }, PXUI_ERR_ARGUMENT);
        assert!(short.0[4..].iter().all(|&x| x == 0xaa), "the library wrote past the caller's record");
        // the look half (`pxui_ui_node_fields`) the same
        let mut short = Raw([0xaa; core::mem::size_of::<Node>()]);
        short.0[..4].copy_from_slice(&8u32.to_ne_bytes());
        assert_eq!(unsafe { crate::pxui_ui_node_fields(ui, &b, short.0.as_mut_ptr().cast()) }, PXUI_ERR_ARGUMENT);
        assert!(short.0[4..].iter().all(|&x| x == 0xaa), "the library wrote past the caller's fields");
        let mut node = Node::DEFAULT;
        assert_eq!(unsafe { crate::pxui_ui_node_layout(ui, &b, &mut node) }, 0);
        node.w = pxui_types::Size::Px(40.0);
        assert_eq!(unsafe { crate::pxui_ui_set_node_layout(ui, &b, &node) }, 0);
        LAID.with(|l| l.set(b));
    }

    /// `pxui_ui_node_layout` honours the caller's `size`; `pxui_ui_set_node_layout` changes only a node the frame being built declared.
    #[test]
    fn node_layout_honours_the_record_size_and_only_this_frames_nodes() {
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        frame(ui, layout_frame);
        let b = LAID.with(Cell::get);
        let mut node = Node::DEFAULT;
        assert_eq!(unsafe { crate::pxui_ui_node_layout(ui, &b, &mut node) }, 0, "a live node is read after its frame");
        assert!(matches!(node.w, pxui_types::Size::Px(w) if w == 40.0));
        assert_eq!(unsafe { crate::pxui_ui_set_node_layout(ui, &b, &Node::DEFAULT) }, PXUI_ERR_STALE, "a retained node is not set outside its build");
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }
    extern "C" fn dialog_body(_: *mut c_void, ui: *mut c_void, config: *const c_void) {
        let c = unsafe { &*config.cast::<PxuiDialogConfig>() };
        assert_eq!(unsafe { pxui_slot(ui.cast(), c.content, core::ptr::null_mut()) }, 0);
    }

    extern "C" fn nested_dialogs(depth: *mut c_void, ui: *mut c_void) {
        let (ui, left) = (ui.cast::<PxuiUi>(), unsafe { *depth.cast::<u32>() });
        let (mut out, buttons) = (pxui_types::ffi::PxuiDialogOut::default(), [text("OK"), text("Cancel")]);
        if left == 0 { assert_eq!(unsafe { crate::pxui_label(ui, text("leaf")) }, 0); return }
        let mut next = left - 1;
        let rc = unsafe { crate::pxui_ui_dialog(ui, text("Title"), Slice { ptr: buttons.as_ptr(), len: 2 }, Some(nested_dialogs), (&mut next as *mut u32).cast(), &mut out) };
        assert_eq!(rc, 0, "{}", error(ui));
    }

    /// A dialog built inside the content of a dialog (both with registered bodies) takes its own span scratch: once the pool has grown, a
    /// frame with three nested dialogs allocates what one dialog does not, nothing.
    #[test]
    fn nested_dialogs_with_registered_bodies_allocate_nothing_once_settled() {
        use mem_sys as _;
        let ui = unsafe { pxui_ui_create(&PxuiUiDesc::default()) };
        assert_eq!(unsafe { pxui_body_register(ui, BodyKind::dialog as u32, text("probe"), Some(dialog_body), core::ptr::null_mut()) }, 1);
        assert_eq!(theme(ui, "dialog.body = probe"), 0);
        let mut depth = 3u32;
        let mut run = || {
            let mut out = FrameOut::default();
            assert_eq!(unsafe { pxui_frame(ui, &FrameIn { width: 400.0, height: 300.0, ..FrameIn::default() }, Some(nested_dialogs), (&mut depth as *mut u32).cast(), &mut out) }, 0, "{}", error(ui));
        };
        for _ in 0..4 { run(); }
        mem_sys::local_reset();
        let before = mem_sys::local_alloc_count();
        for _ in 0..3 { run(); }
        assert_eq!(mem_sys::local_alloc_count() - before, 0, "a settled frame of nested dialogs allocates");
        assert_eq!(unsafe { pxui_ui_destroy(ui) }, 0);
    }
}
