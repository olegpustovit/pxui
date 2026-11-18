//! The Bodies page: eight widget bodies written against the public wrapper (`Ui::register_*_body`, `Ui::part`, `Ui::slot`) and picked by
//! a theme text (`THEME_LINES`: `toggle.body = checkbox`, ...). A checkbox-style toggle, a slider with a round thumb, a segmented control
//! whose marker slides, a table whose rows are ruled and whose cells hold widgets, a search field with an icon and a clear button, and a
//! section index whose letters are pills, the active one filled, a disclosure whose header is its text after a + or - sign, and a modal sheet with a grab handle when it is a bottom or full sheet.
//! It also shows a picker whose rows have group headings and glyph art.
//! The page itself is plain widget calls: with the bodies not selected it draws the built-in widgets, with them the proof bodies.
//! Not in `PAGES`: it needs the wrapper (the pages there also compile against the engine for the A7 bench).
use pxui::bodies::{BodyConfig, InspectorSectionConfig, ModalConfig, SearchFieldConfig, SectionIndexConfig, SegmentedConfig, SliderConfig, ToggleConfig, TableRowConfig};
use pxui::theme::{Look, State};
use pxui::{Overlay, Part, SearchField, Ui};
use pxui_types::Flags;
use pxui_types::{ffi::PxuiStateDesc, geom::Edges, layout::Align, Axis, Node, Size};

/// The theme lines that select the proof bodies, on top of any base theme text.
pub const THEME_LINES: &str = "toggle.body = checkbox\nslider.body = round\nsegmented.body = sliding\ntable_row.body = ruled\nsearch_field.body = boxed\nsection_index.body = pills\ninspector_section.body = signed\nmodal.body = handle\n";

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
    pub letter: usize,
    pub more: bool,
    /// the bottom sheet is open
    pub sheet: bool,
}

impl BodiesPage {
    pub fn new(custom: bool) -> BodiesPage {
        BodiesPage { custom, dirty: false, wifi: true, volume: 0.6, align: 1, query: String::new(), widths: [120.0, 120.0], selected: Some(1), flags: [true, false, true], letter: 0, more: true, sheet: true }
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
fn checkbox_toggle(ui: &mut Ui, c: &ToggleConfig) {
    let state = state_of(&c.state);
    let (look, boxes) = { let t = ui.theme(); (*t.toggle.pick(state), t.checkbox) };
    ui.style(&c.row, &pxui::theme::States::of(look), state);
    ui.node_mut(&c.row, &mut |n| n.inset = Edges { l: 24.0, ..Edges::ZERO });
    ui.text(&c.row, c.text_of(&c.label));
    let b = ui.part(Part::HIT, Node { at: Some((6.0, 6.0)).into(), w: Size::Px(12.0), h: Size::Px(12.0), ..Node::DEFAULT });
    ui.style(&b, &boxes, State { hovered: state.hovered, checked: c.on != 0, ..State::default() });
}

/// A slider with a round 24-point thumb on a 4-point rail: behaviour maps the pointer across the rail, half the thumb kept at each end.
fn round_slider(ui: &mut Ui, c: &SliderConfig) {
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

/// Segments with no outline of their own and one marker, built by the chosen segment into the strip under one key, so it slides
/// between segments: placed from the chosen segment's rectangle last frame, it follows the choice a frame later.
fn sliding_segment(ui: &mut Ui, c: &SegmentedConfig) {
    let state = state_of(&c.state);
    let look = *ui.theme().segment.pick(state);
    let mut node = ui.node_layout(&c.node);
    node.w = Size::Px(70.0);
    ui.set_node_layout(&c.node, &node);
    ui.style(&c.node, &pxui::theme::States::of(Look { text: look.text, font: look.font, ..Look::NONE }), state);
    ui.text(&c.node, c.text_of(&c.text));
    if state.checked {
        let (own, strip) = (ui.rect(&c.node), ui.rect(&c.strip));
        let (at, w) = if own.w > 0.0 { ((own.x - strip.x, 0.0), own.w) } else { ((0.0, 0.0), 40.0) };
        let marker = Node { at: Some(at).into(), w: Size::Px(w), h: Size::Px(3.0), look: ink(ui), ..Node::DEFAULT };
        ui.within(&c.strip, &mut |ui: &mut Ui| { ui.node("marker", marker); });
    }
}

/// A table row with a hairline, the caller's cells inside.
fn ruled_row(ui: &mut Ui, c: &TableRowConfig) {
    let state = state_of(&c.state);
    let accent = ui.theme().tokens.accent;
    let look = if state.checked { Look { fill: accent, ..Look::NONE } } else { Look { border: accent, border_w: 1.0, ..Look::NONE } };
    ui.style(&c.row, &pxui::theme::States::of(look), state);
    ui.slot(c.content);
}

/// A search field as a 24-point well (the row the kit gives it): a 14-point round icon, the editing surface (the slot) and, with a query, a 20-point clear button.
fn boxed_search(ui: &mut Ui, c: &SearchFieldConfig) {
    let state = state_of(&c.state);
    let (look, hint, accent) = { let t = ui.theme(); (*t.search.pick(state), t.placeholder.idle, t.tokens.accent) };
    let mut well = ui.node_layout(&c.well);
    (well.h, well.axis, well.align, well.gap, well.padding) = (Size::Px(24.0), Axis::Row, Align::Center, 6.0, Edges { l: 6.0, ..Edges::ZERO });
    ui.set_node_layout(&c.well, &well);
    ui.style(&c.well, &pxui::theme::States::of(look), state);
    ui.node("icon", Node { w: Size::Px(14.0), h: Size::Px(14.0), look: Look { fill: accent, radius: [7.0; 4], ..Look::NONE }, ..Node::DEFAULT });
    if ui.slot(c.editor) {
        let clear = ui.part(Part::CLEAR, Node { flags: Flags::CLICKABLE | Flags::BLOCKING, w: Size::Px(20.0), h: Size::Px(20.0), look: Look { text: hint.text, ..Look::NONE }, ..Node::DEFAULT });
        ui.text(&clear, "x");
    }
}

/// A section index whose letters are 16-point pills down the strip, the active one filled with the accent.
fn pill_index(ui: &mut Ui, c: &SectionIndexConfig) {
    let (text, accent) = { let t = ui.theme(); (t.row.idle.text, t.tokens.accent) };
    for i in 0..c.letters.len {
        let active = c.has_active != 0 && c.active == i;
        let look = Look { fill: if active { accent } else { Look::NONE.fill }, text, radius: [8.0; 4], ..Look::NONE };
        let pill = ui.node(i as i64, Node { w: Size::Px(16.0), h: Size::Px(16.0), look, ..Node::DEFAULT });
        ui.text(&pill, c.text_at(&c.letters, i));
    }
}

/// A disclosure whose header row is a + (closed) or - (open) sign before its text; the content is built under it while open.
fn signed_section(ui: &mut Ui, c: &InspectorSectionConfig) {
    let text = ui.theme().row.idle.text;
    let open = c.open != 0;
    let mut row = ui.node_layout(&c.title);
    (row.axis, row.align) = (Axis::Row, Align::Center);
    ui.set_node_layout(&c.title, &row);
    ui.within(&c.title, &mut |ui: &mut Ui| {
        let sign = ui.node("sign", Node { w: Size::Px(16.0), h: Size::Px(16.0), look: Look { text, ..Look::NONE }, ..Node::DEFAULT });
        ui.text(&sign, if open { "-" } else { "+" });
        let label = ui.node("text", Node { h: Size::Px(16.0), look: Look { text, ..Look::NONE }, ..Node::DEFAULT });
        ui.text(&label, c.text_of(&c.label));
    });
    if open { ui.slot(c.content); }
}

/// A modal sheet whose bottom or full presentation (`present` 1 or 2) starts with a 36 by 4 point grab handle; the content follows.
fn handle_sheet(ui: &mut Ui, c: &ModalConfig) {
    if c.present != 0 {
        let ink = ui.theme().hint.idle.text;
        let row = ui.node("grab", Node { h: Size::Px(12.0), axis: Axis::Row, justify: pxui_types::layout::Justify::Center, align: Align::Center, ..Node::DEFAULT });
        ui.within(&row, &mut |ui: &mut Ui| { ui.node("handle", Node { w: Size::Px(36.0), h: Size::Px(4.0), look: Look { fill: ink, radius: [2.0; 4], ..Look::NONE }, ..Node::DEFAULT }); });
    }
    ui.slot(c.content);
}

/// Registers the proof bodies; `THEME_LINES` selects them. Once, before the theme that names them is applied.
pub fn register(ui: &mut Ui) -> Result<(), pxui::Error> {
    ui.register_toggle_body("checkbox", checkbox_toggle)?;
    ui.register_slider_body("round", round_slider)?;
    ui.register_segmented_body("sliding", sliding_segment)?;
    ui.register_table_row_body("ruled", ruled_row)?;
    ui.register_search_field_body("boxed", boxed_search)?;
    ui.register_section_index_body("pills", pill_index)?;
    ui.register_inspector_section_body("signed", signed_section)?;
    ui.register_modal_body("handle", handle_sheet)?;
    Ok(())
}

const NAMES: [&str; 3] = ["Wi-Fi", "Bluetooth", "Hotspot"];
const LETTERS: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
const PICKS: [pxui::PickRow; 4] = [
    pxui::PickRow::new("Rays").detail("09-24").art(pxui::PickArt::Glyph('\u{266b}')).heading("Songs"),
    pxui::PickRow::new("Dusk").detail("03-11").art(pxui::PickArt::Glyph('\u{266b}')),
    pxui::PickRow::new("Tides").art(pxui::PickArt::Glyph('\u{25a3}')).heading("Albums"),
    pxui::PickRow::new("Coast").art(pxui::PickArt::Glyph('\u{25a3}')),
];

/// The page: the widgets the proof bodies replace, as plain calls.
pub fn page(ui: &mut Ui, s: &mut BodiesPage) {
    ui.panel("bodies", &Overlay::DEFAULT.w_px(296.0), &mut |ui: &mut Ui| {
        ui.label("Bodies", pxui::Label::DEFAULT);
        let custom = ui.toggle("custom", "Custom bodies", s.custom, &pxui::Toggle::DEFAULT).changed != s.custom;
        if custom != s.custom { (s.custom, s.dirty) = (custom, true); }
        s.wifi = ui.toggle("wifi", "Wi-Fi", s.wifi, &pxui::Toggle::DEFAULT).changed != s.wifi;
        s.volume = ui.slider("volume", "Volume", s.volume, 0.0..=1.0, &pxui::Slider::DEFAULT).value;
        s.align = ui.segmented("align", &[pxui::Segment::new("Left"), pxui::Segment::new("Centre"), pxui::Segment::new("Right")], Some(s.align), &pxui::Segmented::DEFAULT.name("Align")).chosen.unwrap_or(s.align);
        if let Some(v) = ui.search_field("find", &s.query, &SearchField::DEFAULT.icon("?").placeholder("Search").name("Find")).changed { s.query = v.to_owned(); }
        let strip = ui.node("letters", Node { h: Size::Px(112.0), axis: Axis::Row, ..Node::DEFAULT });
        ui.within(&strip, &mut |ui: &mut Ui| { s.letter = ui.section_index("index", &LETTERS, &pxui::SectionIndex::DEFAULT.name("Jump to")).chosen.unwrap_or(s.letter); });
        s.sheet = ui.toggle("sheet", "Queue sheet", s.sheet, &pxui::Toggle::DEFAULT).changed != s.sheet;
        s.more = ui.disclosure("more", "More", s.more, &pxui::Disclosure::DEFAULT, &mut |ui: &mut Ui| { ui.label("Shown while open", pxui::Label::DEFAULT); }).changed != s.more;
    });
    ui.panel("pick", &Overlay::DEFAULT.at(pxui_types::geom::Point::new(332.0, 172.0)).w_px(296.0).window(), &mut |ui: &mut Ui| {
        ui.picker("find", "Search", "", &pxui::Picker::DEFAULT, &mut |_, rows: &mut pxui::PickRows| for row in PICKS { rows.push(row); });
    });
    if s.sheet && !ui.modal("queue", &Overlay::DEFAULT.name("Queue").present(pxui::Presentation::Bottom), &mut |ui: &mut Ui| { ui.label("Up next", pxui::Label::DEFAULT); }).open {
        s.sheet = false;
    }
    ui.panel("table", &Overlay::DEFAULT.at(pxui_types::geom::Point::new(332.0, 12.0)).w_px(296.0).window(), &mut |ui: &mut Ui| {
        let (widths, selected, flags) = (&mut s.widths, &mut s.selected, &mut s.flags);
        // the table fills its parent: give it a height
        let frame = ui.node("frame", Node { h: Size::Px(140.0), ..Node::DEFAULT });
        ui.within(&frame, &mut |ui: &mut Ui| {
            let _ = ui.table_view("settings", &["Name", "On"], widths, 3, &|i| i as i64, selected, &pxui::TableView::DEFAULT, &mut |ui: &mut Ui, row, column| {
                if column == 0 { ui.label(NAMES[row], pxui::Label::DEFAULT); } else { flags[row] = ui.toggle("toggle", NAMES[row], flags[row], &pxui::Toggle::DEFAULT).changed != flags[row]; }
            });
        });
    });
}
