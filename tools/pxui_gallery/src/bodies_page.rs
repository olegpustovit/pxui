//! The Bodies page: eleven widget bodies written against the public wrapper (`Ui::register_*_body`, `Ui::part`, `Ui::slot`) and picked by
//! a theme text (`THEME_LINES`: `toggle.body = checkbox`, ...). A checkbox-style toggle, a slider with a round thumb, a segmented control
//! whose marker slides, a table whose rows are ruled and whose cells hold widgets, a search field with an icon and a clear button, and a
//! section index whose letters are pills, the active one filled, a disclosure whose header is its text after a + or - sign, and a modal sheet with a grab handle when it is a bottom or full sheet.
//! It also shows tabs whose detail (a count) is a pill, checkboxes (plain, sized with the caller's look, passive and disabled) whose box
//! marks on with a dot, and a picker whose rows have group headings and glyph or image art drawn by a cover body (`PickArt::Image`: the
//! image the host gives `BodiesPage::art`, an accent tile until it does).
//! The page itself is plain widget calls: with the bodies not selected it draws the built-in widgets, with them the proof bodies.
//! Not in `PAGES`: it needs the wrapper (the pages there also compile against the engine for the A7 bench).
use pxui::bodies::{BodyConfig, CheckboxConfig, InspectorSectionConfig, ModalConfig, PickerConfig, SearchFieldConfig, SectionIndexConfig, SegmentedConfig, SliderConfig, TabsConfig, ToggleConfig, TableRowConfig};
use pxui::theme::{Look, State};
use pxui::{Image, ImageStyle, Overlay, Part, SearchField, Ui};
use std::cell::Cell;
use pxui_types::Flags;
use pxui_types::{geom::Edges, layout::Align, Axis, Node, Size};

/// The theme lines that select the proof bodies, on top of any base theme text.
pub const THEME_LINES: &str = "toggle.body = checkbox\nslider.body = round\nsegmented.body = sliding\ntable_row.body = ruled\nsearch_field.body = boxed\nsection_index.body = pills\ninspector_section.body = signed\nmodal.body = handle\ntabs.body = counted\ncheckbox.body = dotted\npicker.body = covers\n";

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
    pub table: pxui::TableState,
    pub flags: [bool; 3],
    pub letter: usize,
    pub more: bool,
    /// the bottom sheet is open
    pub sheet: bool,
    pub tab: usize,
    pub sync: bool,
    pub big: bool,
    /// the image the cover body draws for a `PickArt::Image` row (the host's; an accent tile while `None`)
    pub art: Option<Image>,
}

impl BodiesPage {
    /// The settings table's state: its second row selected.
    fn settings() -> pxui::TableState {
        let mut table = pxui::TableState::new();
        table.selection.rows = vec![0, pxui::ListState::SELECTED, 0];
        table
    }

    pub fn new(custom: bool) -> BodiesPage {
        BodiesPage { custom, dirty: false, wifi: true, volume: 0.6, align: 1, query: String::new(), table: BodiesPage::settings(), flags: [true, false, true], letter: 0, more: true, sheet: true, tab: 0, sync: true, big: false, art: None }
    }

    /// The theme text the host applies (`Ui::set_theme`): `base` (a theme file's text, or none), then the body lines when `custom`.
    pub fn theme_text(&self, base: &str) -> String {
        if self.custom { format!("{base}\n{THEME_LINES}") } else { base.to_owned() }
    }
}

fn ink(ui: &Ui) -> Look { Look::NONE.fill(ui.theme().tokens.accent) }

/// A toggle as a checkbox: a 12-point box at the row's start is the hit area, the label after it.
fn checkbox_toggle(ui: &mut Ui, c: &ToggleConfig) {
    let state = State::from(c.state);
    let (look, boxes) = { let t = ui.theme(); (*t.toggle.pick(state), t.checkbox) };
    ui.style(&c.row, &pxui::theme::States::of(look), state);
    ui.node_mut(&c.row, &mut |n| n.inset = Edges::ZERO.l(24.0));
    ui.text(&c.row, c.text_of(&c.label));
    let b = ui.part(Part::HIT, Node::DEFAULT.at(6.0, 6.0).w(Size::Px(12.0)).h(Size::Px(12.0)));
    ui.style(&b, &boxes, State::default().hovered(state.hovered).checked(c.on != 0));
}

/// A slider with a round 24-point thumb on a 4-point rail: behaviour maps the pointer across the rail, half the thumb kept at each end.
fn round_slider(ui: &mut Ui, c: &SliderConfig) {
    let state = State::from(c.state);
    let (rail, fill, thumb) = { let t = ui.theme(); (t.separator.idle, t.mark.idle, t.slider) };
    let fraction = c.fraction;
    ui.part(Part::RAIL, Node::DEFAULT.h(Size::Px(4.0)).look(rail));
    ui.node("fill", Node::DEFAULT.w(Size::Pct(fraction)).h(Size::Px(4.0)).look(fill));
    let lane = ui.node("lane", Node::DEFAULT.h(Size::Px(24.0)).axis(Axis::Row));
    ui.within(&lane, &mut |ui: &mut Ui| {
        ui.node("before", Node::DEFAULT.h(Size::Px(1.0)).grow(fraction));
        let th = ui.part(Part::THUMB, Node::DEFAULT.w(Size::Px(24.0)).h(Size::Px(24.0)).look(Look::NONE.radii([12.0; 4])));
        ui.style(&th, &thumb, state);
        ui.node("after", Node::DEFAULT.h(Size::Px(1.0)).grow(1.0 - fraction));
    });
}

/// Segments with no outline of their own and one marker, built by the chosen segment into the strip under one key, so it slides
/// between segments: placed from the chosen segment's rectangle last frame, it follows the choice a frame later.
fn sliding_segment(ui: &mut Ui, c: &SegmentedConfig) {
    let state = State::from(c.state);
    let look = *ui.theme().segment.pick(state);
    let mut node = ui.node_layout(&c.node);
    node.w = Size::Px(70.0);
    ui.set_node_layout(&c.node, &node);
    ui.style(&c.node, &pxui::theme::States::of(Look::NONE.text(look.text).font(look.font)), state);
    ui.text(&c.node, c.text_of(&c.text));
    if state.checked {
        let (own, strip) = (ui.rect(&c.node), ui.rect(&c.strip));
        let (at, w) = if own.w > 0.0 { ((own.x - strip.x, 0.0), own.w) } else { ((0.0, 0.0), 40.0) };
        let marker = Node::DEFAULT.at(at.0, at.1).w(Size::Px(w)).h(Size::Px(3.0)).look(ink(ui));
        ui.within(&c.strip, &mut |ui: &mut Ui| { ui.node("marker", marker); });
    }
}

/// A table row with a hairline, the caller's cells inside.
fn ruled_row(ui: &mut Ui, c: &TableRowConfig) {
    let state = State::from(c.state);
    let accent = ui.theme().tokens.accent;
    let look = if state.checked { Look::NONE.fill(accent) } else { Look::NONE.border(accent, 1.0) };
    ui.style(&c.row, &pxui::theme::States::of(look), state);
    ui.slot(c.content);
}

/// A search field as a 24-point well (the row the kit gives it): a 14-point round icon, the editing surface (the slot) and, with a query, a 20-point clear button.
fn boxed_search(ui: &mut Ui, c: &SearchFieldConfig) {
    let state = State::from(c.state);
    let (look, hint, accent) = { let t = ui.theme(); (*t.search.pick(state), t.placeholder.idle, t.tokens.accent) };
    let mut well = ui.node_layout(&c.well);
    (well.h, well.axis, well.align, well.gap, well.padding) = (Size::Px(24.0), Axis::Row, Align::Center, 6.0, Edges::ZERO.l(6.0));
    ui.set_node_layout(&c.well, &well);
    ui.style(&c.well, &pxui::theme::States::of(look), state);
    ui.node("icon", Node::DEFAULT.w(Size::Px(14.0)).h(Size::Px(14.0)).look(Look::NONE.fill(accent).radii([7.0; 4])));
    if ui.slot(c.editor) {
        let clear = ui.part(Part::CLEAR, Node::DEFAULT.flags(Flags::CLICKABLE | Flags::BLOCKING).w(Size::Px(20.0)).h(Size::Px(20.0)).look(Look::NONE.text(hint.text)));
        ui.text(&clear, "x");
    }
}

/// A section index whose letters are 16-point pills down the strip, the active one filled with the accent.
fn pill_index(ui: &mut Ui, c: &SectionIndexConfig) {
    let (text, accent) = { let t = ui.theme(); (t.row.idle.text, t.tokens.accent) };
    for i in 0..c.letters.len {
        let active = c.has_active != 0 && c.active == i;
        let look = Look::NONE.fill(if active { accent } else { Look::NONE.fill }).text(text).radii([8.0; 4]);
        let pill = ui.node(i as i64, Node::DEFAULT.w(Size::Px(16.0)).h(Size::Px(16.0)).look(look));
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
        let sign = ui.node("sign", Node::DEFAULT.w(Size::Px(16.0)).h(Size::Px(16.0)).look(Look::NONE.text(text)));
        ui.text(&sign, if open { "-" } else { "+" });
        let label = ui.node("text", Node::DEFAULT.h(Size::Px(16.0)).look(Look::NONE.text(text)));
        ui.text(&label, c.text_of(&c.label));
    });
    if open { ui.slot(c.content); }
}

/// A modal sheet whose bottom or full presentation (`present` 1 or 2) starts with a 36 by 4 point grab handle; the content follows.
fn handle_sheet(ui: &mut Ui, c: &ModalConfig) {
    if c.present != 0 {
        let ink = ui.theme().hint.idle.text;
        let row = ui.node("grab", Node::DEFAULT.h(Size::Px(12.0)).axis(Axis::Row).justify(pxui_types::layout::Justify::Center).align(Align::Center));
        ui.within(&row, &mut |ui: &mut Ui| { ui.node("handle", Node::DEFAULT.w(Size::Px(36.0)).h(Size::Px(4.0)).look(Look::NONE.fill(ink).radii([2.0; 4]))); });
    }
    ui.slot(c.content);
}

/// A tab whose detail (a count) follows its text in an accent pill.
fn counted_tab(ui: &mut Ui, c: &TabsConfig) {
    let state = State::from(c.state);
    let (look, accent, panel) = { let t = ui.theme(); (*t.tab.pick(state), t.tokens.accent, t.tokens.panel) };
    let mut node = ui.node_layout(&c.node);
    (node.w, node.axis, node.align, node.gap) = (Size::Fit, Axis::Row, Align::Center, 4.0);
    ui.set_node_layout(&c.node, &node);
    ui.style(&c.node, &pxui::theme::States::of(look), state);
    ui.within(&c.node, &mut |ui: &mut Ui| {
        let text = ui.node("text", Node::DEFAULT.w(Size::Fit).h(Size::Px(16.0)).look(Look::NONE.text(look.text).font(look.font)));
        ui.text(&text, c.text_of(&c.text));
        let detail = c.text_of(&c.detail);
        if !detail.is_empty() {
            let pill = ui.node("detail", Node::DEFAULT.w(Size::Fit).h(Size::Px(14.0)).padding(Edges::ZERO.l(4.0).r(4.0)).look(Look::NONE.fill(accent).text(panel).radii([4.0; 4])));
            ui.text(&pill, detail);
        }
    });
}

/// A checkbox whose box (the caller's side and looks, or the theme's) marks on with a round dot; a passive one with no text is the box alone.
fn dotted_checkbox(ui: &mut Ui, c: &CheckboxConfig) {
    let state = State::from(c.state);
    let (themed, text, accent) = { let t = ui.theme(); (t.checkbox, t.toggle.pick(state).text, t.tokens.accent) };
    let states = c.states_of(&c.states).copied().unwrap_or(themed);
    let side = c.box_size;
    let b = ui.node("box", Node::DEFAULT.w(Size::Px(side)).h(Size::Px(side)).axis(Axis::Row).justify(pxui_types::layout::Justify::Center).align(Align::Center));
    ui.style(&b, &states, state.checked(c.on != 0));
    if c.on != 0 {
        let dot = Node::DEFAULT.w(Size::Px(side / 2.0)).h(Size::Px(side / 2.0)).look(Look::NONE.fill(accent).radii([side / 4.0; 4]));
        ui.within(&b, &mut |ui: &mut Ui| { ui.node("dot", dot); });
    }
    let label = c.text_of(&c.label);
    if !(c.passive != 0 && label.is_empty()) {
        let n = ui.node("label", Node::DEFAULT.w(Size::Fit).h(Size::Px(16.0)).look(Look::NONE.text(text)));
        ui.text(&n, label);
    }
}

thread_local! {
    /// The image `page` was given (`BodiesPage::art`), which the cover body draws for an image row: a body is a plain `fn`, so the app's
    /// image cache is where it looks its art up.
    static COVER: Cell<Option<Image>> = const { Cell::new(None) };
}

/// A picker row whose art is a 20-point cover (`PickArt::Image`: the app's image for the id, an accent tile until there is one) or its
/// glyph, then its label and, at the end, its detail; a group heading above it in the hint look, a row high, the spacing scale's third step in.
fn cover_row(ui: &mut Ui, c: &PickerConfig) {
    let state = State::from(c.state);
    let (look, hint, accent, heading, inset) = { let t = ui.theme(); (*t.list.pick(state), t.hint.idle.text, t.tokens.accent, t.hint.idle, (t.tokens.space[2] * t.tokens.density).round()) };
    if c.has_head == 1 {
        ui.style(&c.head, &pxui::theme::States::of(heading), pxui::theme::State::default());
        let row_height = ui.row_height();
        ui.set_node_layout(&c.head, &ui.node_layout(&c.head).w(Size::Grow).h(Size::Px(row_height)).margin(Edges::ZERO.l(inset)));
    }
    ui.style(&c.row, &pxui::theme::States::of(look), state);
    let mut row = ui.node_layout(&c.row);
    (row.axis, row.align, row.gap, row.padding) = (Axis::Row, Align::Center, 6.0, Edges::ZERO.l(8.0).r(8.0));
    ui.set_node_layout(&c.row, &row);
    const COVER_STYLE: ImageStyle = ImageStyle::DEFAULT.w_px(20.0).h_px(20.0).fit(pxui::draw::Fit::Cover);
    let tile = Node::DEFAULT.w(Size::Px(20.0)).h(Size::Px(20.0)).look(Look::NONE.fill(accent).radii([3.0; 4]));
    match (c.art_kind, COVER.with(Cell::get)) {
        (2, Some(image)) => { let _ = ui.image("art", c.text_of(&c.label), &image, &COVER_STYLE); }
        (2, None) => { ui.node("art", tile); }
        (1, _) => {
            let mut buf = [0u8; 4];
            let glyph = ui.node("art", Node::DEFAULT.w(Size::Px(20.0)).h(Size::Px(20.0)).look(Look::NONE.text(look.text)));
            ui.text(&glyph, char::from_u32(c.art as u32).unwrap_or(' ').encode_utf8(&mut buf));
        }
        _ => (),
    }
    let label = ui.node("label", Node::DEFAULT.w(Size::Fit).h(Size::Px(16.0)).grow(1.0).look(Look::NONE.text(look.text).font(look.font)));
    ui.text(&label, c.text_of(&c.label));
    let detail = c.text_of(&c.detail);
    if !detail.is_empty() {
        let n = ui.node("detail", Node::DEFAULT.w(Size::Fit).h(Size::Px(16.0)).look(Look::NONE.text(hint)));
        ui.text(&n, detail);
    }
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
    ui.register_tabs_body("counted", counted_tab)?;
    ui.register_checkbox_body("dotted", dotted_checkbox)?;
    ui.register_picker_body("covers", cover_row)?;
    Ok(())
}

const NAMES: [&str; 3] = ["Wi-Fi", "Bluetooth", "Hotspot"];
const LETTERS: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
const TABS: [pxui::Segment; 3] = [pxui::Segment::new("Songs").detail("12"), pxui::Segment::new("Albums").detail("3"), pxui::Segment::new("Mixes")];
/// The checkbox's own look (`Checkbox::look`): an accent outline in every state.
const BOX_LOOK: pxui::theme::States = pxui::theme::States::of(Look::NONE.border(pxui_types::geom::Rgba::rgba(0x4a, 0x90, 0xd9, 255), 1.0).radii([3.0; 4]));
const PICKS: [pxui::PickRow; 5] = [
    pxui::PickRow::new("Rays").detail("09-24").art(pxui::PickArt::Glyph('\u{266b}')).heading("Songs"),
    pxui::PickRow::new("Dusk").detail("03-11").art(pxui::PickArt::Glyph('\u{266b}')),
    pxui::PickRow::new("Tides").art(pxui::PickArt::Glyph('\u{25a3}')).heading("Albums"),
    pxui::PickRow::new("Coast").art(pxui::PickArt::Glyph('\u{25a3}')),
    pxui::PickRow::new("Shore").detail("cover").art(pxui::PickArt::Image(1)),
];

/// The page: the widgets the proof bodies replace, as plain calls.
pub fn page(ui: &mut Ui, s: &mut BodiesPage) {
    COVER.with(|cover| cover.set(s.art));
    ui.panel("bodies", &Overlay::DEFAULT.w_px(296.0), &mut |ui: &mut Ui| {
        ui.label("Bodies", pxui::Label::DEFAULT);
        let custom = ui.toggle("custom", "Custom bodies", s.custom, &pxui::Toggle::DEFAULT).changed != s.custom;
        if custom != s.custom { (s.custom, s.dirty) = (custom, true); }
        s.wifi = ui.toggle("wifi", "Wi-Fi", s.wifi, &pxui::Toggle::DEFAULT).changed != s.wifi;
        s.volume = ui.slider("volume", "Volume", s.volume, 0.0..=1.0, &pxui::Slider::DEFAULT).value;
        s.align = ui.segmented("align", &[pxui::Segment::new("Left"), pxui::Segment::new("Centre"), pxui::Segment::new("Right")], Some(s.align), &pxui::Segmented::DEFAULT.name("Align")).chosen.unwrap_or(s.align);
        s.tab = ui.tabs("views", &TABS, Some(s.tab), &pxui::Tabs::DEFAULT.name("Views")).chosen.unwrap_or(s.tab);
        let boxes = ui.node("boxes", Node::DEFAULT.h(Size::Fit).axis(Axis::Row).gap(4.0));
        ui.within(&boxes, &mut |ui: &mut Ui| {
            s.sync = s.sync != ui.checkbox("sync", "Sync", s.sync, &pxui::Checkbox::DEFAULT).clicked;
            s.big = s.big != ui.checkbox("big", "Big", s.big, &pxui::Checkbox::DEFAULT.size(16.0).look(&BOX_LOOK)).clicked;
            ui.checkbox("seen", "", true, &pxui::Checkbox::DEFAULT.passive(true));
            ui.checkbox("locked", "Locked", true, &pxui::Checkbox::DEFAULT.disabled(true));
        });
        if let Some(v) = ui.search_field("find", &s.query, &SearchField::DEFAULT.icon("?").placeholder("Search").name("Find")).changed { s.query = v.to_owned(); }
        let strip = ui.node("letters", Node::DEFAULT.h(Size::Px(112.0)).axis(Axis::Row));
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
        const SETTINGS: [pxui::Column; 2] = [pxui::Column::new("Name").w_px(120.0), pxui::Column::new("On").w_px(120.0)];
        let (table, flags) = (&mut s.table, &mut s.flags);
        // the table fills its parent: give it a height
        let frame = ui.node("frame", Node::DEFAULT.h(Size::Px(140.0)));
        ui.within(&frame, &mut |ui: &mut Ui| {
            let _ = ui.table("settings", &SETTINGS, 3, &|i| i as i64, table, &pxui::Table::DEFAULT, &mut |ui: &mut Ui, row, column, _| {
                if column == 0 { ui.label(NAMES[row], pxui::Label::DEFAULT); } else { flags[row] = ui.toggle("toggle", NAMES[row], flags[row], &pxui::Toggle::DEFAULT).changed != flags[row]; }
            });
        });
    });
}
