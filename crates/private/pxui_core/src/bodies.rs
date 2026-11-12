//! Widget bodies: a theme's `body.KIND` names, by id, the function that builds a widget kind's nodes; behaviour (box, signal, value,
//! keys, focus, accessibility) stays in the widget's method, and what it needs of the structure it reads from the body's parts
//! (`Ui::part`). Each kind is one line of `pxui_types::body_kinds!`; the registry and the `register_*_body` functions below are
//! generated from it, with the kind's config and body type defined here and its default body next to the widget.
//! A config holds only what behaviour already computed (no measuring, formatting or allocation to fill it); the widget's text comes
//! beside it as `&str`. A body never changes the value, the role or the focus.
use crate::ui_core::*;
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

/// Make `$key` the widget whose parts `Ui::part` keys, enter `$node`, run kind `$kind`'s body and restore both.
macro_rules! in_body {
    ($ui:ident, $key:expr, $node:expr, $kind:ident, $default:ident($($arg:expr),*)) => {{
        let owner = core::mem::replace(&mut $ui.part_owner, $key);
        let saved = $ui.enter(&$node);
        $crate::bodies::body_call!($ui, $kind, $default($($arg),*));
        $ui.leave(saved);
        $ui.part_owner = owner;
    }};
}
pub(crate) use in_body;

macro_rules! registry {
    ($($kind:ident $register:ident $body:ident),* $(,)?) => {
        /// The bodies an app registered, one list per widget kind: a theme's `BodyId` n picks entry n - 1 (and `names[kind]` n - 1 is
        /// its name). Filled at start-up; a frame only reads it.
        pub(crate) struct BodyTable {
            $(pub(crate) $kind: Vec<$body>,)*
            names: Vec<Vec<String>>,
        }

        impl Default for BodyTable {
            fn default() -> BodyTable {
                BodyTable { $($kind: Vec::new(),)* names: Bodies::KINDS.iter().map(|_| Vec::new()).collect() }
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
