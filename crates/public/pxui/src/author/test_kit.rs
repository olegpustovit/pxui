//! Tests of the checkbox style, the disclosure and the section index.
use pxui_types::theme::{Look, States};
use pxui_types::ui::Checkbox;
use crate::bodies::{BodyConfig, CheckboxConfig, InspectorSectionConfig, ModalConfig, SectionIndexConfig, TabsConfig};
use crate::ui::{Node, Role, Size};
use crate::{testing, Disclosure, Event, FrameIn, Overlay, Presentation, SectionIndex, Segment, Tabs, Ui};
use std::cell::{Cell, RefCell};

thread_local! { static SEEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) }; }
const LOOK: States = States::of(Look::NONE);
fn seen(what: String) { SEEN.with(|v| v.borrow_mut().push(what)); }
fn marker(ui: &mut Ui, kind: &'static str) { let n = ui.node(kind, Node { w: Size::Px(10.0), h: Size::Px(10.0), role: Role::Image, ..Node::DEFAULT }); ui.a11y_name(&n, kind); }
fn check(ui: &mut Ui, c: &CheckboxConfig) {
    seen(format!("checkbox {} {} {} {}", c.variant, c.box_size, c.passive, c.states_of(&c.states) == Some(&LOOK)));
    marker(ui, "custom checkbox");
}
fn section(ui: &mut Ui, c: &InspectorSectionConfig) { seen(format!("disclosure {}", c.variant)); marker(ui, "custom section"); }
fn tab(ui: &mut Ui, c: &TabsConfig) { seen(format!("tab {} {}", c.text_of(&c.text), c.text_of(&c.detail))); marker(ui, "custom tab"); }
fn modal(ui: &mut Ui, c: &ModalConfig) { seen(format!("modal {}", c.present)); marker(ui, "custom modal"); }
fn index(ui: &mut Ui, c: &SectionIndexConfig) {
    seen(format!("index {} {}{} {}", c.letters.len, c.text_at(&c.letters, 0), c.text_at(&c.letters, 2), c.variant));
    marker(ui, "custom index");
}

/// A styled checkbox, a disclosure, tabs, a bottom sheet and a section index are built through their registered bodies (the checkbox's,
/// `inspector_section`'s, `tabs`', `modal`'s and `section_index`'s), each given what the built-in body honours: the checkbox's variant,
/// box side, passive mark and looks, the disclosure's variant, a tab's detail, the sheet's presentation and the index's letters and
/// variant.
#[test]
fn the_new_widgets_are_built_through_their_registered_bodies() {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    ui.set_a11y(true);
    ui.register_checkbox_body("c", check).unwrap();
    ui.register_inspector_section_body("c", section).unwrap();
    ui.register_tabs_body("c", tab).unwrap();
    ui.register_modal_body("c", modal).unwrap();
    ui.register_section_index_body("c", index).unwrap();
    ui.set_theme(&crate::ThemeHandle::parse("checkbox.body = c\ninspector_section.body = c\ntabs.body = c\nmodal.body = c\nsection_index.body = c").unwrap()).unwrap();
    SEEN.with(|v| v.borrow_mut().clear());
    let mut build = |ui: &mut Ui| {
        ui.checkbox("box", "Box", true, &Checkbox::DEFAULT.size(18.0).look(&LOOK).passive(true).variant(3));
        ui.disclosure("more", "More", true, &Disclosure::DEFAULT.variant(4), &mut |_| {});
        ui.tabs("tabs", &[Segment::new("Songs").detail("12")], Some(0), &Tabs::DEFAULT);
        ui.section_index("index", &["A", "B", "C"], &SectionIndex::DEFAULT.variant(5));
        ui.modal("sheet", &Overlay::DEFAULT.present(Presentation::Bottom), |_| {});
    };
    ui.frame(&FrameIn::new(320.0, 240.0), &mut build).unwrap();
    assert_eq!(SEEN.with(|v| v.borrow().clone()), ["checkbox 3 18 1 true", "disclosure 4", "tab Songs 12", "index 3 AC 5", "modal 1"]);
    for name in ["custom checkbox", "custom section", "custom tab", "custom index", "custom modal"] { assert!(testing::find(&ui, Role::Image, name).is_some(), "{name}"); }
}

/// A passive checkbox takes no press (the row under it does); a disabled one reports no click; an enabled one flips.
#[test]
fn passive_and_disabled_checkboxes_take_no_input() {
    let (passive, disabled, enabled, row) = (Cell::new(false), Cell::new(false), Cell::new(false), Cell::new(false));
    let mut build = |ui: &mut Ui| {
        let r = ui.node("row", Node { flags: crate::ui::Flags::CLICKABLE, w: Size::Px(300.0), h: Size::Px(24.0), axis: crate::ui::Axis::Row, ..Node::DEFAULT });
        ui.within(&r, &mut |ui| {
            passive.set(passive.get() | ui.checkbox("p", "", false, &Checkbox::DEFAULT.passive(true)).clicked);
        });
        row.set(row.get() | ui.signal(&r).clicked);
        disabled.set(disabled.get() | ui.checkbox("d", "Off", false, &Checkbox::DEFAULT.disabled(true)).clicked);
        enabled.set(enabled.get() | ui.checkbox("e", "On", false, &Checkbox::DEFAULT).changed);
    };
    let mut ui = crate::doc::frames(1, &mut build);
    let (d, e) = (testing::find(&ui, Role::CheckBox, "Off").expect("disabled row"), testing::find(&ui, Role::CheckBox, "On").expect("enabled row"));
    testing::click(&mut ui, 5.0, 12.0, &mut build);
    assert!(!passive.get(), "no press of its own");
    assert!(row.get(), "the row under it takes the press");
    testing::click(&mut ui, f64::from(d.x + 4.0), f64::from(d.y + 4.0), &mut build);
    assert!(!disabled.get());
    testing::click(&mut ui, f64::from(e.x + 4.0), f64::from(e.y + 4.0), &mut build);
    assert!(enabled.get());
}

/// The disclosure runs its body only while open, and a press on the header is `changed`, the caller flipping the value it keeps.
#[test]
fn a_disclosure_toggles_and_runs_its_body_only_while_open() {
    let (open, ran) = (Cell::new(false), Cell::new(0));
    let mut build = |ui: &mut Ui| { if ui.disclosure("more", "More", open.get(), &Disclosure::DEFAULT, &mut |_| ran.set(ran.get() + 1)).changed { open.set(!open.get()); } };
    let mut ui = crate::doc::frames(1, &mut build);
    assert_eq!((open.get(), ran.get()), (false, 0));
    let head = testing::find(&ui, Role::Heading, "More").expect("header");
    testing::click(&mut ui, f64::from(head.x + 4.0), f64::from(head.y + 4.0), &mut build);
    testing::frame(&mut ui, &[], &mut build);
    assert!(open.get());
    assert!(ran.get() > 0);
}

/// A press and drag on the strip chooses the letter under the pointer; with nothing held there is no choice.
#[test]
fn the_section_index_reports_the_letter_under_the_pointer() {
    let got = Cell::new(None);
    let letters = ["A", "B", "C", "D"];
    let mut build = |ui: &mut Ui| {
        let column = ui.node("column", Node { w: Size::Px(40.0), h: Size::Px(200.0), ..Node::DEFAULT });
        ui.within(&column, &mut |ui| { let r = ui.section_index("index", &letters, &SectionIndex::DEFAULT); assert_eq!(r.response.changed, r.chosen.is_some()); got.set(r.chosen); });
    };
    let mut ui = crate::doc::frames(2, &mut build);
    assert_eq!(got.get(), None);
    let strip = testing::find(&ui, Role::Slider, "").expect("the strip");
    let at = |i: f32| strip.y + strip.h * (i + 0.5) / 4.0;
    testing::frame(&mut ui, &[Event { kind: Event::POINTER_PRESSED, x: f64::from(strip.x + 4.0), y: f64::from(at(2.0)), ..Default::default() }], &mut build);
    assert_eq!(got.get(), Some(2));
    testing::frame(&mut ui, &[Event { kind: Event::POINTER_MOVED, x: f64::from(strip.x + 4.0), y: f64::from(at(3.0)), ..Default::default() }], &mut build);
    assert_eq!(got.get(), Some(3));
    testing::frame(&mut ui, &[Event { kind: Event::POINTER_RELEASED, x: f64::from(strip.x + 4.0), y: f64::from(at(3.0)), ..Default::default() }], &mut build);
    assert_eq!(got.get(), Some(3), "the release frame reports the final letter");
    testing::frame(&mut ui, &[], &mut build);
    assert_eq!(got.get(), None);
}

/// What a screen reader is told: the checkbox is a checkable CheckBox with its checked state (a disabled one not enabled), the disclosure
/// header an expandable Heading with its expanded state, and the section index a focusable Slider named by its style over the letters,
/// its value the active letter.
#[test]
fn the_new_widgets_report_their_role_and_state() {
    use pxui_types::a11y::state::{CHECKABLE, CHECKED, ENABLED, EXPANDABLE, EXPANDED, FOCUSABLE, READ_ONLY};
    let open = Cell::new(true);
    let mut build = |ui: &mut Ui| {
        ui.checkbox("on", "On", true, &Checkbox::DEFAULT);
        ui.checkbox("off", "Off", false, &Checkbox::DEFAULT.disabled(true));
        ui.checkbox("seen", "Seen", true, &Checkbox::DEFAULT.passive(true));
        ui.disclosure("more", "More", open.get(), &Disclosure::DEFAULT, &mut |_| {});
        let column = ui.node("column", Node { w: Size::Px(40.0), h: Size::Px(120.0), ..Node::DEFAULT });
        ui.within(&column, &mut |ui| { ui.section_index("index", &["A", "B", "C"], &SectionIndex::DEFAULT.name("Jump to")); });
    };
    let mut ui = crate::doc::frames(2, &mut build);
    let states = |ui: &Ui, role: Role, name: &str| ui.a11y_nodes().iter().find(|n| n.is_role(role) && ui.a11y_name_of(n) == name).map(|n| n.states).expect(name);
    let on = states(&ui, Role::CheckBox, "On");
    assert!(on & CHECKABLE != 0 && on & CHECKED != 0 && on & ENABLED != 0 && on & READ_ONLY == 0, "a checked checkbox");
    let seen = states(&ui, Role::CheckBox, "Seen");
    assert!(seen & CHECKABLE != 0 && seen & CHECKED != 0 && seen & READ_ONLY != 0, "a passive checkbox reports its state read-only");
    let off = states(&ui, Role::CheckBox, "Off");
    assert!(off & CHECKED == 0 && off & ENABLED == 0, "an unchecked disabled checkbox");
    let head = states(&ui, Role::Heading, "More");
    assert!(head & EXPANDABLE != 0 && head & EXPANDED != 0, "an open disclosure");
    open.set(false);
    testing::frame(&mut ui, &[], &mut build);
    assert!(states(&ui, Role::Heading, "More") & EXPANDED == 0, "a closed disclosure");
    let strip = ui.a11y_nodes().iter().find(|n| n.is_role(Role::Slider) && ui.a11y_name_of(n) == "Jump to").expect("the strip");
    assert!(strip.states & FOCUSABLE != 0 && strip.states & ENABLED != 0, "a focusable strip");
    assert_eq!((strip.value.min, strip.value.max, strip.value.step), (0.0, 2.0, 1.0), "a slider over the three letters");
}

/// Warm frames of all three widgets allocate nothing.
#[test]
fn the_new_widgets_allocate_nothing_once_warm() {
    let mut ui = Ui::new(&crate::UiDesc::DEFAULT).expect("library");
    const LOOK: States = States::of(Look::NONE);
    let mut build = |ui: &mut Ui| {
        ui.checkbox("a", "A", true, &Checkbox::DEFAULT.size(14.0).look(&LOOK).passive(true));
        ui.checkbox("b", "B", false, &Checkbox::DEFAULT.disabled(true));
        ui.section_index("index", &["A", "B", "C"], &SectionIndex::DEFAULT.name("Letters"));
        ui.disclosure("more", "More", true, &Disclosure::DEFAULT, &mut |ui| { ui.label("inside", crate::Label::BODY); });
    };
    let input = FrameIn::new(320.0, 240.0);
    for _ in 0..4 { ui.frame(&input, &mut build).expect("frame"); }
    mem_sys::local_reset();
    for _ in 0..4 { ui.frame(&input, &mut build).expect("frame"); }
    assert_eq!(mem_sys::local_alloc_count(), 0);
}
