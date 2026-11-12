//! The Bodies page: five widget bodies written against the public wrapper (`Ui::register_*_body`, `Ui::part`, `Ui::slot`) and picked by
//! a theme text (`THEME_LINES`: `toggle.body = checkbox`, ...). A checkbox-style toggle, a slider with a round thumb, a segmented control
//! whose marker slides, a table whose rows are ruled and whose cells hold widgets, and a search field with an icon and a clear button.
//! The page itself is plain widget calls: with the bodies not selected it draws the built-in widgets, with them the proof bodies.
//! Not in `PAGES`: it needs the wrapper (the pages there also compile against the engine for the A7 bench).
use pxui::bodies::{text_of, PxuiSearchFieldConfig, PxuiSegmentConfig, PxuiSliderConfig, PxuiToggleConfig, PxuiRowConfig};
use pxui::theme::{Look, State};
use pxui::{ui_kit::PanelOpts, Part, Ui};
use pxui_types::{ffi::PxuiStateDesc, geom::Edges, layout::Align, ui::int_key, Axis, Node, Size};

/// The theme lines that select the proof bodies, on top of any base theme text.
pub const THEME_LINES: &str = "toggle.body = checkbox\nslider.body = round\nsegment.body = sliding\ntable_view.body = ruled\nsearch_field.body = boxed\n";

/// The page's state.
pub struct BodiesPage {
    /// the proof bodies are selected (the host applies `theme_text`)
    pub custom: bool,
    /// the host should apply the theme again
    pub dirty: bool,
    pub wifi: bool,
    pub volume: f32,
    pub align: usize,
    pub query: String,
    pub widths: [f32; 2],
    pub selected: Option<usize>,
    pub flags: [bool; 3],
}

impl BodiesPage {
    pub fn new(custom: bool) -> BodiesPage {
        BodiesPage { custom, dirty: false, wifi: true, volume: 0.6, align: 1, query: String::new(), widths: [120.0, 120.0], selected: Some(1), flags: [true, false, true] }
    }

    /// The theme text the host applies (`Ui::set_theme`): `base` (a theme file's text, or none), then the body lines when `custom`.
    pub fn theme_text(&self, base: &str) -> String {
        if self.custom { format!("{base}\n{THEME_LINES}") } else { base.to_owned() }
    }
}

fn state_of(s: &PxuiStateDesc) -> State {
    State { hovered: s.hovered != 0, pressed: s.pressed != 0, focused: s.focused != 0, disabled: s.disabled != 0, checked: s.checked != 0 }
}

fn ink(ui: &Ui) -> Look { Look { fill: ui.theme().tokens.accent, ..Look::NONE } }

/// A toggle as a checkbox: a 12-point box at the row's start is the hit area, the label after it.
fn checkbox_toggle(ui: &mut Ui, c: &PxuiToggleConfig) {
    let state = state_of(&c.state);
    let (look, boxes) = { let t = ui.theme(); (*t.toggle.pick(state), t.checkbox) };
    ui.set_look(&c.row, look, state);
    ui.node_mut(&c.row, |n| n.inset = Edges { l: 24.0, ..Edges::ZERO });
    ui.text(&c.row, text_of(&c.label));
    let b = ui.part(Part::HIT, Node { at: Some((6.0, 6.0)).into(), w: Size::Px(12.0), h: Size::Px(12.0), ..Node::DEFAULT });
    ui.style(&b, &boxes, State { hovered: state.hovered, checked: c.on != 0, ..State::default() });
}

/// A slider with a round 24-point thumb on a 4-point rail: behaviour maps the pointer across the rail, half the thumb kept at each end.
fn round_slider(ui: &mut Ui, c: &PxuiSliderConfig) {
    let state = state_of(&c.state);
    let (rail, fill, thumb) = { let t = ui.theme(); (t.separator.idle, t.mark.idle, t.slider) };
    let fraction = c.fraction;
    ui.part(Part::RAIL, Node { h: Size::Px(4.0), look: rail, ..Node::DEFAULT });
    ui.node("fill", Node { w: Size::Pct(fraction), h: Size::Px(4.0), look: fill, ..Node::DEFAULT });
    let lane = ui.node("lane", Node { h: Size::Px(24.0), axis: Axis::Row, ..Node::DEFAULT });
    ui.within(&lane, &mut |ui: &mut Ui| {
        ui.node("before", Node { h: Size::Px(1.0), grow: fraction, ..Node::DEFAULT });
        let th = ui.part(Part::THUMB, Node { w: Size::Px(24.0), h: Size::Px(24.0), look: Look { radius: [12.0; 4], ..Look::NONE }, ..Node::DEFAULT });
        ui.style(&th, &thumb, state);
        ui.node("after", Node { h: Size::Px(1.0), grow: 1.0 - fraction, ..Node::DEFAULT });
    });
}

/// Segments with no outline of their own and one marker (`Part::THUMB`, built by the first segment) placed over the chosen segment from
/// that segment's rectangle last frame, so it follows the choice a frame later.
fn sliding_segment(ui: &mut Ui, c: &PxuiSegmentConfig) {
    let state = state_of(&c.state);
    let look = *ui.theme().segment.pick(state);
    let mut node = ui.node_layout(&c.node);
    node.w = Size::Px(70.0);
    ui.set_node_layout(&c.node, &node);
    ui.set_look(&c.node, Look { text: look.text, font: look.font, ..Look::NONE }, state);
    ui.text(&c.node, text_of(&c.text));
    if c.index == 0 {
        let rect_of = |ui: &Ui, key: i64| ui.box_of_key(key).map(|b| ui.rect(&b)).filter(|r| r.w > 0.0);
        let (first, chosen) = (rect_of(ui, c.node.box_key), rect_of(ui, int_key(c.group, c.selected as i64)));
        let (at, w) = match (first, chosen) { (Some(f), Some(s)) => ((s.x - f.x, 0.0), s.w), _ => ((0.0, 0.0), 40.0) };
        let marker = ink(ui);
        ui.part(Part::THUMB, Node { at: Some(at).into(), w: Size::Px(w), h: Size::Px(3.0), look: marker, ..Node::DEFAULT });
    }
}

/// A table row with a hairline, the caller's cells inside.
fn ruled_row(ui: &mut Ui, c: &PxuiRowConfig) {
    let state = state_of(&c.state);
    let accent = ui.theme().tokens.accent;
    let look = if state.checked { Look { fill: accent, ..Look::NONE } } else { Look { border: accent, border_w: 1.0, ..Look::NONE } };
    ui.set_look(&c.row, look, state);
    ui.slot(c.content);
}

/// A search field as a 24-point well (the row the kit gives it): a 14-point round icon, the editing surface (the slot) and, with a query, a 20-point clear button.
fn boxed_search(ui: &mut Ui, c: &PxuiSearchFieldConfig) {
    let state = state_of(&c.state);
    let (look, hint, accent) = { let t = ui.theme(); (*t.search.pick(state), t.placeholder.idle, t.tokens.accent) };
    let mut well = ui.node_layout(&c.well);
    (well.h, well.axis, well.align, well.gap, well.padding) = (Size::Px(24.0), Axis::Row, Align::Center, 6.0, Edges { l: 6.0, ..Edges::ZERO });
    ui.set_node_layout(&c.well, &well);
    ui.set_look(&c.well, look, state);
    ui.node("icon", Node { w: Size::Px(14.0), h: Size::Px(14.0), look: Look { fill: accent, radius: [7.0; 4], ..Look::NONE }, ..Node::DEFAULT });
    if ui.slot(c.editor) {
        let clear = ui.part(Part::CLEAR, Node { flags: pxui_types::ui::CLICKABLE | pxui_types::ui::BLOCKING, w: Size::Px(20.0), h: Size::Px(20.0), look: Look { text: hint.text, ..Look::NONE }, ..Node::DEFAULT });
        ui.text(&clear, "x");
    }
}

/// Registers the proof bodies; `THEME_LINES` selects them. Once, before the theme that names them is applied.
pub fn register(ui: &mut Ui) -> Result<(), String> {
    ui.register_toggle_body("checkbox", checkbox_toggle)?;
    ui.register_slider_body("round", round_slider)?;
    ui.register_segment_body("sliding", sliding_segment)?;
    ui.register_table_view_body("ruled", ruled_row)?;
    ui.register_search_field_body("boxed", boxed_search)?;
    Ok(())
}

const NAMES: [&str; 3] = ["Wi-Fi", "Bluetooth", "Hotspot"];

/// The page: the widgets the proof bodies replace, as plain calls.
pub fn page(ui: &mut Ui, s: &mut BodiesPage) {
    ui.panel(PanelOpts::default().width(296.0), "bodies", &mut |ui: &mut Ui| {
        ui.label("Bodies");
        let custom = ui.toggle("Custom bodies", s.custom);
        if custom != s.custom { (s.custom, s.dirty) = (custom, true); }
        s.wifi = ui.toggle("Wi-Fi", s.wifi);
        s.volume = ui.slider("Volume", s.volume, 0.0..=1.0);
        s.align = ui.segmented("Align", &["Left", "Centre", "Right"], s.align);
        if let Some(v) = ui.search_field("Find", "?", &s.query, "Search") { s.query = v; }
    });
    ui.panel(PanelOpts::default().at(332.0, 12.0).width(296.0).window(true), "table", &mut |ui: &mut Ui| {
        let (widths, selected, flags) = (&mut s.widths, &mut s.selected, &mut s.flags);
        // the table fills its parent: give it a height
        let frame = ui.node("frame", Node { h: Size::Px(140.0), ..Node::DEFAULT });
        ui.within(&frame, &mut |ui: &mut Ui| {
            let _ = ui.table_view_with("Settings", &["Name", "On"], widths, 3, None, selected, &mut |ui: &mut Ui, row, column| {
                if column == 0 { ui.label(NAMES[row]); } else { flags[row] = ui.toggle(NAMES[row], flags[row]); }
            });
        });
    });
}
