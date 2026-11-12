//! Widget bodies: a theme's `body.KIND` names, by id, the function that builds a widget kind's nodes; behaviour (box, signal, value,
//! keys, focus, accessibility) stays in the widget's method, and what it needs of the structure it reads from the body's parts
//! (`Ui::part`). Each kind is one line of `pxui_types::body_kinds!`; the registry and the `register_*_body` functions below are
//! generated from it, with the kind's config and body type defined here and its default body next to the widget.
//! A config holds only what behaviour already computed (no measuring, formatting or allocation to fill it); the widget's text comes
//! beside it as `&str`. A body never changes the value, the role or the focus.
use crate::ui_core::*;
use crate::opts::ValueFieldOpts;
use crate::ui_kit::part_key;

/// What a toggle body is given. It runs with the widget's row entered (`row`, whose `box_key` owns the parts) and builds the row's
/// look, label and switch. Behaviour reads one part: `Part::HIT`, the area that takes presses (the whole row when the body builds none).
pub struct ToggleConfig {
    pub state: State,
    pub on: bool,
    pub row: Handle,
}

pub type ToggleBody = fn(&mut Ui, &ToggleConfig, &str);

/// What a slider body is given (and the label beside it, as a `&str`; the label itself is built by behaviour). It runs with `track` entered: the node that takes the pointer and keys (a 20-point stack the
/// full width of the row), and builds the rail, the filled part and the thumb inside it. Behaviour reads two parts to turn the
/// pointer into a value: the rail built as `Part::RAIL` (the pointer maps across its width) and the thumb built as `Part::THUMB`
/// (half its width is kept at each end). A body without them gets the whole `track` and no thumb. `fraction` is the value's
/// place, 0 to 1; the body's own thumb carries it.
pub struct SliderConfig {
    pub state: State,
    pub fraction: f32,
    pub track: Handle,
}

pub type SliderBody = fn(&mut Ui, &SliderConfig, &str);

/// What a progress body is given (and the label beside it, as a `&str`). It runs with the row entered (its `box_key` owns the parts) and builds the label, the track and the
/// bar. Behaviour reads and sets the node built as `Part::TRACK` (the accessible name, number and busy state, the sweep's extent)
/// and `busy_step` moves the node built as `Part::BAR` along it; a determinate bar is the body's own business.
pub struct ProgressConfig {
    pub value: Option<f32>,
    pub row: Handle,
}

pub type ProgressBody = fn(&mut Ui, &ProgressConfig, &str);

/// What a button body is given (the caller's `States`, then the button's text, beside it: `states.pick(state)` is the look the
/// caller asked for). The button is a `row` (its key, hit area, focus and accessible name, the node a press lands on) inside a `line`
/// that places it in its row; both come with their behaviour only, and the body sets their layout and look (`Ui::node_mut`) and
/// builds what is inside `row`, which it runs with entered. `bare`, `icon` and `at_end` are the variant the caller asked for.
/// Behaviour reads no part.
pub struct ButtonConfig {
    pub state: State,
    pub bare: bool,
    pub icon: bool,
    pub at_end: bool,
    pub line: Handle,
    pub row: Handle,
}

pub type ButtonBody = fn(&mut Ui, &ButtonConfig, &States, &str);

/// What an icon button body is given (its label, then its icon, beside it). It runs with the button's `node` entered, which carries
/// the key, the hit area and the accessible name; the body sets its size and look (the theme's `icon_button` for `state`) and draws
/// the icon.
pub struct IconButtonConfig {
    pub state: State,
    pub node: Handle,
}

pub type IconButtonBody = fn(&mut Ui, &IconButtonConfig, &str, &str);

/// What a checkbox or radio body is given (the option's text beside it). It runs with the `row` entered, which takes the presses
/// and carries the role, the checked state and the accessible name; the body sets the row's look and builds the box or ring and
/// the label. `state` is hovered and pressed; `on` is the value. Behaviour reads no part.
pub struct CheckConfig {
    pub state: State,
    pub on: bool,
    pub row: Handle,
}

pub type CheckBody = fn(&mut Ui, &CheckConfig, &str);

/// What a spinner body is given (its label beside it). It runs with the spinner's `node` entered, which carries the role, the busy
/// state and the accessible name; the body sizes it and builds the dots. `busy_step` turns the nodes built as `Part::dot(i)`, i from
/// 0, until the first missing one: a body with none does not animate.
pub struct SpinnerConfig {
    pub node: Handle,
}

pub type SpinnerBody = fn(&mut Ui, &SpinnerConfig, &str);

/// What a badge body is given (its label, then its text, beside it). It runs with the badge's `node` entered, which carries the
/// role, the accessible name and value and the key `patch_text` finds; the body sizes and looks it and puts the text on it (a count
/// patched later lands there).
pub struct BadgeConfig {
    pub node: Handle,
}

pub type BadgeBody = fn(&mut Ui, &BadgeConfig, &str, &str);

/// What a label body is given (its text beside it). It runs with the label's `node` entered; `first` is whether it is the first
/// thing of its parent (the default body gives it 4 points above).
pub struct LabelConfig {
    pub node: Handle,
    pub first: bool,
}

pub type LabelBody = fn(&mut Ui, &LabelConfig, &str);

/// What a footer body is given (the key at its end, then the key and what-it-does pairs, beside it). It runs with the footer's `row`
/// entered, which carries the status-bar role, and builds everything: the rule, the hints, the key at the end.
pub struct FooterConfig {
    pub row: Handle,
}

pub type FooterBody = fn(&mut Ui, &FooterConfig, Option<&str>, &[(String, String)]);

/// What a grip body is given. It runs with the grip's `grip` box entered: the line between panes, which carries the role and takes
/// the cursor and the drag; its place is the caller's (`floating`: at a rectangle the caller gave, else a 1-point flow child that
/// fills the row or column across, `vertical` for a line between side-by-side panes). The body sets its look, may set its
/// thickness, and sets its press band with `Ui::hit_outset` on `grip` (the box itself when it sets none). Behaviour reads no part.
pub struct GripConfig {
    pub state: State,
    pub vertical: bool,
    pub floating: bool,
    pub grip: Handle,
}

pub type GripBody = fn(&mut Ui, &GripConfig);

/// What a panel, window, popup or modal body is given (its label, then the caller's content, beside it). The sheet is `node`, entered:
/// it already carries the kit's look, scroll, clip, size, role and name, which the body may restyle (`Ui::node_mut`, `set_look`) or
/// surround with chrome of its own; it calls `content` once where the content goes. Popup placement, stacking, dismissal and the
/// scroll offset stay with behaviour, which reads the sheet's own box. `window` is the `window` kind (a panel opened with
/// `PanelOpts::window`), the `modal` kind is the sheet a modal or dialog opens, the `popup` kind any other popup.
pub struct PanelConfig {
    pub node: Handle,
}

pub type PanelBody = fn(&mut Ui, &PanelConfig, &str, &mut dyn FnMut(&mut Ui));

/// What a dialog body is given (its title, its button labels, then the caller's content, beside it). It runs inside the modal's sheet
/// (entered) and builds the title bar, calls `content` once, then builds the buttons; the sheet itself is the `modal` body's.
/// Behaviour reads three kinds of part: the title bar built as `Part::TITLE` (it gets the heading role and the title as name; none, no
/// heading), and button `i` built as `Part::button(i)`, a clickable node (it gets the button role, its label as name and a press on it
/// is the dialog's answer). Escape, a press outside and Enter (the last button) are behaviour's.
pub type DialogBody = fn(&mut Ui, &str, &[&str], &mut dyn FnMut(&mut Ui));

/// What an inspector section body is given (its label, then the caller's content, beside it). It runs in the section's parent, not
/// entered: `title` is the header box (a full-width row that takes the click, carries the heading role, the expandable and
/// expanded states and the name, and has the height and the gap above it already), `open` its state. The body styles the header
/// (look, inset, text, the disclosure mark; `gap` is the space above that the section's place asks for) and calls `content` once
/// after it when `open`. Behaviour reads no part: a press anywhere on `title` toggles it.
pub struct SectionConfig {
    pub title: Handle,
    pub open: bool,
    pub gap: f32,
}

pub type SectionBody = fn(&mut Ui, &SectionConfig, &str, &mut dyn FnMut(&mut Ui));

/// What an inspector row body is given (its label beside it). It runs with the row entered: a clickable row of the kit's height whose
/// label starts at `label_x` (its inset), carrying the label role and name. The body gives it the look for `state`, puts the label
/// on it fitted into `label_w` points and draws the pin dot when `pin` (`Some(on)`) is given. The control the caller puts
/// in the row is the caller's, at the places the call returns. Behaviour reads no part.
pub struct InspectorRowConfig {
    pub row: Handle,
    pub state: State,
    pub label_w: f32,
    pub pin: Option<bool>,
}

pub type InspectorRowBody = fn(&mut Ui, &InspectorRowConfig, &str);

/// What a split body is given (the two panes' content, beside it). It runs with the `split` box entered and builds, in order, the first
/// pane, the grip and the second pane; each pane is a growing node with `grow` and the minimum along `axis` (`min_a`, `min_b`) that
/// calls its content once. The grip is built with `Ui::grip(format_args!("grip"), axis, None)` (its own body is the `grip` kind's):
/// behaviour reads its signal for the drag and the returned ratio; a body that builds none cannot be resized.
pub struct SplitConfig {
    pub split: Handle,
    pub axis: Axis,
    pub ratio: f32,
    pub min_a: f32,
    pub min_b: f32,
}

pub type SplitBody = fn(&mut Ui, &SplitConfig, &mut dyn FnMut(&mut Ui), &mut dyn FnMut(&mut Ui));

/// What a scroll view body is given (the caller's content beside it). The view is `node`, entered: it carries the scroll flags,
/// the offset, the role and the name, and the kit's look, padding and scrollbar (`Node::bar`: thumb colour, radius, width, gap),
/// which the body may restyle; it calls `content` once. Scrolling, the thumb's drag and the offset are the engine's. Behaviour reads no part.
pub struct ScrollConfig {
    pub node: Handle,
}

pub type ScrollBody = fn(&mut Ui, &ScrollConfig, &mut dyn FnMut(&mut Ui));

/// What a toast body is given (its text beside it). It runs with the toast's `node` entered (keyed by its text, clickable, the
/// status role and the clock behaviour keeps); the body sizes and looks it for `state` (hovered) and puts the text on it. The stack's
/// place, order and gap are behaviour's; a click, or the toast's time, is what dismisses it.
pub struct ToastConfig {
    pub state: State,
    pub node: Handle,
}

pub type ToastBody = fn(&mut Ui, &ToastConfig, &str);

/// What a tooltip body is given (the text's wrapped lines beside it, which behaviour already cut to `width`). It runs with the
/// `tip` sheet entered: a root node `width` x `height` (the room the lines take, whatever the body draws) at the place behaviour chose,
/// with the tooltip role, the full text as name and the overlay order. The body gives it its look and builds the lines.
pub struct TipConfig {
    pub tip: Handle,
    pub width: f32,
    pub height: f32,
}

pub type TipBody = fn(&mut Ui, &TipConfig, &[String]);

/// What a tab body is given (the tab's text beside it). It runs with the tab's `node` entered (the built-in body is called without it):
/// a growing-height row item, keyed by its place in the bar, with the tab role, its name and its selected state; behaviour reads
/// its presses. The body gives it its width (`Size::Px`, the text's by default), its look for `state` (`checked`: it is the tab in
/// use) and the text. A click on a later tab unselects an earlier one in the same frame; a body's own look follows on the next frame.
pub struct TabConfig {
    pub state: State,
    pub index: usize,
    pub node: Handle,
}

pub type TabBody = fn(&mut Ui, &TabConfig, &str);

/// What a segmented-control body is given (the option's text beside it). It runs with the segment's `node` entered: a radio-role item
/// keyed by its place in the strip, checked when it is the choice; behaviour reads its presses. `selected` is the choice's index and
/// `group` the strip's key, so a body can find the choice's segment (`Ui::part_rect(int_key(group, selected), part)`) and move a
/// marker to it. The body gives the segment its width, inset, look for `state` (`checked`) and text.
pub struct SegmentConfig {
    pub state: State,
    pub index: usize,
    pub selected: usize,
    pub group: i64,
    pub node: Handle,
}

pub type SegmentBody = fn(&mut Ui, &SegmentConfig, &str);

/// What a dropdown body is given (the chosen option's text beside it). It runs inside the `button` (entered): the combo box that takes
/// the presses and keys, with its role, expanded state and name; `key` is the dropdown's own key (the built-in body keys its text and
/// chevron from it). The body gives the button its look for `state` and builds what shows in it. The open menu is built after the body
/// returns, under the button; its rows are the `menu_item` kind's.
pub struct DropdownConfig {
    pub state: State,
    pub open: bool,
    pub key: i64,
    pub button: Handle,
}

pub type DropdownBody = fn(&mut Ui, &DropdownConfig, &str);

/// What a menu bar title (`menu_bar`) or a row of a dropdown's or menu bar's open menu (`menu_item`) body is given (its text beside
/// it). It runs with the item's `node` entered (the built-in body is called without it): a row item with the menu-item role (and, for a
/// title, its expandable and expanded states) whose presses and hover behaviour reads. `state.checked` is a title's menu being open or
/// a row being the lit one. The body gives the item its size, inset, look and text.
pub struct MenuItemConfig {
    pub state: State,
    pub index: usize,
    pub node: Handle,
}

pub type MenuItemBody = fn(&mut Ui, &MenuItemConfig, &str);

/// What a context menu row body is given (its text, then its shortcut, beside it). It runs with the `row` entered: a row of the kit's
/// height that takes the click and the pointer when it is enabled, with the menu-item role and its checked and expandable states.
/// The body gives it the look for `state` (`hovered`: the pointer, or its submenu is open; `disabled`) with `danger` picking the
/// error look, its inset, text and what sits in it: the current-choice mark (`selected`), a colour `swatch`, a submenu `chevron`, the
/// shortcut. `lead` and `swatch_pad` are the menu's room for the mark and the swatches. Separators are behaviour's.
pub struct MenuRowConfig {
    pub state: State,
    pub index: i64,
    pub row: Handle,
    pub selected: bool,
    pub chevron: bool,
    pub danger: bool,
    pub lead: f32,
    pub swatch_pad: f32,
    pub swatch: Option<crate::geom::Rgba>,
}

pub type MenuRowBody = fn(&mut Ui, &MenuRowConfig, &str, Option<&str>);

/// What a `list_view` or `table_view` row body is given (the row's content beside it: the text, or the caller's cells, which it calls
/// once where it goes). It runs with the `row` entered (the built-in body is called without owning parts): a full-width row item that
/// carries the kit's look and padding, its role, selected state and presses; the body may restyle it (`Ui::node_mut`, `set_look`) and
/// builds what is inside. `state`: hovered, pressed, `checked` (selected), `focused` (the keyboard's row). Which rows are built, the
/// selection and the keys are behaviour's. In a table the content is the cells, built with their widths and roles.
pub struct RowConfig {
    pub state: State,
    pub index: usize,
    pub row: Handle,
}

pub type RowBody = fn(&mut Ui, &RowConfig, &mut dyn FnMut(&mut Ui));

/// What a tree row body is given (the row's content beside it). Like `RowConfig`; `node` is the tree node the row shows, `depth` its
/// level, `has` whether it has children and `open` whether they show. Behaviour reads one part: the node built as `Part::DISCLOSURE`
/// (a press on it expands or collapses). The built-in body builds the disclosure mark (a spacer without children) and then the content.
pub struct TreeRowConfig {
    pub state: State,
    pub index: usize,
    pub node: usize,
    pub depth: usize,
    pub has: bool,
    pub open: bool,
    pub row: Handle,
}

pub type TreeRowBody = fn(&mut Ui, &TreeRowConfig, &mut dyn FnMut(&mut Ui));

/// What a table header cell body is given (the column's title beside it). It runs with the cell's `node` entered: the column's width,
/// the column-header role and name, and the presses (a click is the sort intent). `ascending` is the sort direction when the table is
/// sorted by this column. The body gives the node its look for `state` (`checked`: it is the sorted column) and builds the title and
/// the sort mark. The resize grip after it is behaviour's.
pub struct TableHeadConfig {
    pub state: State,
    pub index: usize,
    pub ascending: Option<bool>,
    pub node: Handle,
}

pub type TableHeadBody = fn(&mut Ui, &TableHeadConfig, &str);

/// What a read-only `table` cell body is given (the cell's text beside it, as the caller's builder gave it). The cell is a header cell
/// (`header`, with `row` -1) or the data cell of `row`; `key` is the key its node takes (the row's or the header's own, then the
/// column); the column spans `left` to `right` across the table and the row is `height` high. The body builds the cell's nodes
/// (the built-in one a node `height` high, left-aligned for a header and right-aligned for data). The row's hit area, its look and
/// the clicked row are behaviour's.
pub struct TableCellConfig {
    pub header: bool,
    pub row: i64,
    pub column: i64,
    pub key: i64,
    pub left: f32,
    pub right: f32,
    pub height: f32,
}

pub type TableCellBody = fn(&mut Ui, &TableCellConfig, &str);

/// What a picker row body is given (the row's label, its detail and the query that was typed, beside it). It runs with the `row`
/// entered: a clickable row of the kit's height with the list-item role and its name; behaviour reads its click. The body gives it the
/// look for `state` (`checked`: the keyboard's row; `hovered`), with `doomed` (armed for deletion) and `unavailable` (cannot be
/// picked) its looks, and builds the label (the query's letters in the accent), the detail, the colour `swatch` when the picker marks
/// rows (`marked`) and the Enter hint on the keyboard's row.
pub struct PickerRowConfig {
    pub state: State,
    pub index: i64,
    pub current: bool,
    pub doomed: bool,
    pub unavailable: bool,
    pub marked: bool,
    pub swatch: Option<crate::geom::Rgba>,
    pub row: Handle,
}

pub type PickerRowBody = fn(&mut Ui, &PickerRowConfig, &str, &str, &str);

/// What a search field body is given (its icon beside it, then `editor`). It runs with the `well` entered (a node of the kit's key
/// with no layout or look yet, which the body sizes and styles for `state`) and builds what is in it, in order: the icon,
/// the editing surface (it calls `editor` once, which builds the editing node, runs the editing and returns whether the query is not
/// empty) and, when it is not, the clear button. `key` is the field's own. Behaviour reads one part: the node built as `Part::CLEAR`
/// (a press on it clears the query; behaviour gives it the button role and the name Clear). Caret, selection, undo and the
/// accessible text are the editing node's, which behaviour made.
pub struct SearchFieldConfig {
    pub state: State,
    pub well: Handle,
    pub key: i64,
}

pub type SearchFieldBody = fn(&mut Ui, &SearchFieldConfig, &str, &mut dyn FnMut(&mut Ui) -> bool);

/// What a text field body is given (the field's label, then its value, beside it). It runs with the `row` entered: a row of the kit's
/// height that takes presses on its `Part::EDITOR` (the whole row until one has been laid out) and carries the role, name and value.
/// The body gives it its look for `state` (`hovered`, `focused`), the inset, the label, and builds the editing surface as
/// `Part::EDITOR`: a node whose rectangle behaviour reads for the caret and the I-beam, which shows `value` unless `state.focused`
/// (then behaviour paints the text, caret and selection on it). A body that builds none edits on the row.
pub struct TextFieldConfig {
    pub state: State,
    pub row: Handle,
}

pub type TextFieldBody = fn(&mut Ui, &TextFieldConfig, &str, &str);

/// What a line field body is given (what it shows beside it: the placeholder while the text is empty, else the text). It runs with the
/// `field` entered, the box that takes presses and keys and is the editing surface: the body gives it its look for `state`
/// (`focused`; `empty`: the text is, so the placeholder shows) and an inset of 2 points at the start (where editing places the text),
/// and puts the shown text on it unless focused (behaviour then paints the text, caret and selection). It may build more nodes inside.
pub struct LineFieldConfig {
    pub state: State,
    pub empty: bool,
    pub field: Handle,
}

pub type LineFieldBody = fn(&mut Ui, &LineFieldConfig, &str);

/// What a value field body is given (the text it shows beside it, and the caller's `ValueFieldOpts`). It runs with the `field` entered,
/// the box that takes the pointer and the keys and is the editing surface. `state.focused` is the field being edited (behaviour then
/// paints the text, caret and selection and the body leaves the inside alone); `invalid` is the text being refused. The body gives
/// the box its look and builds the fraction line, the placeholder, the lead and the trail and the value, `w` x `h` points, with the
/// text starting 2 points in (where editing places it).
pub struct ValueFieldConfig {
    pub state: State,
    pub invalid: bool,
    pub w: f32,
    pub h: f32,
    pub field: Handle,
}

pub type ValueFieldBody = fn(&mut Ui, &ValueFieldConfig, &ValueFieldOpts, &str);

/// What a picker's search field body is given (its placeholder beside it). It runs with the `search` entered, the box that takes
/// presses and keys and is the editing surface: the body gives it its look for `state` (`focused`; a field `resting` has the hairline
/// and no caret) and builds the `/` prefix when `slash` and, while `empty`, the placeholder. Editing places the query `prefix` points
/// in from the box's start plus `inset` more (behaviour paints it, the caret and the selection, over the body's nodes): the body
/// leaves that room.
pub struct PickerFieldConfig {
    pub state: State,
    pub empty: bool,
    pub slash: bool,
    pub prefix: f32,
    pub inset: f32,
    pub search: Handle,
}

pub type PickerFieldBody = fn(&mut Ui, &PickerFieldConfig, &str);

/// The index of `name` among a kind's registered names, added when new; `None` for `default`, which is no registered body.
fn body_slot(names: &mut Vec<String>, name: &str) -> Option<usize> {
    if name == "default" {
        return None;
    }
    Some(names.iter().position(|n| n == name).unwrap_or_else(|| {
        names.push(name.to_owned());
        names.len() - 1
    }))
}

fn kind_index(kind: &str) -> usize {
    Bodies::KINDS.iter().position(|k| *k == kind).unwrap_or(0)
}

/// Run kind `$kind`'s body for widget `$ui`: the built-in `$default` (a direct call) for body 0, else the registered one,
/// `$default` again for an id nothing was registered under. The arguments follow the `&mut Ui`.
macro_rules! body_call {
    ($ui:ident, $kind:ident, $default:ident($($arg:expr),*)) => {
        match $ui.theme().body.$kind {
            BodyId(0) => $default($ui, $($arg),*),
            id => match $ui.bodies.$kind.get((id.0 as usize).wrapping_sub(1)).copied() {
                Some(body) => body($ui, $($arg),*),
                None => $default($ui, $($arg),*),
            },
        }
    };
}
pub(crate) use body_call;

/// Make `$key` the widget whose parts `Ui::part` keys, enter `$node`, run kind `$kind`'s body and restore both. With `flat` the
/// built-in body (which builds no parts and no children, so needs neither) is called as it is.
macro_rules! in_body {
    (flat; $ui:ident, $key:expr, $node:expr, $kind:ident, $default:ident($($arg:expr),*)) => {
        match $ui.theme().body.$kind {
            BodyId(0) => $default($ui, $($arg),*),
            id => match $ui.bodies.$kind.get((id.0 as usize).wrapping_sub(1)).copied() {
                Some(body) => {
                    let owner = core::mem::replace(&mut $ui.part_owner, $key);
                    let saved = $ui.enter(&$node);
                    body($ui, $($arg),*);
                    $ui.leave(saved);
                    $ui.part_owner = owner;
                }
                None => $default($ui, $($arg),*),
            },
        }
    };
    ($ui:ident, $key:expr, $node:expr, $kind:ident, $default:ident($($arg:expr),*)) => {{
        let owner = core::mem::replace(&mut $ui.part_owner, $key);
        let saved = $ui.enter(&$node);
        $crate::bodies::body_call!($ui, $kind, $default($($arg),*));
        $ui.leave(saved);
        $ui.part_owner = owner;
    }};
}
pub(crate) use in_body;

/// `in_body!` for a widget whose body runs where it was called, with no node entered (an inspector section: its header and its content
/// are siblings). The built-in body is called as it is; a registered one first becomes the owner of `Ui::part`.
macro_rules! in_place {
    ($ui:ident, $key:expr, $kind:ident, $default:ident($($arg:expr),*)) => {
        match $ui.theme().body.$kind {
            BodyId(0) => $default($ui, $($arg),*),
            id => match $ui.bodies.$kind.get((id.0 as usize).wrapping_sub(1)).copied() {
                Some(body) => {
                    let owner = core::mem::replace(&mut $ui.part_owner, $key);
                    body($ui, $($arg),*);
                    $ui.part_owner = owner;
                }
                None => $default($ui, $($arg),*),
            },
        }
    };
}
pub(crate) use in_place;

/// Kind `$kind`'s registered body, `None` for the built-in (body 0, or an id nothing was registered under): a collection resolves it once
/// before its item loop.
macro_rules! body_of {
    ($ui:ident, $kind:ident) => {
        match $ui.theme().body.$kind {
            BodyId(0) => None,
            id => $ui.bodies.$kind.get((id.0 as usize).wrapping_sub(1)).copied(),
        }
    };
}
pub(crate) use body_of;

/// `in_body!` for one item of a collection, with `$body` resolved by `body_of!`: the built-in body is called with `$node` entered (with
/// `flat`, as it is: it builds no children; with `place`, nothing is entered) and owns no parts; a registered one gets both.
macro_rules! in_item {
    (place; $ui:ident, $body:expr, $key:expr, $default:ident($($arg:expr),*)) => {
        match $body {
            None => $default($ui, $($arg),*),
            Some(body) => {
                let owner = core::mem::replace(&mut $ui.part_owner, $key);
                body($ui, $($arg),*);
                $ui.part_owner = owner;
            }
        }
    };
    (flat; $ui:ident, $body:expr, $key:expr, $node:expr, $default:ident($($arg:expr),*)) => {
        match $body {
            None => $default($ui, $($arg),*),
            Some(body) => {
                let owner = core::mem::replace(&mut $ui.part_owner, $key);
                let saved = $ui.enter(&$node);
                body($ui, $($arg),*);
                $ui.leave(saved);
                $ui.part_owner = owner;
            }
        }
    };
    ($ui:ident, $body:expr, $key:expr, $node:expr, $default:ident($($arg:expr),*)) => {
        match $body {
            None => {
                let saved = $ui.enter(&$node);
                $default($ui, $($arg),*);
                $ui.leave(saved);
            }
            Some(body) => {
                let owner = core::mem::replace(&mut $ui.part_owner, $key);
                let saved = $ui.enter(&$node);
                body($ui, $($arg),*);
                $ui.leave(saved);
                $ui.part_owner = owner;
            }
        }
    };
}
pub(crate) use in_item;

macro_rules! registry {
    ($($kind:ident $register:ident $body:ident),* $(,)?) => {
        /// The bodies an app registered, one list per widget kind: a theme's `BodyId` n picks entry n - 1 (and `names[kind]` n - 1 is
        /// its name). Filled at start-up; a frame only reads it.
        pub(crate) struct BodyTable {
            $(pub(crate) $kind: Vec<$body>,)*
            names: Vec<Vec<String>>,
            /// the row a list, tree or table view is building (`Ui::item`); here, not in `Ui`, so `Ui` stays the size it was
            pub(crate) item: UiBox,
        }

        impl Default for BodyTable {
            fn default() -> BodyTable {
                BodyTable { $($kind: Vec::new(),)* names: Bodies::KINDS.iter().map(|_| Vec::new()).collect(), item: UiBox { index: 0, box_key: 0, box_slot: 0, generation: 0 } }
            }
        }

        impl Ui {
            $(
                #[doc = concat!("Register `body` as the ", stringify!($kind), " body called `name`, for a theme's `body.", stringify!($kind), "` (or `", stringify!($kind), ".body = NAME` in a theme file).")]
                /// Registering a name again replaces its body and keeps its id; `default` is the built-in body and registers nothing.
                // ponytail: ids are u16; a start-up that registers 65,535 bodies of one kind wraps, so widen `BodyId` then.
                pub fn $register(&mut self, name: &str, body: $body) -> BodyId {
                    let Some(i) = body_slot(&mut self.bodies.names[kind_index(stringify!($kind))], name) else { return BodyId::DEFAULT };
                    if i == self.bodies.$kind.len() { self.bodies.$kind.push(body) } else { self.bodies.$kind[i] = body }
                    BodyId(i as u16 + 1)
                }
            )*
        }
    };
}

pxui_types::body_kinds!(registry);

impl Ui {
    /// `Theme::parse` where `KIND.body = NAME` names the bodies this `Ui` registered; an unregistered name is a `ThemeError`.
    pub fn parse_theme(&self, text: &str, base: &Theme) -> Result<Theme, Vec<crate::theme::ThemeError>> {
        let named = |kind: &str, name: &str| self.bodies.names[kind_index(kind)].iter().position(|n| n == name).map(|i| BodyId(i as u16 + 1));
        Theme::parse_bodies(text, base, &named)
    }

    /// A node of the widget whose body is building, found again by its role: keyed from the widget and `part`, so behaviour
    /// reads its rectangle (`part_rect`) without a table. A body builds the parts its widget's config lists with this.
    pub fn part(&mut self, part: Part, node: Node) -> Handle {
        self.node_keyed(part_key(self.part_owner, part), node)
    }

    /// The box of `part` of the widget keyed `owner`, if it was built.
    pub fn part_box(&self, owner: i64, part: Part) -> Option<UiBox> {
        self.box_of_key(part_key(owner, part)).filter(|b| self.live(b))
    }

    /// The rectangle `part` of the widget keyed `owner` had last frame; `None` before it has been laid out (its first frame) or if
    /// the body builds none.
    pub fn part_rect(&self, owner: i64, part: Part) -> Option<Rect> {
        self.part_box(owner, part).map(|b| self.rect(&b)).filter(|r| r.w > 0.0 && r.h > 0.0)
    }
}
