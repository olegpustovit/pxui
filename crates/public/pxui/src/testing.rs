//! A headless frame driver, input helpers and accessibility lookup for examples and tests.
//! Every helper takes the `Ui` and the build closure, because an event reaches the app through the next frame's build.
use crate::geom::Rect;
use crate::ui::Role;
use crate::{Event, FrameIn, Handle, Ui};

/// Runs one 320 x 240 frame of `build` with `events` as that frame's input.
///
/// Events are in points, window space. Allocates: as any frame. Panics: when the frame fails.
///
/// ```
/// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
/// pxui::testing::frame(&mut ui, &[], &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
/// assert_eq!(ui.frame_index(), 2);
/// ```
pub fn frame(ui: &mut Ui, events: &[Event], build: &mut dyn FnMut(&mut Ui)) {
    let mut input = FrameIn::new(320.0, 240.0);
    input.wire.events = events.as_ptr();
    input.wire.event_count = events.len();
    ui.frame(&input, build).expect("pxui: a testing frame failed");
}

/// Clicks the point (`x`, `y`) in window points: a press frame, then a release frame.
///
/// Allocates: as two frames. Panics: when a frame fails.
///
/// ```
/// use pxui::ui::{Flags, Node, Size, Role};
/// let mut clicked = false;
/// let mut build = |ui: &mut pxui::Ui| {
///     let b = ui.node("b".into(), Node { flags: Flags::CLICKABLE, role: Role::Button, w: Size::Px(50.0), h: Size::Px(20.0), ..Node::DEFAULT });
///     clicked |= ui.signal(&b).clicked;
/// };
/// let mut ui = pxui::doc::frames(1, &mut build);
/// pxui::testing::click(&mut ui, 10.0, 10.0, &mut build);
/// assert!(clicked);
/// ```
pub fn click(ui: &mut Ui, x: f64, y: f64, build: &mut dyn FnMut(&mut Ui)) {
    for kind in [Event::POINTER_PRESSED, Event::POINTER_RELEASED] {
        frame(ui, &[Event { kind, x, y, ..Default::default() }], build);
    }
}

/// Clicks the center of `node` as laid out by the last frame.
///
/// Allocates: as two frames. Panics: when a frame fails.
///
/// ```
/// use pxui::ui::{Flags, Node, Size, Role};
/// use std::cell::Cell;
/// let (clicked, node) = (Cell::new(false), Cell::new(None));
/// let mut build = |ui: &mut pxui::Ui| {
///     let b = ui.node("b".into(), Node { flags: Flags::CLICKABLE, role: Role::Button, w: Size::Px(50.0), h: Size::Px(20.0), ..Node::DEFAULT });
///     clicked.set(clicked.get() | ui.signal(&b).clicked);
///     node.set(Some(b));
/// };
/// let mut ui = pxui::doc::frames(1, &mut build);
/// pxui::testing::click_node(&mut ui, &node.get().unwrap(), &mut build);
/// assert!(clicked.get());
/// ```
pub fn click_node(ui: &mut Ui, node: &Handle, build: &mut dyn FnMut(&mut Ui)) {
    let r = ui.rect(node);
    click(ui, (r.x + r.w / 2.0) as f64, (r.y + r.h / 2.0) as f64, build);
}

/// Presses and releases the key `code` (a Unicode scalar value, or `KEY_SPECIAL` plus an index) in one frame.
///
/// Allocates: as one frame. Panics: when the frame fails.
///
/// ```
/// let mut ui = pxui::doc::frames(1, &mut |ui| { ui.label("x", pxui::Label::DEFAULT); });
/// pxui::testing::key(&mut ui, 'a' as u32, &mut |ui| { let _ = ui.key_pressed('a' as u32, None); });
/// ```
pub fn key(ui: &mut Ui, code: u32, build: &mut dyn FnMut(&mut Ui)) {
    let events = [Event { kind: Event::KEY_PRESSED, code, ..Default::default() }, Event { kind: Event::KEY_RELEASED, code, ..Default::default() }];
    frame(ui, &events, build);
}

/// The rectangle, in window points, of the first accessibility node of the last frame with `role` and `name`.
///
/// `None` when no node matches, or accessibility is off (`doc::frames` turns it on). Allocates: never. Panics: never.
///
/// ```
/// use pxui::ui::{Node, Size, Role};
/// let ui = pxui::doc::frames(1, &mut |ui| {
///     let n = ui.node("n".into(), Node { role: Role::Button, w: Size::Px(50.0), h: Size::Px(20.0), ..Node::DEFAULT });
///     ui.a11y_name(&n, "Save");
/// });
/// let r = pxui::testing::find(&ui, Role::Button, "Save").unwrap();
/// assert_eq!((r.w, r.h), (50.0, 20.0));
/// assert!(pxui::testing::find(&ui, Role::Button, "Load").is_none());
/// ```
pub fn find(ui: &Ui, role: Role, name: &str) -> Option<Rect> {
    ui.a11y_nodes().iter().find(|n| n.is_role(role) && ui.a11y_name_of(n) == name).map(|n| n.rect)
}
