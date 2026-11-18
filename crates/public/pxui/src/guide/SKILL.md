---
name: pxui-app
description: Write pxui application UI code in the current public API shape (const style values, keyed interactive widgets, zero warm allocation). Use when adding or changing screens, rows, buttons, menus, dialogs or labels in an app built on pxui.
---

# Writing a pxui app

Every Rust block below is compiled and run by `cargo t -p pxui --doc` (it is the `guide` module's doctest set), so
a sample that stops matching the API fails CI. The code is the reference for signatures: read the `///` comment of
the function you call (what it does, `Allocates:`, `Identity:`, `Panics:`, example).

## The shape

- Style and option types (`Label`, `Button`, `IconButton`, `Toggle`, `Segmented`, `Menu`, `MenuItem`, `Dialog`,
  `DialogButton`, `Segment`, `Node`) are `Copy` values with a `DEFAULT` (labels also `BODY`, `CAPTION`, `TITLE`, `MONO`)
  and `const fn` builders that take `self` by value. Declare them as `const` items; never write a struct literal with
  `..Default::default()` at a call site, and never build a style per frame from data.
- One function per job. `label`, `label_fmt`, `label_digits`; `button_with` (text) and `button_content` (your own
  contents); `icon_button`; `toggle_with`; `segmented`; `menu`; `dialog`; `pressable`; `list`; `view_of`;
  `row`, `column`, `stack`. The short `button(text)` and `toggle(text, on)` exist for prototypes and key by position.
- Labels are leaves: no key, identity by position among siblings, no state. Everything interactive takes a key first.

## Keys

A key is a `&'static str` or an item id (`i64`/`u64`). Never the display text: text changes (translation, a
counter, a song title) would reset the widget's hover, press, focus and animation state, and two equal texts
collide. A list row is keyed by its model id, so reordering moves its state with it.

## Rows, labels and pressables

```rust
use pxui::{theme::ColorToken, ui::{Node, Look, Role, Size, States}, Label, Pressable};

const ROW: Node = Node::row().h(Size::Px(44.0)).role(Role::Button);
const LIST_ROW: States = States::all(Look::NONE); // the app's theme look per state
const TITLE: Label = Label::BODY.size(15.0).bold();
const DIM: Label = Label::BODY.color(ColorToken(3));

struct Song { id: u64, title: String, duration: String }

let songs = [Song { id: 7, title: "Blue in Green".into(), duration: "5:37".into() }];
let mut played = None;
let mut build = |ui: &mut pxui::Ui| {
    for song in &songs {
        let row = ui.pressable(song.id, ROW, &Pressable::DEFAULT.look(&LIST_ROW), |ui, _state| {
            ui.label(&song.title, TITLE.grow());
            ui.label(&song.duration, DIM);
        });
        if row.clicked { played = Some(song.id); }
    }
};
let mut ui = pxui::doc::frames(1, &mut build);
pxui::testing::click(&mut ui, 20.0, 20.0, &mut build);
assert_eq!(played, Some(7));
```

`pressable` looks the row by its state (`Pressable::look`, `selected`, `enabled`, `tab_stop`), gives the
contents that state, and returns a `Response` (`clicked`, `hovered`, `held`, `focused`, `secondary` for a
context menu, `handle`; take a gesture with `ui.gesture(&row.handle, ..)`). The app reads it and
records an intent; it does not mutate the model inside the build.

## No formatting and no Vec per frame

A warm frame allocates nothing (the test suite counts). So:

- `ui.label(&text, style)` for text you already own (a cached `String` on the model).
- `ui.label_fmt(format_args!(..), style)` formats into a 128-byte stack buffer for short, changing text.
- `ui.label_digits(format_args!(..), style)` is for clocks and counters: pinned digit glyphs, no shaping.
- Never `format!`, `to_string`, `collect` or `Vec::new` inside a build or a `view_of`/`list` closure. Make the string
  when the data changes, on the model.

```rust
use pxui::Label;
const CLOCK: Label = Label::MONO;
let (m, s, index, total) = (3, 7, 4, 120);
pxui::doc::frames(2, &mut |ui| {
    ui.label_fmt(format_args!("{} of {}", index + 1, total), Label::CAPTION);
    ui.label_digits(format_args!("{m}:{s:02}"), CLOCK);
});
```

## Buttons

`button_with(key, text, &Button)` for text; `button_content(key, &Button, |ui, state| ..)` when you draw the
contents. `state` is a by-value snapshot (`hovered`, `pressed`, `focused`, `disabled`, `checked`) read before the closure
runs. Give a content button a size and a `name` (its accessible name). `icon_button(key, Icon, name, &IconButton)`
is the glyph or drawn-icon button; `name` is the accessible name. `Ui::disabled(flag, |ui| ..)` disables every
interactive widget inside.

```rust
use pxui::{ui::{Node, Size}, Button, Icon, IconButton};
const SAVE: Button = Button::sized(90.0, 30.0);
const DELETE: Button = Button::sized(120.0, 30.0).danger();
const KEY: IconButton = IconButton::sized(36.0, 36.0).press_scale(0.96);
let mut play = false;
pxui::doc::frames(2, &mut |ui| {
    ui.button_with("save", "Save", &SAVE);
    ui.disabled(!play, |ui| { ui.button_with("clear", "Clear offline cache", &DELETE); });
    if ui.icon_button("play", Icon::Glyph('>'), "Play", &KEY).clicked { play = true; }
    let pill = Button::sized(90.0, 30.0).name("Shuffle");
    ui.button_content("shuffle", &pill, |ui, state| {
        let side = if state.pressed { 14.0 } else { 16.0 };
        ui.node("icon", Node { w: Size::Px(side), h: Size::Px(side), ..Node::DEFAULT });
    });
});
```

## Toggles and segmented controls

`toggle_with(key, text, on, &Toggle)` returns a `Response`: the new value is `on != response.changed`; the app owns
the value. `segmented(key, &[Segment], selected, &Segmented)` takes the choice as `Option<usize>` (`None`: none
chosen) and returns a `PickResponse`: `response` (`changed` when the choice moved), `chosen`, and `secondary`, the
option a right/two-finger press began on (a context-menu request, never a choice). The strip is one Tab stop, the
chosen option; Left/Right move the focus and the choice. Segments are `Segment::new(text)` values in a `const` array,
never `&[&str]`.

```rust
use pxui::{Segment, Segmented, Toggle};
const SETTING: Toggle = Toggle::DEFAULT.row_h(52.0);
const LOOKS: Segmented = Segmented::DEFAULT.h_px(28.0).name("Appearance");
const OPTIONS: [Segment; 3] = [Segment::new("System"), Segment::new("Light"), Segment::new("Dark")];
let (mut offline, mut look) = (false, Some(0));
pxui::doc::frames(2, &mut |ui| {
    if ui.toggle_with("offline", "Offline mode", offline, &SETTING).changed { offline = !offline; }
    let strip = ui.segmented("appearance", &OPTIONS, look, &LOOKS);
    if strip.response.changed { look = strip.chosen; }
});
```

## Menus and dialogs

Call them every frame while open; stop when they report a result. A menu is a `const` `[MenuItem]` plus a `Menu`;
empty text is a separator. `menu.chosen` is the row index; `menu.dismissed` is Escape or a press outside. A dialog
takes `const` `[DialogButton]`, a `Dialog`, and a body closure; `chosen` is the button index.

```rust
use pxui::{ui::{Menu, MenuItem}, Dialog, DialogButton, Label};
const ITEMS: [MenuItem; 3] = [MenuItem::new("Play next"), MenuItem::new("Add to playlist"), MenuItem::new("Delete").danger()];
const MENU: Menu = Menu::DEFAULT.row_h(36.0);
const BUTTONS: [DialogButton; 2] = [DialogButton::new("Cancel").cancel(), DialogButton::new("Delete").primary().danger()];
let (mut chosen, mut confirmed) = (None, None);
pxui::doc::frames(2, &mut |ui| {
    chosen = ui.menu("song-menu", (10.0, 10.0), &ITEMS, &MENU).chosen;
    confirmed = ui.dialog("delete", "Delete song", &BUTTONS, &Dialog::DEFAULT, &mut |ui: &mut pxui::Ui| {
        ui.label("This cannot be undone.", Label::BODY);
    }).chosen;
});
assert!(chosen.is_none() && confirmed.is_none());
```

## Lists, views and layout

`ui.list(key, count, row_h, &|i| id, |ui, i| ..)` virtualizes: only the rows in view are built, so a
100,000-song library costs what a screen costs. The `id` closure gives each row its model id (the key; unique among
rows). `view_of(key, &inputs, |ui| ..)` rebuilds its subtree only when `inputs` (anything `Hash`) changes: list in
`inputs` everything the closure reads, or it goes stale. `row`, `column` and `stack` take a key, a `const` `Node` and
the children closure.

```rust
use pxui::{ui::{Node, Size}, Label};
const BAR: Node = Node::row().h(Size::Px(44.0));
let titles = ["Blue in Green", "So What", "Flamenco Sketches"];
pxui::doc::frames(2, &mut |ui| {
    ui.column("screen", Node::column(), |ui| {
        ui.view_of("header", &titles.len(), |ui| {
            ui.row("bar", BAR, |ui| { ui.label_fmt(format_args!("{} songs", titles.len()), Label::TITLE); });
        });
        ui.list("songs", titles.len(), 44.0, &|i| i as i64, |ui, i| { ui.label(titles[i], Label::BODY); });
    });
});
```

## Widget bodies: structure belongs to the app, looks to the theme

Every widget kind with a body (label, button, icon button, toggle, segment, dialog, menu row, ...) is built through
its body. The app registers one with `ui.register_<kind>_body(name, fn)` (a plain `fn(&mut Ui, &Config)`, before
the first frame) and a theme file selects it with `button.body = name`. The body decides structure (parts, nesting,
sizes); the theme decides colours, fonts and state transitions. A call's style value reaches the body in its
config, and `.variant(n)` on a style picks between structures of one body per call. Do not draw a widget by hand to
get a different structure; register a body.

```rust
use pxui::{bodies::ButtonConfig, Button, Size, Ui, UiDesc};
fn pill(ui: &mut Ui, c: &ButtonConfig) {
    let mut row = ui.node_layout(&c.row);
    (row.w, row.h) = (Size::Px(120.0), Size::Px(40.0));
    ui.set_node_layout(&c.row, &row);
}
let mut ui = Ui::new(&UiDesc::DEFAULT).expect("library");
ui.register_button_body("pill", pill).expect("registered");
const WIDE: Button = Button::DEFAULT.variant(1);
let _ = WIDE;
```

## Checklist

1. Style values are `const`; builders only.
2. Keys are static strings or ids, never display text.
3. Intents leave the build as values (`Response`, `chosen`, `changed`); the controller applies them.
4. Nothing allocates in a build: no `format!`, no `Vec`, no `String` building; cache text on the model.
5. Long collections go through `list`; unchanged panels through `view_of`.
6. Structure changes are widget bodies, look changes are theme tokens.
7. Add a test that runs two warm frames under the counting allocator when you add a screen.
