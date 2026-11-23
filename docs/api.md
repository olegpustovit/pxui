# pxui API ledger

The target public surface of `pxui` and `pxui_types` for the Unification Plan (U0–U6). Part 1 is the
plan's section 5 for people; part 2 is the same target as a machine list that LocalWave's
`tools/dx/surface.py --ledger` compares with the generated surface (`tools/dx/pxui_surface.txt`).

Rule: an item not marked with a step keeps its HEAD signature exactly. An item marked with a step
changes once, in that step, to the line shown. A step that changes the public surface updates its
lines here in the same commit; the orchestrator owns the rest of the file.

## 1. The ledger (human view)

The target public surface. **Rule:** a call not marked with a step keeps its HEAD signature exactly. A call marked with a step changes once, in that step, to the signature shown. U0.3 generates the full HEAD → ledger delta from `tools/dx/surface.py`, and every delta line must carry a step id here (otherwise it reverts to HEAD). `K: Into<Key>` and `F: FnMut(..)` are the one-line shells; `&mut self` is omitted.

### Rules every row obeys

- Style values are `Copy`, have `DEFAULT` and presets, `const fn` builders taking `self`; looks borrowed (`Option<&'a States>`) in every style value that has looks. No call site writes a struct literal. Reused styles are named `const`s; one-offs and runtime fields may be built inline (D25.2).
- Labels are keyless leaves `(text, Label)`. Interactive calls: key first, return `Response` (clicked, secondary, hovered, held, focused, activated, changed, handle) or one result struct. Runtime data (value, range, title, anchor, widths, selection) stays a parameter; style values hold only constants.
- Sizes `f32`, positions `Point`. `Result<_, Error>` for fallible-by-data calls, `Option` for "absent", panics only for documented caller bugs.
- Build-time setters (`text`, `style`, `set_view`) set state during the build; the `patch_*` family (`patch_text`, `patch_look`, `patch_rect`, `patch_mark`, `patch_field`) updates retained nodes without a rebuild. Both families are documented as such.

The open-tier `pxui_sdl::app::run_with_density(title, width, height, high_density, callback)`
uses the same callback lifecycle as `app::run`. On Apple platforms, `false` requests a drawable
at the window's logical size for native 1× captures. `app::run` requests display density.

| Job | Signature (target) | Body kind | Step |
|---|---|---|---|
| Containers | `row/column/stack<F>(node: Node, f: F) -> Handle` (keyless, D25.1); `id<K, F>(key, f)` scopes descendant identities without adding a node; the instance getter `id() -> usize` is renamed `instance_id() -> usize` (D26) so `id` names one thing; `node<K>(key: K, node: Node) -> Handle` (HEAD takes `Key`; becomes key-first generic like every other call, codemod drops `.into()`); `within(&Handle, &mut dyn FnMut(&mut Ui))` | — | keyless + `id` U1b.6; `node` U1b.2; `enter/leave` → crate-private U1a.3 |
| Pressable / disabled | `pressable<K, F>(key, node: Node, &Pressable, f: F /* (ui, State) */) -> Response`; `disabled<F>(bool, f)` | — | HEAD |
| Retained view | `view_of<K, T: Hash, F>(key, inputs: &T, f) -> Handle`; `set_view(&Handle, &ViewTransform)` (const clip, blur and saturation builders leave omitted fields alone. clip(None) lifts the clip) | — | U1b.4 |
| Scroll | `scroll_view<K, F>(key, node: Node, f) -> Handle`; `scroll(&Handle) -> Scroll`; `set_scroll_offset`; `reveal` | scroll_view | U1b.5 |
| Split | `split<K>(key, ratio: f32, &Split, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> SplitResponse { response, ratio: Option<f32> }`; `Split` holds axis, min_a, min_b | split | U1b.1 |
| Text | `label(&str, Label) -> Handle`, `label_fmt(Arguments, Label)`, `label_digits(Arguments, Label)`; `text(&Handle, &str)`; `text_spans`, `text_line_box` take `Label` | label | HEAD; spans U1b.3 |
| Measure (D20) | `Ui::text_service() -> UiText<'_>` and `Paint::text_service() -> UiText<'_>`; `UiText::measure(&str, Label, max_w: Option<f32>) -> Result<TextMetrics, Error>`, `measure_spans`, `width`, `rasterize_rgba`, `quads`, `add_font`, `enable_system_fonts`; worker form `Text::measure(&str, &TextStyle, max_w)` | — | U1b.3 |
| Text worker | `TextStyle`, `Text`, `TextWorker`, `PreparedText`, `RetiredText`, `UiText::fork/install`, `prewarm_text`, `set_text_worker`, `resolve(&Label) -> TextStyle` | — | HEAD (D20) |
| Button | `button<K>(key, text: &str, &Button) -> Response` (D25: one name, no short form, no `_with`); `button_content<K, F>(key, &Button, f /* (ui, State) */) -> Response`. `Button`: icon, icon_size, detail, accessory, label(Label), selected, disabled, danger, align, pad, axis, variant, look | button | name + `disabled` U1b.6; `icon_size` U2b.7; body configs `ButtonConfig::icon()`/`IconButtonConfig::icon() -> Option<Icon>` (D18) U2b.7; `ButtonConfig::label()`/`detail_label()`/`accessory_label() -> Option<Label>` (the caller's styles, for a Rust body) U2b.4.1; `PxuiButtonConfig.built_in` (a slot: `ui.slot(config.built_in)` draws the kit's built-in button, so a registered body styles only its own variants) U2b.4.2; `ButtonConfig::width()`/`height() -> Option<Size>` and `content_axis() -> Option<Axis>` (the caller's `Button::size` and `axis` as the types they were, `None` the kit's; the record's `w_kind`/`h_kind`/`axis` numbers stay its C encoding; review finding 12) U2b.4.27; `ButtonConfig::danger() -> bool` (the caller's `Button::danger`; C `PxuiButtonConfig.danger`, ABI 1.127) U2b.4.35; `State::from(config.state)` (`impl From<PxuiStateDesc> for State`: every body config's `state`, and a search field's `clear`, read typed through one conversion) and `ButtonConfig::centered() -> bool` (`Button::centered`) (U4b review 3, Rust only) U4b |
| Icon button | `icon_button<K>(key, Icon, name: &str, &IconButton) -> Response`; `PxuiIconButtonConfig.built_in` (a slot: `ui.slot(config.built_in)` draws the kit's icon button, so a registered body styles only its own variants, as the button's U2b.4.2) | icon_button | HEAD; built_in U2b.4.6; `IconButton::selected(true)` keeps the checked look while hovered or held, press scale unchanged, U2b.4.9 |
| Toggle | `toggle<K>(key, text, on: bool, &Toggle) -> Response` (D25) | toggle | U1b.6 |
| Checkbox | `checkbox<K>(key, text, on: bool, &Checkbox) -> Response`; `Checkbox<'a>`: look (borrowed), size, disabled(bool), passive(bool), variant | checkbox | U2a.3 |
| Pick one (D4) | `segmented<K>(key, &[Segment], selected: Option<usize>, &Segmented) -> PickResponse`; `tabs<K>(key, &[Segment], Option<usize>, &Tabs) -> PickResponse`; `radio<K>(key, &[Segment], Option<usize>, &Radio) -> PickResponse`; `dropdown<K>(key, text, &[Segment], Option<usize>, &Dropdown) -> PickResponse`; `Segment::new(text).id(i64).detail(&str).name(&str)`; a segment body reads `SegmentedConfig::selected()`/`stored() -> Option<usize>` (the record's `has_selected`/`has_stored` pairs, as `ButtonConfig::icon`; U4a re-review R4, Rust only); a tabs body owns its bar as a segment body its strip: `TabsConfig.bar` (`PxuiTabConfig.bar`, U2b.4.29, ABI 1.124); the tabs' `response.clicked`/`activated` fire for a press or key that chooses any tab, the one in use included, as Segmented's strip (U2b.4.31, ABI 1.125, behaviour only) | segmented / tabs / radio / dropdown | U1b.1 (segmented, radio, dropdown); U2a.2 (tabs) |
| Section index | `section_index<K>(key, letters: &[&str], &SectionIndex) -> PickResponse`; `SectionIndex`: DEFAULT, name, variant; its accessible value is the current letter (the number's words, `A11yValue` tag 2 text range; U2b.4.34, ABI 1.126) | section_index (new) | U2a.3; U2b.4.34 |
| Disclosure | `disclosure<K>(key, text: &str, open: bool, &Disclosure, f: &mut dyn FnMut(&mut Ui)) -> Response` (changed = toggled) | disclosure (section engine) | U2a.3 |
| Slider (D10) | `slider<K>(key, text, value: f32, range: RangeInclusive<f32>, &Slider) -> SliderResponse`; `scrub<K>(key, value, range, &Scrubber) -> SliderResponse` | slider (scrub: caller-painted) | U1b.1 |
| Progress / spinner / badge | `progress<K>(key, text, value: Option<f32>, &Progress) -> Handle`; `spinner<K>(key, name, &Spinner) -> Handle`; `badge<K>(key, name, text, &Badge) -> Handle` (argument order as HEAD); a progress body reads `ProgressConfig::value() -> Option<f32>` (`has_value`/`value`; U4a re-review R4, Rust only) | progress / spinner / badge | U1b.5 |
| Menu | `menu<K>(key, at: Point, &[MenuItem], &Menu) -> Chosen`; `menu_bar<K>(key, &[MenuBarMenu], &MenuBar) -> Chosen` (flat index in menu order, item identity via `MenuItem::id`); `MenuBarMenu::new(title, &[MenuItem])` | menu_row / menu_item / menu_bar | U1b.1 |
| Dialog | `dialog<K>(key, title: &str, &[DialogButton], &Dialog, body: &mut dyn FnMut(&mut Ui)) -> Chosen`; `DialogConfig.built_in` / `PxuiDialogConfig.built_in` (a slot: `ui.slot(config.built_in)` builds the kit's buttons where a registered dialog body places them, each through the button body with the dialog's variant, which the built-in dialog's buttons take too; as `ButtonConfig.built_in`, ABI 1.131) U2b.4.38 | dialog | HEAD; U2b.4.38 |
| Overlays | `panel/popup/modal<K, F>(key, &Overlay, f) -> OverlayResponse { handle, open: bool }`; `anchored<K, F>(key, anchor: Rect, &Overlay, f) -> OverlayResponse`; sheets via `Overlay::present(Presentation::{Centered, Bottom, Full, Top})` (`Top`: centred across, top edge at `Overlay::at`'s y, U2b.5) + `.dismiss(DismissOpts)`; a modal body reads it as `ModalConfig::presentation() -> Presentation` (the record's `present` number; review finding 12, U2b.4.27); `tooltip(&Handle, &str)` keyed by the owner; `toasts<K>(key, &[Toast], seconds: f64) -> Chosen` | panel / popup / modal / tooltip / toasts | U1b.1 (results, tooltip key); U2a.4 (present) |
| Lists | `list<K, F>(key, count, row_h: f32, row_key: &dyn Fn(usize) -> i64, f) -> ListResponse` (short form, never reorderable); `section_list<K>(key, &SectionList, row_key, build /* (ui, SectionPart, CellState) */) -> ListResponse` (`SectionList::reorder(bool)`; the row content builds its `Part::HANDLE`; the callback's `CellState` carries the row's box, which the app names, and the part's content Y, as `selection_list`'s, U2b.4.10; C `pxui_ui_section_list` takes a `PxuiListCell`, `PxuiSectionBuild` deleted); `grid_list<K>(key, count, &GridList, build) -> Handle`; `ListResponse { handle, activated, selected, secondary, row: Option<Handle>, moved: Option<(usize, usize)>, drop_at: Option<usize> }` (U2b.4.18: `focused` deleted; a focused list takes the keys it handles from `key_pressed`; U2b.4.25: Escape clears a focused multi-selection and is the list's, unless a lifted row or the app's `cancel_carry` spent it; a plain held press leaves it to the selection; U2b.4.26: Enter activates the keyboard's row only while it is selected, and an app-written selection brings the keyboard's row to its anchor; U2b.4.33: the exception is `SelectMode::None`, which selects nothing, where Enter activates the keyboard's row) (`selected`: set whenever the widget changed the selection this frame, naming the row; `moved.1` is the final index; no `Eq`: `Handle` has none); `SectionPart::Trailing` builds the `trail` area (an action row ending a list) | — (rows are app content; the insertion line is the row body's) | U1b.5 (grid_list); U2b.2–U2b.3 (ListResponse, reorder); U2b.4.10 (row handle); U2b.4.11 (focused) |
| Selection list | `selection_list<K>(key, count, row_key, state: &mut ListState, &SelectionList, cell: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse`; `ListState { rows: Vec<u8> /* SELECTED, DISABLED, LIVE flags */, anchor: Option<usize>, revision: Option<u64> }`, `ListState::new()` (`revision`, U2b.4.13: the rows' content revision, as `view_of`'s inputs: `Some` keeps each row's build (body, cells) while it, the row's key and its selected, disabled and focused state hold, moved on scroll; a pointer, key or action reaching a row rebuilds it; `LIVE` rows, and a reorderable list's in view, rebuild every frame; a kept row past the viewport keeps its build until it scrolls into view (U2b.4.14, ABI 1.107); a table's rows also when a column's width changes; C `has_revision`/`revision` in `PxuiSelectionListDesc` and `PxuiTableDesc`, row flag 4, ABI 1.106); `CellState { selected, enabled, focused, held, handle, y }`; `SelectionList` carries the `SectionList` geometry (`row_h`, `lead`, `header`, `trail`, `measured`, `overlay_scrollbar`) plus `mode(SelectMode)` (`SelectMode::{Single, Multi, Toggle, None}`: one row; Command toggles and Shift extends; as Multi with a plain click toggling and no click or double-click activation; nothing selected and rows not selectable, presses and keys move the keyboard's row, Command-A/Escape/Space stay the app's; C `PxuiSelectionListDesc.mode`, the discriminant, in `multi`'s byte; replaces `multi()`, U2b.4.33, ABI 1.128), `reorder(bool)`, `variant(u16)`, `fit()` (as tall as its rows, no scroll), `drop_look(Look)` (a row's look while a payload hovers its drop target; it wins over a row body's look, U2b.4.21), `ListState::NO_DROP` (row flag 8: the row takes no payload and keeps its look; U2b.4.21, ABI 1.120), a row body reads `PxuiRowConfig.drop` against `PxuiRowConfig::DROP_NONE`/`DROP_BEFORE`/`DROP_AFTER`/`DROP_OVER` (0-3; review finding 12, U2b.4.27), `bare()` (no frame look and no 1-point inset; U2b.4.3); selection semantics (click, Meta-toggle, Shift-range, double-click, keyboard, Command-A) in the widget; C `PXUI_ROW_DROP_NONE`/`_BEFORE`/`_AFTER`/`_OVER` name `PxuiRowConfig.drop` as Rust's `PxuiRowConfig::DROP_*` (U4a re-review R4, ABI 1.123) | selection_list | U2a.1 (row_key), U2b.1 (rest; reorder U2b.3); U2b.4.13 (kept rows), U2b.4.14 (kept rows past the viewport); U2b.4.32 fix (a widget given a variant in the lead-in, heading or trailer leaves the list's variant to its rows' bodies); U2b.4.33 (`mode`) |
| Table | `table<K>(key, &[Column], rows, row_key, state: &mut TableState, &Table, cell: &mut dyn FnMut(&mut Ui, usize, usize, CellState)) -> TableResponse { rows: ListResponse, head: HeadResponse }`; `table_head<K>(key, &[Column], state: &mut TableState, &TableHead) -> HeadResponse { response, sorted: bool, resized: bool }` (also a `selection_list` heading: it writes `TableState::widths`); `Column::new(title).sortable(bool).share(f32).w_px(f32).grip(bool).id(i64)` (every column stays in the slice; `w_px(0.0)` hides one; without a grip `w_px` holds every frame); `TableState { widths: Vec<(i64, f32)>, sort: Option<(i64, Sort)>, selection: ListState }` (keyed by column id), `TableState::new()`; `Table::DEFAULT.mode(SelectMode).open_on_click(bool).bare().row_h(f32).head(TableHead).variant(u16)` (U2b.4.37: the selection list's `SelectMode`, `open_on_click` and `bare`; `multi()` deleted; C `PxuiTableDesc.mode` in `multi`'s byte, `open_on_click`/`bare` from `reserved2`, ABI 1.130); `TableHead::DEFAULT.h_px(f32).variant(u16)`; `Sort::{Ascending, Descending}`, `None` = original order; `TableView` and the fixed text table deleted; a table head body reads `TableHeadConfig::ascending() -> Option<bool>` (`has_ascending`/`ascending`; U4a re-review R4, Rust only); `PxuiTableHeadConfig.built_in` (a slot: `ui.slot(config.built_in)` draws the kit's heading, so a registered body styles only its own variants, as the icon button's U2b.4.6) U2b.4.36 | table_row / table_head (`table_cell` deleted with the text table) | U2b.1 |
| Tree | `tree<K>(key, &[TreeNode], state: &mut TreeState, cell) -> TreeResponse`; `TreeState { open: Vec<u8>, selected: Option<usize>, visible: Vec<u32>, revision: Option<u64> }`, `TreeState::new()`, as `ListState`/`TableState` (U2b.4.28, review finding 13: the four state arguments folded into one `&mut`, `open` and `visible` sized to the nodes every frame; the C record `PxuiTreeViewDesc` already carries the same state in one record, unchanged; review finding 14, ABI 1.121: kept rows that no longer name a node, as a caller that broke its revision promise leaves them, are re-derived, never read past the nodes; U4a re-review R3: each kept row is bounds-checked where it is read, so a warm frame checks the keyboard's row and the rows in view, not every shown row) (`revision`, U2b.4.17: the content revision of `nodes` and `open`, as `ListState::revision`: `Some` keeps the shown rows in `visible` between frames while it and the tree's own expansions hold, so a warm frame walks only the rows in view; `None` re-derives them every frame, a walk over every node; C `has_revision`/`revision` in `PxuiTreeViewDesc`, ABI 1.112); `TreeNode::new(depth, has_children)` | tree | HEAD; builder U1b.5; U2b.4.17 (revision); U2b.4.28 (TreeState) |
| Picker | `picker<K>(key, placeholder, query, &Picker, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse`; `PickRows::push(PickRow)`; `PickRow::new(label).detail(..).art(PickArt).heading(..)`; `PickerOpts` → `Picker`; `Picker::image(&dyn Fn(usize) -> Option<Image>)` (resolved as `Picker::mark`; a row body's `PickerConfig.image`, `has_image`; the built-in row draws it in the art slot); `PickerConfig.head` (`has_head`): the heading box of the group the row starts, which the row body lays out and styles (the `line` pattern); a picker field body's `Part::EDITOR` is the editing surface, the query painted in its look's font (as the search field's, U2b.4.4); the results scroll (wheel, trackpad, the keyboard's row kept in view), `Picker::limit` rows of the kit's height high at most, rows as high as their body sets them (U2b.4.15; C `PxuiPickerImage`, `PxuiPickerRowConfig.has_image`/`has_head`/`image`/`head`, ABI 1.110); `PickerFieldConfig.list`: the results region, built before the field body runs, which the body lays out with `set_node_layout` (maximum and minimum height, padding), `Picker::limit` kit rows by default (the `line` pattern, as `SearchFieldConfig.line`; U2b.4.16, ABI 1.111); `PickArt::Draw(word)`: the app's icon by its word (as `Icon::Draw`'s), which a registered row body draws, read with `PickerConfig::art() -> Option<PickArt>` (as `ButtonConfig::icon`; C `PXUI_PICK_ART_DRAW` 3, U2b.4.19, ABI 1.117); `PickerConfig::image() -> Option<Image>` and `head() -> Option<Handle>` read `image`/`has_image` and `head`/`has_head` (as `ButtonConfig::icon`; review finding 12, U2b.4.27, Rust only) | picker | U1b.1 (rename); U2a.4 (rows); U2b.4.15; U2b.4.16; U2b.4.19; U2b.4.27 |
| Text input | `text_field<K>(key, value: &str, &TextField) -> TextFieldResponse`; `search_field<K>(key, value, &SearchField) -> SearchFieldResponse` (`SearchField::variant(u16)`, `SearchField::action(ReturnKey)` with `TextTraits::SEARCH` by default; a search field body's `Part::EDITOR`, built before its editor slot, is the editing node, edited and painted in its look's font, U2b.4.4; `SearchFieldConfig.line` / `PxuiSearchFieldConfig.line`, the field's line a body sizes with `set_node_layout` as `ButtonConfig.line`, U2b.4.8); `SearchFieldConfig.clear` / `PxuiSearchFieldConfig.clear`, the clear key's `State` (`Part::CLEAR`, hovered and pressed), which behaviour owns with its press, role and name and a body styles the key for (`ui.style(&key, &LOOKS, state)`), as `ButtonConfig.state` (U2b.4.23, ABI 1.122), read typed with `State::from(config.clear)` (U4b review 3); `text_area<K>(key, value, node: Node, &TextArea, &AreaCallbacks) -> TextAreaResponse` | text_field / search_field | HEAD; text_area U1b.5; SearchField U2b.7; U2b.4.23 |
| Gestures, drag, drop | `gesture`, `gesture_state`, `close_swipe`, `end_refresh`, `carry`, `carrying`, `transfer_carry`, `cancel_carry` (U2b.4.25: spends the frame's Escape press that cancels a carry), `drop_target` (any node, no pressable needed; a plain node's drop look is the app's pick of two theme `States` consts on hover, list rows take `SelectionList::drop_look`; `States` gains nothing); `GestureOpts`, `SwipeActions`, `DismissOpts` | — | HEAD; drop U2b.2 |
| Keys (D23) | `key_pressed(KeyCode, Mods) -> bool` (U2b.4.18: a press a focused widget handled is not reported; a text editor's keys are, gated by `text_input_focused`), `key_down(KeyCode) -> bool`, `testing::key(KeyCode)` | — | U1a.4, U2b.4.18 |
| Local state | `state/set_state(&Handle, ..)`, `text_state/set_text_state`, `expanded<K>(key) -> Option<bool>`, `hover_delay<K>(key) -> bool`, `load_text_edit<K>(key, ..)`, `box_of_key<K>(key)`, `root_at(Point, &[Key]) -> Option<Key>` (innermost candidate), `part_box/part_rect(&Handle, ..)`. Authored keys resolve under the current seed. Already-resolved keys use `Key::Resolved` | — | U1b.2 |
| Paint | `canvas/canvas_over(&Handle, f)`; Paint shapes Rect + Fill/Stroke; `Paint::text/text_chars/text_spans` take `Point` and `Label`. Measurement is on `text_service()`; `Paint::image(Rect, &Image, &ImageStyle)` with a tint builder on `ImageStyle` | — | U1b.3 (text), U1b.2 (image) |
| Looks | `style(&Handle, &States, State)` (`set_look` folded in, `States::of(Look)`; the wrapper picks the state's look and C `pxui_ui_style` takes that one `PxuiLookDesc`, U4a.6 fix, ABI 1.108); `patch_*` family; a node's own `Node::transition` (declared, or carried by `set_node_layout`) runs its changes of state instead of the theme's, also under a theme that does not move (U2b.4.5); `States::pick` is the one state→look priority of every kit widget and body: disabled > checked (on, selected) > pressed > hovered > focused > idle (U2b.4.20, ABI 1.118 behaviour) | — | U1b.4; U2b.4.5; U4a.6 fix (C one look); U2b.4.20 |
| Animation | `animate(&Handle, Target, Curve)`, `animate_keys(&Handle, &[Keyframe], &Keyframes)` (two jobs) | — | HEAD |
| Images | `image<K>(key, name, &Image, &ImageStyle) -> Result<Handle, Error>`; `Image::rgba8(width, height, vk_image_view, mtl_texture)` describes native storage; `images::{Want, Request}`, `Images::store/get/requests` | — | U1b.2; builder U1b.5 |
| A11y | `a11y_name(&Handle, &str) -> bool` with UTF-8-safe display clipping; `a11y_value/a11y_number(&Handle, ..) -> Result<bool, Error>`; `set_role(&Handle, Role, A11yStates)`; `a11y_tree() -> A11yTree<'_>` (borrowed strings, revision and first child); `a11y_nodes`, `a11y_name_of`, `a11y_value_of`, `box_of_a11y` | — | U1a.3 (rule), U1b.4 (types) |
| Theme | `set_theme(&ThemeHandle) -> Result<(), Error>`, `themed(&ThemeHandle, f)`, `theme()`, `tokens()`, `ThemeHandle::parse/set_color/density/reduced_motion`; `bodies::register_<kind>_body(..) -> Result<BodyId, Error>`; `Rgba::token(T).alpha(a)` (D19); `ColorToken::ACCENT` (= `ColorToken(64)`, the theme's `Tokens::accent`); `Appearance`; sizes `f32` (`font_size`, `row_height`, `density`, `set_font_size`) | — | U1a.2; U1b.3 (units); U2b.7 (alpha, accent) |
| Frame / lifecycle | `Ui::new(&UiDesc) -> Result<Ui, Error>` with `UiDesc::new(Platform).font_size(f32).fonts(&[FontData])` and `UiDesc::DEFAULT`; one `frame(&FrameIn, build)`; `FrameIn::new(w, h)` with const builders (`surface(u64)`; `events(&'a [Event])`, borrowed for the frame call, U5.1; the snapshot form was removed in U5.2), `Ui::add_surface(u64)`/`remove_surface(u64)` (`Ui::new` registers `pxui_types::FrameIn::MAIN`; an unregistered surface is `InvalidInput`; one surface per Ui); `layer(i) -> Option<Layer>`, `layers() -> Layers`, `output() -> Output`; `focus_box(&Handle)` stays public for the registered command editor body, with `focused` crate-private and `enter`/`leave` internal to the engine and C ABI; `focus_default(&Handle)` beside it (U2b.4.24): the box a key press takes as its focus while no node holds it, stated each build (an app's main list as the window's default key target); while another widget (not a text editor) holds the focus, a press it leaves untaken falls through to it (U4a re-review R1: a focused sidebar leaves ⌘A/Esc to the main list, keeps Return) | — | U1a.3, U2b.4.24 |
| Host integration | `Renderer::new -> Result<_, Error>`; `Ui::register_<kind>_body(..) -> Result<BodyId, Error>`; `A11yBridge::connect -> Option`; `Gpu`, `Pass`, `Commands`, `Input`, `Output`, `Platform`, `Bytes`; pxui_host `Graphics::open/draw/close`, `Accessibility::open`, `Platform::pump/apply` → `Result<_, Error>`; `Accessibility::frame(ui, surface)` routes native actions to the frame's surface | — | U1a.3; U5.9 review fix |
| Diagnostics | `phase_times`, `phase_cpu_times`, `set_timing`, `repaint_after`, `wants_frame`, `library_counter` (D3), `verify_views` (D22: per `Ui`, on under `debug_assertions`; `verify_new_views` deleted) | — | HEAD; U1c.2 |
| Getters | `keyboard()`, `environment()`, `text_traits()` (was `text_input_options`), `ime() -> Option<Ime>`, `platform()`, `now()`, `frame_index()`, `system_appearance() -> Appearance` | — | U1a.4 (appearance), U1b.2 |
| Signals | `signal`, `signal_keys` → `Signal`: documented as the body/custom-control layer | — | HEAD |
| Hover / press queries | `hovered(&Handle, within: bool) -> bool` replaces the `hovered_exactly`/`hovered_within` twin; `press_keys`, `last_press_within(&[Event], &[Key]) -> Option<Key>`, `context_at(&Handle) -> Option<Point>`, free `context_clicked` | — | U1b.2 |
| Kept as HEAD (listed so the two-way check passes) | Build/retained: `set_at`, `to_front`, `node_layout`/`set_node_layout`, `node_mut` (made non-generic U1c.1), `cached_label`, `busy_step`, `set_dir`, `rtl`, `view_size() -> ViewSize` (width and height in logical points, U1b.3). Input/text: `captured`, `reordering` (a list row lifted by its handle, U2b.4.12), `clipboard_request`, `paste_requested`, `passed_undo`, `text_input_focused` (U2b.4.18: a text editor holds the focus, not any focus), `text_undo_bytes`, `unfocus`, `remember<K>(key, ..)` (key-first with `load_text_edit`, U1b.2), `event_text`, `dismiss_popup`. Frame/diagnostics: `request_frame`, `step` (transitions without a build, ABI 2.4), `request_cursor`, `arranged_nodes`, `built_nodes`, `rebuilt_views`, `rebuilt_named` (counts retained views rebuilt on any settling pass), `draw_calls`, `draw_revision`, `text_layout_count`, `verifying`, `abi_version`. A11y: `set_a11y`, `inject_a11y`. | — | HEAD unless noted |
| Testing | `testing::{frame, click, click_node, key, find}` | — | U1a.4 |
| Extension set (no stability) | `pxui::ext::{inspector_*, footer, line_field, value_field, grip, field_drag, inspector_width, inspector_label_x}`: free functions taking the `Ui` first (`pxui::ext::inspector_toggle(ui, key, label, on)`); the inspector toggle and button are the toggle and button bodies' | through bodies | U1c.3 |
| 2.0 additions | Error/diagnostics export and font list at create (C, U5.9); `pxui_supports` baseline (U5.8); surface registration (U5.2); `A11yBridge::update(surface, tree)` and `remove_surface(surface)`, with each returned `A11yAction.surface` naming its destination (owner-approved U5.9 review fix, 21 Nov); injection refuses another surface before applying any action | — | U5 |
| 2.1 additions | `Ui::label_at(Key, Rect, &str, Label) -> Result<Handle, Error>` places a plain text line box under an authored key. `ThemeHandle::animate_swap(bool)` controls appearance transitions when the theme is installed. | — | 2.1 |
| 2.3 addition | `Ui::retain_palette_paint()` opts in existing views with palette-independent geometry and symbolic canvas shape colors. | — | 2.3 |
| 2.2 additions | `Ui::retain_palette_labels()` opts a positioned-label view into reuse on instant palette changes. `Ui::retain_view_on_resize()` opts a viewport-independent view into reuse while its local size and device scale hold. | — | 2.2 |

## 2. Machine list

One item per line in exactly `surface.py`'s rendering (`<path> <kind> <signature>`), then a tag:

- `@HEAD`: the item exists today with this line and keeps it.
- `@U<step>`: the item's target line; it exists once that step has landed (a commit subject on HEAD
  starting with `[U<step>`). Until then the HEAD line of the same path is expected instead.
- `-<line> @U<step>`: the item is deleted by that step.

`surface.py --ledger` reports both directions (surface items missing here, items here missing from the
surface) for landed steps; `--all` compares the whole target, which is `tools/dx/ledger_delta.txt`.
Lines added for new items carry best-known names; the step that adds an item corrects its line here.

The U1a frame descriptor leaves the C input records and routes available. The event payload
lifetime work stays in U5.1. In Rust, a layer lends published instance and dirty-range bytes.
The output descriptor is a value so fresh queries can reflect a draw acknowledgment while a
borrowed layer remains valid. The C output query and layout stay intact. Programmatic
`focus_box` serves the command palette and compact Find on the build that creates the field.

`KeyCode::Char` preserves the character supplied by the host. `Mods::ANY` is a wildcard and
`Mods::NONE` requires no modifiers. A nonempty requirement matches one press with every requested
modifier held, even when more modifiers are held or a later event releases them.

### U1b shape decisions

Approved at the U1b shape review (2026-11-18) unless marked as a finding to honor in implementation.

- Pick-one options keep a borrowed name and detail. An explicit item id keys the item's node, focus and secondary-press routing, so they follow it through a reorder, and becomes `chosen_id` in U1b.6. Items without an id are positional in a namespace distinct from ids (explicit id 1 and index 1 differ). Id, name and detail cross in both link modes. `chosen` is the current selection, `response.changed` reports a change, and `secondary` reports the item where the secondary press began.
- Sliders keep value, drag preview and final commit separate. A held drag supplies `preview`; a release, keyboard step or accessibility edit supplies `commit` with `preview` equal to it on that frame, so a caller painting from the preview shows the committed value before an asynchronous update. `SplitResponse::ratio` is present while the divider is held and on its release frame (the final value), and always none for a split body without a grip.
- `OverlayResponse::open` preserves modal and popup dismissal. The handle is empty after a dismissal. Toasts report a removed item's index and id while the stack stays open.
- Lookups (`node` lookup, `box_of_key`, `root_at`, `last_press_within`, `hover_delay`) resolve authored keys under the active scope seed during a build and under the root seed outside one; `Key::Resolved` is a valid identity everywhere, creation calls included: it crosses as a 0x01-tagged label that the engine uses as the key itself, never hashed again, and the reserved `Resolved(0)`/`Resolved(1)` are refused as invalid keys, never aliased to `Id(0)`/`Id(1)` (a creation call, `expanded` and `hover_delay` panic at the caller as for an overlong key; `box_of_key` and `root_at` find nothing; C refuses a 0x01 label carrying 0 or 1 or malformed in every key-taking call). Authored labels beginning with a byte below 0x20 are reserved. `root_at` prefers the innermost matching ancestor regardless of candidate order and keeps the 65,536-candidate bound. The Rust hover timer and tooltip owner use the resolved key without allocating; the 1.x C text-keyed hover export keeps its behavior.
- `style` validates all six looks before applying any, as the 1.x style route does, including an invalid value in an inactive look (accepted; the cost of six looks for migrated `set_look` callers is measured at CP1).
- `Ime::caret` is a horizontal position, so `f32` points.
- Keyless `row`, `column` and `stack`, and labels and images (formerly the reserved texts "\0label" and "\0image"), take engine positional keys derived in the engine's own hash domain in U1b.6, so no `Key` can collide with them. `id(key, f)` returns unit: it scopes descendant keys, adds no node, and a caller captures a descendant's handle inside `f`.
- `ViewSize` describes the surface's width and height. `Size` remains a one-axis layout rule. `UiText` borrows its UI or painter, and `Text` keeps the resolved worker style. Tuple positions this list pins as HEAD (`set_at`, `ViewTransform::about`) stay HEAD; D21 is not expanded at U1b.
- Fractional units: `Font` gains `size_points: f32` in its existing tail padding at offset 28 (size, alignment and 1.x offsets unchanged); `Font::size(f32)` sets it and the rounded legacy `size`, `points()` returns it, and the engine reads it only from records whose size prefix declares the fractional version. Rust `Ui::theme()` borrows the engine-owned fractional snapshot: `Tokens` text sizes are `f32`, and the legacy integer `ThemeSnapshot` is C-only, with no twin type in Rust. The legacy `PxuiFontDesc`, `PxuiLookDesc`, `PxuiFont`, `Node` and borrowed `ThemeSnapshot` layouts and field meanings are unchanged. New size-prefixed records carry the `f32` font size (creation, setter, looks and nodes) and panel padding, validated separately; the integer C getters keep reporting integers. The Rust theme query borrows an engine-owned fractional snapshot; the legacy projection is prepared at theme or style mutation, never on the warm path. Paint and UI density keep the existing rounded choice.
- `A11yStates` authors flags and exposes their wire mask for inspecting `A11yNode`. The C bridge still takes the wire tree, available in `Ui::output()`. The borrowed Rust tree does not keep it alive beyond the UI borrow.
- `Image::rgba8` is a const initializer of the existing native GPU descriptor: it copies and uploads no pixels, and the host keeps the native view and texture alive through every GPU use. `image(key, name, ..)` keeps a runtime accessible name distinct from the key.
- `GridList` has `DEFAULT` (64-point square cells) and `w_px`/`h_px` cell builders, and no variant: the grid builds app-defined cells and has no body. Existing SectionList trailing-height and overlay-scrollbar builders, font tracking and Look field builders already cover those authoring values.
- The wire Event constructors fill scalar fields. `Str` converts only from `&'static str` and `Slice<u8>` only from `&'static [u8]` until U5.1 defines borrowed events and payloads.
- C wire (U1b.1, ABI 1.76): the Rust results cross through one result per call (`pxui_ui_pick`, `pxui_ui_segmented_items`, the slider and scrub `_result` exports, the overlay `_root` exports, `pxui_ui_split_result`, `pxui_ui_menu_bar_result`). Pick-one items cross as the new `PxuiPickItem` record (id, name, detail); the append-only manifest refuses grown records, so no 1.x record changes size. The earlier exports keep their results.

### U2a shape decisions

Accepted at the U2a shape review (2026-11-18).

- `Disclosure` has no lifetime: it borrows nothing (`DEFAULT`, `variant`), so `<'a>` would need a marker field.
- `PickRow` and `PickArt` live in `pxui_types::ui` beside `PickRows` (its inherent `push(PickRow)` cannot be added from `pxui`) and are re-exported at the `pxui` root and prelude.
- `PickRows::get(i)` returns `Option<PickRow<'_>>` (not `(&str, &str)`): the one accessor through which the engine reads a row's art and heading.
- New public fields, `pxui_types` style: `Checkbox` `size`/`look`/`disabled`/`passive`; `Overlay` `present`/`dismiss`; `DismissOpts` `fraction`/`velocity`. New C records in the surface: `PxuiCheckboxDesc`, `PxuiSheetDesc`, `PxuiPickRow`, `PxuiSectionIndexConfig`, and the `PxuiPickerRowConfig` art fields (extension set).
- Registered bodies receive what the built-in bodies honour, through records grown at their end under the growth rule (docs/abi.md): `PxuiTabConfig.detail` (1.84), `PxuiCheckConfig.states`/`box_size`/`passive` (1.85), `PxuiPanelConfig.present` (1.86), and the `section_index` body kind (`PxuiSectionIndexConfig`, `PxuiBodies.section_index`, 1.85). No second register path.
- The `PxuiPickerRows` callback changed in place (extension set, no stability promise); a C caller built before 1.86 is refused by the row size check, never misread.

### U2b shape decisions

Proposed at the U2b shape stop (2026-11-18); see LocalWave `scratch/u2b-shape-report.md`.

- `selection_list`'s cell takes `SectionPart` (not `usize`): the list carries the `SectionList` geometry, so the lead-in and the sticky heading (a `table_head`) are built by the same callback, as `section_list` does today for the songs list. `CellState.y` carries the part's content Y that `section_list` passes as its third argument. `CellState.handle` is the per-row handle (a row drop target).
- `ListState` and `TableState` are owned (`Vec`), so the app holds one value and passes one `&mut` (D25.3); the wrapper sizes them, so the library never allocates for them. Per-row enabled is the app-written `ListState::DISABLED` flag in the same byte the widget writes `SELECTED` to (one span crosses the ABI).
- `TableResponse` moves to `pxui` (it holds `HeadResponse`): `{ rows: ListResponse, head: HeadResponse }`; `pxui_types::ui::TableResponse` and `TableView` (with its fixed text table) are deleted with the old engine route.
- `HeadResponse` reports `sorted` (a click changed `TableState::sort`; read the sort there, `None` being the original order) apart from `resized` (a grip drag changed a width). `TableState::widths` holds every column's `(id, laid-out width)` in slice order, so rows built elsewhere share them; widths and `sort` are keyed by column id (`Column::id`, the index without one: `Option<i64>`, no sentinel), so a dragged width or a sort follows its column.
- `Column::w_px(f32)` is added beside `share`, with `Column::grip(bool)` (default true): with a grip `w_px` is the starting width, without one it is the width every frame (app-computed responsive widths). Columns stay in the slice every frame (indices are the cell callback's columns); `w_px(0.0)` hides one.
- Shared columns round their cumulative boundaries and assign the full remaining width after point columns and visible separators.
- No dropping look on `States` (review: a seventh look would need a second C record beside `PxuiStates`, a twin). `States` and `PxuiStates` stay the six looks and `State` gains no bit (it is embedded in `PxuiNode`). A list row's drop look is the list style's (`SelectionList::drop_look`, carried in the list's own C record); the row body is told through the library-written row config (`PxuiRowConfig` grows by `drop: u8` — 0 none, 1 insertion line before, 2 after, 3 a payload over the row) and draws the insertion line. A plain drop-target node gets its look from the app picking one of two theme `States` consts while its `drop_target` reports a hover (`ui.style(&target, if hover { &UP_NEXT_DROP } else { &UP_NEXT }, ..)`, consts from builders).
- `ListResponse::moved` is `(from, to)` with `to` the final index (remove at `from`, insert at `to`); `selected` is set by every widget write to the selection (navigation lists act on it; multi lists invalidate caches on it). `Part::HANDLE` takes the touch on press (as `GestureOpts::touch_drag`), so a handle drag never scrolls the list.
- `SectionPart::Trailing` (gap from the site map, sidebar "+ New list"): the `trail` height is built by the callback after the last row, never a row (not selected, counted or keyed); a callback that ignores it leaves blank space as before.
- `ListResponse` loses `PartialEq`/`Eq` (`PxuiBox` has neither). `SelectionList` loses `Eq` (it has `f32` fields).
- `list` stays the short form with no style value, so it is never reorderable; a reorderable list is `section_list` with `SectionList::reorder(true)`. External drops between rows are reported by reorderable lists only.
- `Presentation::Top` places a modal centred across with its top edge at `Overlay::at`'s y (the palette computes its 12%-capped rule inline, D25.2) and `Overlay::max_h` as its height cap; the C sheet record grows at its end (size-checked) by the top and max height. `PxuiPanelConfig.present` 3 reaches a registered body only for a modal the app asked to be `Top`, which needs the grown sheet record (the route refuses present 3 when the record's size does not cover `top`/`max_h`), so an app built before that minor never produces it (minimum-library rule: an older app is served as before); a body must draw a `present` it does not know as 0 (centred).
- Site-map gaps (U2b.9) closed in the shape: `ListResponse::selected` (single-click navigation), `SelectionList::fit()` (sidebar Library group), `SectionPart::Trailing` (sidebar Lists group keeps virtual scrolling with its add row), `CellState::held` (a row drag starts the app's carry), `SearchField::action(ReturnKey)` (the ledger's "return key"); app-written `ListState::SELECTED` is honoured. Swipe-armed looks pick between two consts: no new items. (Palette icons first travelled as private-use `PickArt::Glyph`s; U2b.4.19 replaced that with `PickArt::Draw`.)
- `ColorToken::ACCENT` is `ColorToken(64)`, the first number past the app table, resolved to the theme's accent.
- C route (U2b.1, in place under D28; no parallel exports): `pxui_ui_section_list(ui, keyed, PxuiSectionListDesc, key, build, user, PxuiListResult*)` (fixed and measured rows, the trailer as build index `usize::MAX - 2`), `pxui_ui_selection_list(ui, keyed, PxuiSelectionListDesc, key, cell, user, PxuiListResult*)`, `pxui_ui_table(ui, keyed, PxuiTableDesc, key, cell, user, PxuiTableResult*)` and `pxui_ui_table_head(ui, keyed, PxuiTableHeadDesc, PxuiHeadResult*)`. Columns cross as one span of size-prefixed `PxuiColumn { size, sortable, grip, has_id, title, id, share, w }`, checked in one pass before any effect (record size, 0/1 flags, reserved zero, UTF-8 title of at most 64 KiB, finite non-negative share and width, unique ids with an index standing in for a missing id, at most 64 columns), the widths as one `(id, w)` span of the column count, the row flags as one byte span of at least the row count; a sort naming no column reads as none. The library writes the whole result in one size-checked record. Deleted in the same step: `pxui_ui_list_view`/`_with`/`_rows`, `pxui_ui_table_view`/`_with`/`_keyed`/`_rows`, `pxui_ui_section_list_measured`, the fixed text table and its `table_cell` body kind (later body kinds renumbered).
- The "Sort by Title, ascending" accessible name is composed once per title or sort change into the engine's bounded store (64 sortable headings, round robin past it) and copied while it holds; a warm sort change allocates nothing once the store has its room.
- A click that changes the selection builds the frame again before it is drawn (rows built earlier in the frame were styled from the old flags); the click is reported by the build it happened in. The keyboard steps over `DISABLED` rows; arrows select (never activate, even with `open_on_click`).
- U2b.8 test plan, one test per bug: gone-slot `set_state` (pxui_core `state_on_gone_slot_is_ignored`, plus a wrapper test in both link modes); submenu beside the measured width (`submenu_opens_beside_measured_width`, Menus golden regenerated, listed); `Paint::text_spans` origin doc and a 12-sample CoreText baseline test (`text_spans_baseline_matches_coretext`, |Δ| ≤ 0.5 pt); `motion.slow` (`transition_honours_slow`, `moves_honours_slow`); tooltip `text_at` through a registered body (`tooltip_body_reads_text_at`); layout_scaling: nothing (U0.5 found no cause); the Layout page's popup layer pinned to one draw call (`popup_layer_is_one_draw_call`), fixed before U2b.10.

<!-- surface -->
```text
pxui mod @HEAD
pxui::A11yAction use pxui_types::ffi::A11yAction @HEAD
pxui::A11yBridge impl Drop @HEAD
pxui::A11yBridge struct (_) @HEAD
pxui::A11yBridge::connect fn (name: &str) -> Option<Self> @HEAD
pxui::A11yBridge::fd fn (&self) -> i32 @HEAD
pxui::A11yBridge::pump fn (&mut self, out: &mut dyn FnMut(A11yAction)) -> bool @HEAD
pxui::A11yBridge::remove_surface fn (&mut self, surface: u64) -> bool @U5.9
pxui::A11yBridge::update fn (&mut self, surface: u64, tree: &A11yTree) -> bool @U5.9
pxui::A11yNode use pxui_types::ffi::A11yNode @HEAD
pxui::A11yStates impl Clone @U1b.4
pxui::A11yStates impl Copy @U1b.4
pxui::A11yStates impl Debug @U1b.4
pxui::A11yStates impl Default @U1b.4
pxui::A11yStates impl Eq @U1b.4
pxui::A11yStates impl PartialEq @U1b.4
pxui::A11yStates impl StructuralPartialEq @U1b.4
pxui::A11yStates struct (_) @U1b.4
pxui::A11yStates::ACTIVE const A11yStates @U1b.4
pxui::A11yStates::BUSY const A11yStates @U1b.4
pxui::A11yStates::CHECKABLE const A11yStates @U1b.4
pxui::A11yStates::CHECKED const A11yStates @U1b.4
pxui::A11yStates::DEFAULT const A11yStates @U1b.4
pxui::A11yStates::EDITABLE const A11yStates @U1b.4
pxui::A11yStates::ENABLED const A11yStates @U1b.4
pxui::A11yStates::EXPANDABLE const A11yStates @U1b.4
pxui::A11yStates::EXPANDED const A11yStates @U1b.4
pxui::A11yStates::FOCUSABLE const A11yStates @U1b.4
pxui::A11yStates::FOCUSED const A11yStates @U1b.4
pxui::A11yStates::HORIZONTAL const A11yStates @U1b.4
pxui::A11yStates::MIXED const A11yStates @U1b.4
pxui::A11yStates::MODAL const A11yStates @U1b.4
pxui::A11yStates::MULTILINE const A11yStates @U1b.4
pxui::A11yStates::NONE const A11yStates @U1b.4
pxui::A11yStates::PRESSED const A11yStates @U1b.4
pxui::A11yStates::READ_ONLY const A11yStates @U1b.4
pxui::A11yStates::SELECTABLE const A11yStates @U1b.4
pxui::A11yStates::SELECTED const A11yStates @U1b.4
pxui::A11yStates::VERTICAL const A11yStates @U1b.4
pxui::A11yStates::VISIBLE const A11yStates @U1b.4
pxui::A11yStates::bits fn const (self) -> u32 @U1b.4
pxui::A11yStates::with fn const (self, other: A11yStates) -> A11yStates @U1b.4
pxui::A11yTree impl Clone @U1b.4
pxui::A11yTree impl Copy @U1b.4
pxui::A11yTree impl Debug @U1b.4
pxui::A11yTree struct <'a> @U1b.4
-pxui::A11yTree use pxui_types::ffi::A11yTree @U1b.4
pxui::A11yTree::first_child field u32 @U1b.4
pxui::A11yTree::revision field u64 @U1b.4
pxui::A11yTree::strings field &'a [u8] @U1b.4
pxui::Appearance use pxui_types::Appearance @U1a.3
pxui::AreaCallbacks impl Clone @HEAD
pxui::AreaCallbacks impl Copy @HEAD
pxui::AreaCallbacks struct <'a> @HEAD
pxui::AreaCallbacks::DEFAULT const AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::language fn const (self, language: Language<'a>) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_caret fn const (self, f: &'a dyn Fn(usize)) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_caret_move fn const (self, f: &'a dyn Fn(usize)) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_click fn const (self, f: &'a dyn Fn(usize, bool)) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_context fn const (self, f: &'a dyn Fn((f32, f32))) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_drop fn const (self, f: &'a dyn Fn(usize, DropRef<'_>)) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_scrub fn const (self, f: &'a dyn Fn(ScrubPhase)) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_scrub_edit fn const (self, f: &'a dyn Fn((usize, usize), &str)) -> AreaCallbacks<'a> @HEAD
pxui::AreaCallbacks::on_select fn const (self, f: &'a dyn Fn((usize, usize))) -> AreaCallbacks<'a> @HEAD
pxui::AreaNote impl Clone @HEAD
pxui::AreaNote impl Copy @HEAD
pxui::AreaNote impl Debug @HEAD
pxui::AreaNote impl Eq @HEAD
pxui::AreaNote impl PartialEq @HEAD
pxui::AreaNote impl StructuralPartialEq @HEAD
pxui::AreaNote struct <'a> @HEAD
pxui::AreaNote::new fn const (line: usize, text: &'a str) -> AreaNote<'a> @HEAD
pxui::AreaNote::range fn const (self, start: usize, end: usize) -> AreaNote<'a> @HEAD
pxui::AtlasKind enum @HEAD
pxui::AtlasKind impl Clone @HEAD
pxui::AtlasKind impl Copy @HEAD
pxui::AtlasKind impl Debug @HEAD
pxui::AtlasKind impl Eq @HEAD
pxui::AtlasKind impl PartialEq @HEAD
pxui::AtlasKind impl StructuralPartialEq @HEAD
pxui::AtlasKind::Color variant @HEAD
pxui::AtlasKind::Color2 variant @HEAD
pxui::AtlasKind::Mask variant @HEAD
pxui::AtlasKind::Mask2 variant @HEAD
pxui::Badge impl Clone @U1b.5
pxui::Badge impl Copy @U1b.5
pxui::Badge impl Debug @U1b.5
pxui::Badge struct @U1b.5
pxui::Badge::DEFAULT const Badge @U1b.5
pxui::Badge::variant fn const (self, variant: u16) -> Badge @U1b.5
pxui::BodyId use pxui_types::theme::BodyId @HEAD
pxui::BodyKind use pxui_types::theme::BodyKind @HEAD
pxui::BracketsFn type <'a>= dyn Fn(&str) -> Vec<(usize, usize)> + 'a @HEAD
pxui::Button impl Clone @HEAD
pxui::Button impl Copy @HEAD
pxui::Button struct <'a> @HEAD
pxui::Button::DEFAULT const Button<'a> @HEAD
pxui::Button::accessory fn const (self, accessory: &'a str) -> Button<'a> @HEAD
pxui::Button::accessory_label fn const (self, style: Label) -> Button<'a> @HEAD
pxui::Button::align fn const (self, align: Justify) -> Button<'a> @HEAD
pxui::Button::axis fn const (self, axis: Axis) -> Button<'a> @HEAD
pxui::Button::centered fn const (self) -> Button<'a> @HEAD
pxui::Button::content_align fn const (self, align: Along) -> Button<'a> @HEAD
pxui::Button::danger fn const (self) -> Button<'a> @HEAD
pxui::Button::detail fn const (self, detail: &'a str) -> Button<'a> @HEAD
pxui::Button::detail_label fn const (self, style: Label) -> Button<'a> @HEAD
pxui::Button::disabled fn const (self, disabled: bool) -> Button<'a> @U1b.6
-pxui::Button::enabled fn const (self, enabled: bool) -> Button<'a> @U1b.6
pxui::Button::focus_on_click fn const (self, focus: bool) -> Button<'a> @HEAD
pxui::Button::gap fn const (self, gap: f32) -> Button<'a> @HEAD
pxui::Button::icon fn const (self, icon: Icon) -> Button<'a> @HEAD
pxui::Button::icon_size fn const (self, points: f32) -> Button<'a> @U2b.7
pxui::Button::label fn const (self, style: Label) -> Button<'a> @HEAD
pxui::Button::look fn const (self, look: &'a States) -> Button<'a> @HEAD
pxui::Button::name fn const (self, name: &'a str) -> Button<'a> @HEAD
pxui::Button::pad fn const (self, pad: Edges) -> Button<'a> @HEAD
pxui::Button::selected fn const (self, selected: bool) -> Button<'a> @HEAD
pxui::Button::size fn const (self, w: Size, h: Size) -> Button<'a> @HEAD
pxui::Button::sized fn const (w: f32, h: f32) -> Button<'a> @HEAD
pxui::Button::variant fn const (self, variant: u16) -> Button<'a> @HEAD
pxui::Bytes impl Clone @HEAD
pxui::Bytes impl Copy @HEAD
pxui::Bytes impl Debug @HEAD
pxui::Bytes impl Default @HEAD
pxui::Bytes impl Eq @HEAD
pxui::Bytes impl PartialEq @HEAD
pxui::Bytes impl StructuralPartialEq @HEAD
pxui::Bytes struct @HEAD
pxui::Bytes::atlas field i64 @HEAD
pxui::Bytes::glyph_lists field i64 @HEAD
pxui::Bytes::text_undo field i64 @HEAD
pxui::COUNTER_ALLOCATIONS const u32 @HEAD
pxui::Capitalize use pxui_types::Capitalize @HEAD
pxui::CellState impl Clone @U2b.1
pxui::CellState impl Copy @U2b.1
pxui::CellState impl Debug @U2b.1
pxui::CellState impl Default @U2b.1
pxui::CellState struct @U2b.1
pxui::CellState::enabled field bool @U2b.1
pxui::CellState::focused field bool @U2b.1
pxui::CellState::handle field Handle @U2b.1
pxui::CellState::held field bool @U2b.1
pxui::CellState::selected field bool @U2b.1
pxui::CellState::y field f32 @U2b.1
pxui::Checkbox use pxui_types::ui::Checkbox @HEAD
-pxui::Choice use pxui_types::ui::Choice @U2a.2
pxui::Chosen use pxui_types::ui::Chosen @HEAD
pxui::ColorizeFn type <'a>= dyn Fn(&str) -> Vec<(usize, usize, Rgba)> + 'a @HEAD
pxui::Column impl Clone @U2b.1
pxui::Column impl Copy @U2b.1
pxui::Column impl Debug @U2b.1
pxui::Column impl PartialEq @U2b.1
pxui::Column impl StructuralPartialEq @U2b.1
pxui::Column struct <'a> @U2b.1
pxui::Column::grip fn const (self, grip: bool) -> Column<'a> @U2b.1
pxui::Column::id fn const (self, id: i64) -> Column<'a> @U2b.1
pxui::Column::new fn const (title: &'a str) -> Column<'a> @U2b.1
pxui::Column::share fn const (self, share: f32) -> Column<'a> @U2b.1
pxui::Column::sortable fn const (self, sortable: bool) -> Column<'a> @U2b.1
pxui::Column::w_px fn const (self, points: f32) -> Column<'a> @U2b.1
pxui::Commands use pxui_types::Commands @HEAD
pxui::CompleteFn type <'a>= dyn Fn(&str, usize) -> Vec<Completion> + 'a @HEAD
pxui::ContextAtFn type <'a>= dyn Fn(&str, usize) -> CodeContext + 'a @HEAD
pxui::DescribeFn type <'a>= dyn Fn(&str, usize, &mut String) -> Option<(usize, usize)> + 'a @U5.9
pxui::Dialog use pxui_types::ui::Dialog @HEAD
pxui::DialogButton use pxui_types::ui::DialogButton @HEAD
pxui::Disclosure impl Clone @U2a.3
pxui::Disclosure impl Copy @U2a.3
pxui::Disclosure impl Debug @U2a.3
pxui::Disclosure struct @U2a.3
pxui::Disclosure::DEFAULT const Disclosure @U2a.3
pxui::Disclosure::variant fn const (self, variant: u16) -> Disclosure @U2a.3
-pxui::DismissOpts impl Clone @U2a.4
-pxui::DismissOpts impl Copy @U2a.4
-pxui::DismissOpts impl Debug @U2a.4
-pxui::DismissOpts impl Default @U2a.4
-pxui::DismissOpts impl PartialEq @U2a.4
-pxui::DismissOpts impl StructuralPartialEq @U2a.4
-pxui::DismissOpts struct @U2a.4
pxui::DismissOpts use pxui_types::ui::DismissOpts @U2a.4
-pxui::DismissOpts::DEFAULT const DismissOpts @U2a.4
-pxui::DismissOpts::fraction fn const (self, fraction: f32) -> DismissOpts @U2a.4
-pxui::DismissOpts::velocity fn const (self, velocity: f32) -> DismissOpts @U2a.4
pxui::DropRef enum <'a> @HEAD
pxui::DropRef impl Clone @HEAD
pxui::DropRef impl Copy @HEAD
pxui::DropRef impl Debug @HEAD
pxui::DropRef impl Eq @HEAD
pxui::DropRef impl PartialEq @HEAD
pxui::DropRef impl StructuralPartialEq @HEAD
pxui::DropRef::Dropped variant (PayloadRef<'a>) @HEAD
pxui::DropRef::Hover variant (PayloadRef<'a>) @HEAD
pxui::Dropdown use pxui_types::ui::Dropdown @HEAD
pxui::Ellipsis use pxui_types::text::Ellipsis @U1a.1
pxui::Environment use pxui_types::Environment @HEAD
pxui::Error enum @HEAD
pxui::Error impl Clone @HEAD
pxui::Error impl Copy @HEAD
pxui::Error impl Debug @HEAD
pxui::Error impl Display @HEAD
pxui::Error impl Eq @HEAD
pxui::Error impl Error @HEAD
pxui::Error impl PartialEq @HEAD
pxui::Error impl StructuralPartialEq @HEAD
pxui::Error::Allocation variant @HEAD
pxui::Error::Engine variant @HEAD
pxui::Error::Gpu variant @HEAD
pxui::Error::Host variant (&'static str) @U1a.3
pxui::Error::InvalidInput variant @HEAD
pxui::Error::Panic variant @HEAD
pxui::Error::Stale variant @HEAD
pxui::Error::State variant @HEAD
pxui::Error::Version variant @U1a.3
pxui::Error::from_code fn (code: i32) -> Error @HEAD
pxui::Event use pxui_types::Event @HEAD
pxui::Feature enum @HEAD
pxui::Feature impl Clone @HEAD
pxui::Feature impl Copy @HEAD
pxui::Feature impl Debug @HEAD
pxui::Feature impl Eq @HEAD
pxui::Feature impl PartialEq @HEAD
pxui::Feature impl StructuralPartialEq @HEAD
pxui::Feature::A11yBridge variant = pxui_types::feature::A11Y_BRIDGE @HEAD
pxui::Feature::ExtensionSet variant = pxui_types::feature::EXTENSION_SET @HEAD
pxui::Feature::Metal variant = pxui_types::feature::METAL @HEAD
-pxui::Feature::SegmentedResponse variant = pxui_types::feature::SEGMENTED_RESPONSE @U1b.1
pxui::Feature::Vulkan variant = pxui_types::feature::VULKAN @HEAD
pxui::Fill impl Clone @HEAD
pxui::Fill impl Copy @HEAD
pxui::Fill impl Debug @HEAD
pxui::Fill impl PartialEq @HEAD
pxui::Fill impl StructuralPartialEq @HEAD
pxui::Fill struct @HEAD
pxui::Fill::DEFAULT const Fill @HEAD
pxui::Fill::fractional fn const (self) -> Fill @HEAD
pxui::Fill::new fn const (color: Rgba) -> Fill @HEAD
pxui::Flags use pxui_types::Flags @U1a.1
pxui::FontData impl Clone @U1a.3
pxui::FontData impl Copy @U1a.3
pxui::FontData struct <'a>(&'a [u8]) @U1a.3
pxui::Frame impl Deref @HEAD
pxui::Frame struct <'a> @HEAD
pxui::FrameIn impl Clone @U1a.3
pxui::FrameIn impl Copy @U1a.3
pxui::FrameIn impl Debug @U1a.3
pxui::FrameIn impl Default @U1a.3
pxui::FrameIn struct <'a> @U1a.3
pxui::FrameIn::appearance fn const (self, appearance: Appearance) -> FrameIn<'a> @U1a.3
pxui::FrameIn::environment fn const (self, environment: Environment) -> FrameIn<'a> @U1a.3
pxui::FrameIn::events fn const (self, events: &'a [Event]) -> FrameIn<'a> @U5.1
pxui::FrameIn::keyboard fn const (self, keyboard: Keyboard) -> FrameIn<'a> @U1a.3
pxui::FrameIn::new fn const (width: f32, height: f32) -> FrameIn<'a> @U1a.3
pxui::FrameIn::reduced_motion fn const (self, on: bool) -> FrameIn<'a> @U1a.3
pxui::FrameIn::scale fn const (self, scale: f32) -> FrameIn<'a> @U1a.3
pxui::FrameIn::surface fn const (self, surface: u64) -> FrameIn<'a> @U5.1
pxui::FrameIn::time fn const (self, time: f64) -> FrameIn<'a> @U1a.3
pxui::FrameIn::wire field FrameIn @U1a.3
pxui::FrameOut use pxui_types::FrameOut @HEAD
pxui::GestureOpts impl Clone @HEAD
pxui::GestureOpts impl Copy @HEAD
pxui::GestureOpts impl Debug @HEAD
pxui::GestureOpts impl Default @HEAD
pxui::GestureOpts impl PartialEq @HEAD
pxui::GestureOpts impl StructuralPartialEq @HEAD
pxui::GestureOpts struct @HEAD
pxui::GestureOpts::DEFAULT const GestureOpts @HEAD
pxui::GestureOpts::dismiss fn const (self, dismiss: DismissOpts) -> GestureOpts @HEAD
pxui::GestureOpts::edge_back fn const (self) -> GestureOpts @HEAD
pxui::GestureOpts::long_press fn const (self) -> GestureOpts @HEAD
pxui::GestureOpts::pull_to_refresh fn const (self) -> GestureOpts @HEAD
pxui::GestureOpts::refresh fn const (self, points: f32) -> GestureOpts @HEAD
pxui::GestureOpts::row_swipe fn const (self, width: f32) -> GestureOpts @HEAD
pxui::GestureOpts::swipe_actions fn const (self, actions: SwipeActions) -> GestureOpts @HEAD
pxui::GestureOpts::touch_drag fn const (self) -> GestureOpts @HEAD
pxui::GestureState impl Clone @HEAD
pxui::GestureState impl Copy @HEAD
pxui::GestureState impl Debug @HEAD
pxui::GestureState impl Default @HEAD
pxui::GestureState impl PartialEq @HEAD
pxui::GestureState impl StructuralPartialEq @HEAD
pxui::GestureState struct @HEAD
pxui::GestureState::back field f32 @HEAD
pxui::GestureState::back_active field bool @HEAD
pxui::GestureState::back_cancelled field bool @HEAD
pxui::GestureState::back_committed field bool @HEAD
pxui::GestureState::dismiss field f32 @HEAD
pxui::GestureState::dismiss_cancelled field bool @HEAD
pxui::GestureState::dismiss_committed field bool @HEAD
pxui::GestureState::dismissing field bool @HEAD
pxui::GestureState::highlighted field bool @HEAD
pxui::GestureState::long_pressed field bool @HEAD
pxui::GestureState::pull field f32 @HEAD
pxui::GestureState::refresh_fired field bool @HEAD
pxui::GestureState::refreshing field bool @HEAD
pxui::GestureState::swipe field f32 @HEAD
pxui::GestureState::swipe_commit_leading field bool @HEAD
pxui::GestureState::swipe_commit_trailing field bool @HEAD
pxui::GestureState::swipe_open field bool @HEAD
pxui::GestureState::swipe_open_leading field bool @HEAD
pxui::GestureState::swipe_open_trailing field bool @HEAD
pxui::GestureState::swiping field bool @HEAD
pxui::Gpu use pxui_types::Gpu @HEAD
pxui::GridList impl Clone @U1b.5
pxui::GridList impl Copy @U1b.5
pxui::GridList impl Debug @U1b.5
pxui::GridList struct @U1b.5
pxui::GridList::DEFAULT const GridList @U1b.5
pxui::GridList::h_px fn const (self, points: f32) -> GridList @U1b.5
pxui::GridList::w_px fn const (self, points: f32) -> GridList @U1b.5
pxui::Handle type = PxuiBox @HEAD
pxui::HeadResponse impl Clone @U2b.1
pxui::HeadResponse impl Copy @U2b.1
pxui::HeadResponse impl Debug @U2b.1
pxui::HeadResponse struct @U2b.1
pxui::HeadResponse::resized field bool @U2b.1
pxui::HeadResponse::response field Response @U2b.1
pxui::HeadResponse::sorted field bool @U2b.1
pxui::Icon enum @HEAD
pxui::Icon impl Clone @HEAD
pxui::Icon impl Copy @HEAD
pxui::Icon::Draw variant (fn(&mut Paint<'_>, Rect, Rgba, u32), u32) @HEAD
pxui::Icon::Glyph variant (char) @HEAD
pxui::Icon::draw fn const (f: fn(&mut Paint<'_>, Rect, Rgba, u32), word: u32) -> Icon @HEAD
pxui::Icon::glyph fn const (ch: char) -> Icon @HEAD
pxui::IconButton impl Clone @HEAD
pxui::IconButton impl Copy @HEAD
pxui::IconButton struct <'a> @HEAD
pxui::IconButton::DEFAULT const IconButton<'a> @HEAD
pxui::IconButton::disabled fn const (self, disabled: bool) -> IconButton<'a> @U1b.6
-pxui::IconButton::enabled fn const (self, enabled: bool) -> IconButton<'a> @U1b.6
pxui::IconButton::look fn const (self, look: &'a States) -> IconButton<'a> @HEAD
pxui::IconButton::press_curves fn const (self, curves: &'static [Curve; 2]) -> IconButton<'a> @HEAD
pxui::IconButton::press_scale fn const (self, scale: f32) -> IconButton<'a> @HEAD
pxui::IconButton::selected fn const (self, on: bool) -> IconButton<'a> @HEAD
pxui::IconButton::size fn const (self, w: Size, h: Size) -> IconButton<'a> @HEAD
pxui::IconButton::sized fn const (w: f32, h: f32) -> IconButton<'a> @HEAD
pxui::IconButton::variant fn const (self, variant: u16) -> IconButton<'a> @HEAD
pxui::Image use pxui_types::Image @HEAD
pxui::ImageStyle impl Clone @HEAD
pxui::ImageStyle impl Copy @HEAD
pxui::ImageStyle impl Debug @HEAD
pxui::ImageStyle struct @HEAD
pxui::ImageStyle::DEFAULT const ImageStyle @HEAD
pxui::ImageStyle::fit fn const (self, fit: Fit) -> ImageStyle @HEAD
pxui::ImageStyle::h fn const (self, size: Size) -> ImageStyle @HEAD
pxui::ImageStyle::h_px fn const (self, points: f32) -> ImageStyle @HEAD
pxui::ImageStyle::tint fn const (self, tint: Rgba) -> ImageStyle @U1b.2
pxui::ImageStyle::w fn const (self, size: Size) -> ImageStyle @HEAD
pxui::ImageStyle::w_px fn const (self, points: f32) -> ImageStyle @HEAD
pxui::Ime impl Clone @U1b.2
pxui::Ime impl Copy @U1b.2
pxui::Ime impl Debug @U1b.2
pxui::Ime impl PartialEq @U1b.2
pxui::Ime impl StructuralPartialEq @U1b.2
pxui::Ime struct @U1b.2
pxui::Ime::caret field f32 @U1b.2
pxui::Ime::rect field Rect @U1b.2
pxui::IndentFn type <'a>= dyn Fn(&str, usize) -> String + 'a @HEAD
pxui::Input use pxui_types::ffi::Input @HEAD
pxui::Key use author::Key @HEAD
pxui::KeyCode enum @U1a.4
pxui::KeyCode impl Clone @U1a.4
pxui::KeyCode impl Copy @U1a.4
pxui::KeyCode impl Debug @U1a.4
pxui::KeyCode impl Eq @U1a.4
pxui::KeyCode impl PartialEq @U1a.4
pxui::KeyCode impl StructuralPartialEq @U1a.4
pxui::KeyCode::Alt variant @U1a.4
pxui::KeyCode::ArrowDown variant @U1a.4
pxui::KeyCode::ArrowLeft variant @U1a.4
pxui::KeyCode::ArrowRight variant @U1a.4
pxui::KeyCode::ArrowUp variant @U1a.4
pxui::KeyCode::Backspace variant @U1a.4
pxui::KeyCode::Char variant (char) @U1a.4
pxui::KeyCode::Ctrl variant @U1a.4
pxui::KeyCode::Delete variant @U1a.4
pxui::KeyCode::End variant @U1a.4
pxui::KeyCode::Enter variant @U1a.4
pxui::KeyCode::Escape variant @U1a.4
pxui::KeyCode::F1 variant @U1a.4
pxui::KeyCode::F10 variant @U1a.4
pxui::KeyCode::F11 variant @U1a.4
pxui::KeyCode::F12 variant @U1a.4
pxui::KeyCode::F2 variant @U1a.4
pxui::KeyCode::F3 variant @U1a.4
pxui::KeyCode::F4 variant @U1a.4
pxui::KeyCode::F5 variant @U1a.4
pxui::KeyCode::F6 variant @U1a.4
pxui::KeyCode::F7 variant @U1a.4
pxui::KeyCode::F8 variant @U1a.4
pxui::KeyCode::F9 variant @U1a.4
pxui::KeyCode::Home variant @U1a.4
pxui::KeyCode::Insert variant @U1a.4
pxui::KeyCode::Meta variant @U1a.4
pxui::KeyCode::PageDown variant @U1a.4
pxui::KeyCode::PageUp variant @U1a.4
pxui::KeyCode::Shift variant @U1a.4
pxui::KeyCode::Space variant @U1a.4
pxui::KeyCode::Tab variant @U1a.4
pxui::Keyboard use pxui_types::Keyboard @HEAD
pxui::Keyframes use pxui_types::ui::Keyframes @HEAD
pxui::Label use pxui_types::Label @HEAD
pxui::Language impl Clone @HEAD
pxui::Language impl Copy @HEAD
pxui::Language struct <'a> @HEAD
pxui::Language::DEFAULT const Language<'a> @HEAD
pxui::Language::brackets fn const (self, f: &'a BracketsFn<'a>) -> Language<'a> @HEAD
pxui::Language::colorize fn const (self, f: &'a ColorizeFn<'a>) -> Language<'a> @HEAD
pxui::Language::comment fn const (self, prefix: &'a str) -> Language<'a> @HEAD
pxui::Language::complete fn const (self, f: &'a CompleteFn<'a>) -> Language<'a> @HEAD
pxui::Language::context_at fn const (self, f: &'a ContextAtFn<'a>) -> Language<'a> @HEAD
pxui::Language::describe fn const (self, f: &'a DescribeFn<'a>) -> Language<'a> @HEAD
pxui::Language::indent fn const (self, f: &'a IndentFn<'a>) -> Language<'a> @HEAD
pxui::Language::number_at fn const (self, f: &'a NumberAtFn<'a>) -> Language<'a> @HEAD
pxui::Language::pairs fn const (self, pairs: &'a [(char, char)]) -> Language<'a> @HEAD
pxui::Language::rewrite fn const (self, f: &'a RewriteFn<'a>) -> Language<'a> @HEAD
pxui::Layer impl Clone @HEAD
pxui::Layer impl Copy @HEAD
pxui::Layer impl Debug @HEAD
pxui::Layer struct <'a> @HEAD
pxui::Layer::breaks field usize @HEAD
pxui::Layer::dirty field &'a [PxuiDirty] @HEAD
pxui::Layer::id field u32 @HEAD
pxui::Layer::instances field &'a [Instance] @HEAD
pxui::Layer::key field i64 @HEAD
pxui::Layers impl Iterator @U1a.3
pxui::Layers struct <'a> @U1a.3
pxui::LineField impl Clone @HEAD
pxui::LineField impl Copy @HEAD
pxui::LineField impl Debug @HEAD
pxui::LineField impl Eq @HEAD
pxui::LineField impl PartialEq @HEAD
pxui::LineField impl StructuralPartialEq @HEAD
pxui::LineField struct <'a> @HEAD
pxui::LineField::DEFAULT const LineField<'a> @HEAD
pxui::LineField::grab fn const (self, on: bool) -> LineField<'a> @HEAD
pxui::LineField::name fn const (self, name: &'a str) -> LineField<'a> @HEAD
pxui::LineField::placeholder fn const (self, text: &'a str) -> LineField<'a> @HEAD
pxui::LineFieldResponse impl Clone @HEAD
pxui::LineFieldResponse impl Copy @HEAD
pxui::LineFieldResponse impl Debug @HEAD
pxui::LineFieldResponse struct <'a> @HEAD
pxui::LineFieldResponse::changed field Option<&'a str> @HEAD
pxui::LineFieldResponse::response field Response @HEAD
pxui::LineFieldResponse::step field LineStep @HEAD
pxui::ListResponse use pxui_types::ui::ListResponse @HEAD
pxui::ListState impl Clone @U2b.1
pxui::ListState impl Debug @U2b.1
pxui::ListState impl Default @U2b.1
pxui::ListState impl Eq @U2b.1
pxui::ListState impl PartialEq @U2b.1
pxui::ListState impl StructuralPartialEq @U2b.1
pxui::ListState struct @U2b.1
pxui::ListState::DISABLED const u8 @U2b.1
pxui::ListState::LIVE const u8 @U2b.4.13
pxui::ListState::NO_DROP const u8 @U2b.4.21
pxui::ListState::SELECTED const u8 @U2b.1
pxui::ListState::anchor field Option<usize> @U2b.1
pxui::ListState::new fn const () -> ListState @U2b.1
pxui::ListState::revision field Option<u64> @U2b.4.13
pxui::ListState::rows field Vec<u8> @U2b.1
pxui::Menu use pxui_types::ui::Menu @HEAD
pxui::MenuBar impl Clone @U1b.1
pxui::MenuBar impl Copy @U1b.1
pxui::MenuBar impl Debug @U1b.1
pxui::MenuBar struct @U1b.1
pxui::MenuBar::DEFAULT const MenuBar @U1b.1
pxui::MenuBar::variant fn const (self, variant: u16) -> MenuBar @U1b.1
pxui::MenuBarMenu impl Clone @U1b.1
pxui::MenuBarMenu impl Copy @U1b.1
pxui::MenuBarMenu impl Debug @U1b.1
pxui::MenuBarMenu struct <'a> @U1b.1
pxui::MenuBarMenu::new fn const (title: &'a str, items: &'a [MenuItem<'a>]) -> MenuBarMenu<'a> @U1b.1
pxui::MenuItem use pxui_types::ui::MenuItem @HEAD
pxui::Mods impl Clone @U1a.4
pxui::Mods impl Copy @U1a.4
pxui::Mods impl Debug @U1a.4
pxui::Mods impl Eq @U1a.4
pxui::Mods impl PartialEq @U1a.4
pxui::Mods impl StructuralPartialEq @U1a.4
pxui::Mods struct (_) @U1a.4
pxui::Mods::ALT const Mods @U1a.4
pxui::Mods::ANY const Mods @U1a.4
pxui::Mods::CTRL const Mods @U1a.4
pxui::Mods::META const Mods @U1a.4
pxui::Mods::NONE const Mods @U1a.4
pxui::Mods::SHIFT const Mods @U1a.4
pxui::Mods::with fn const (self, other: Mods) -> Mods @U1a.4
pxui::Node use pxui_types::Node @HEAD
pxui::NodeFields impl Clone @HEAD
pxui::NodeFields impl Copy @HEAD
pxui::NodeFields impl Debug @HEAD
pxui::NodeFields impl PartialEq @HEAD
pxui::NodeFields impl StructuralPartialEq @HEAD
pxui::NodeFields struct @HEAD
pxui::NodeFields::inset field Edges @HEAD
pxui::NodeFields::look field Look @HEAD
pxui::NodeFields::mark field Mark @HEAD
pxui::NumberAtFn type <'a>= dyn Fn(&str, usize) -> Option<(usize, usize)> + 'a @HEAD
pxui::Output use pxui_types::ffi::Output @HEAD
pxui::Overlay use pxui_types::ui::Overlay @HEAD
pxui::OverlayResponse impl Clone @U1b.1
pxui::OverlayResponse impl Copy @U1b.1
pxui::OverlayResponse impl Debug @U1b.1
pxui::OverlayResponse struct @U1b.1
pxui::OverlayResponse::handle field Handle @U1b.1
pxui::OverlayResponse::open field bool @U1b.1
pxui::Paint struct <'a> @HEAD
pxui::Paint::brackets fn (&mut self, rect: Rect, offset: f32, length: f32, stroke: Stroke) @HEAD
pxui::Paint::chevron fn (&mut self, at: (f32, f32), direction: Dir, stroke: Stroke) @HEAD
pxui::Paint::circle fn (&mut self, at: (f32, f32), radius: f32, fill: Option<Fill>, stroke: Option<Stroke>) @HEAD
pxui::Paint::clip_radius fn (&self) -> f32 @HEAD
pxui::Paint::color fn (&self, token: ColorToken) -> Rgba @HEAD
pxui::Paint::cubic fn (&mut self, points: [(f32, f32); 4], stroke: Stroke) @HEAD
pxui::Paint::curve fn (&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32), stroke: Stroke) @HEAD
pxui::Paint::dashed fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @HEAD
pxui::Paint::dashed_rect fn (&mut self, rect: Rect, stroke: Stroke) @HEAD
pxui::Paint::density fn (&self) -> f32 @U1b.3
-pxui::Paint::density fn (&self) -> i64 @U1b.3
pxui::Paint::fill fn (&mut self, rect: Rect, fill: Fill) @HEAD
pxui::Paint::flag fn (&mut self, at: (f32, f32), round: bool, on: bool) @HEAD
pxui::Paint::frame fn (&mut self, rect: Rect, stroke: Stroke) @HEAD
pxui::Paint::grid fn (&mut self, rect: Rect, origin: (f32, f32), spacing: f32, dot: f32, fill: Fill) @HEAD
pxui::Paint::icon fn (&mut self, at: (f32, f32), font: u16, size: f32, ch: char, color: Rgba) -> f32 @HEAD
-pxui::Paint::image fn (&mut self, rect: Rect, image: &Image, fit_mode: Fit, tint: Rgba) -> Result<(), Error> @U1b.2
pxui::Paint::image fn (&mut self, rect: Rect, image: &Image, style: &ImageStyle) -> Result<(), Error> @U1b.2
pxui::Paint::line fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @HEAD
pxui::Paint::rtl fn (&self) -> bool @HEAD
pxui::Paint::scroll_y fn (&mut self, slot: u64) -> f32 @HEAD
pxui::Paint::stroke fn (&mut self, rect: Rect, stroke: Stroke) @HEAD
-pxui::Paint::text fn (&mut self, at: (f32, f32), style: &Label, width: f32, s: &str) -> TextMetrics @U1b.3
pxui::Paint::text fn (&mut self, at: Point, style: Label, width: f32, s: &str) -> TextMetrics @U1b.3
-pxui::Paint::text_chars fn (&mut self, at: (f32, f32), style: &Label, s: &str) -> f32 @U1b.3
pxui::Paint::text_chars fn (&mut self, at: Point, style: Label, s: &str) -> f32 @U1b.3
-pxui::Paint::text_metrics fn (&mut self, style: &TextStyle, width: f32, s: &str) -> TextMetrics @U1b.3
pxui::Paint::text_service fn (&mut self) -> UiText<'_> @U1b.3
-pxui::Paint::text_spans fn (&mut self, at: (f32, f32), style: &TextStyle, width: f32, text: &str, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1b.3
pxui::Paint::text_spans fn (&mut self, at: Point, style: Label, width: f32, text: &str, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1b.3
-pxui::Paint::theme fn (&self) -> Tokens @U1b.4
pxui::Paint::tokens fn (&self) -> Tokens @U1b.4
pxui::Paint::triangle fn (&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), fill: Fill) @HEAD
pxui::Part use pxui_types::theme::Part @HEAD
pxui::Pass use pxui_types::Pass @HEAD
pxui::PayloadRef impl Clone @HEAD
pxui::PayloadRef impl Copy @HEAD
pxui::PayloadRef impl Debug @HEAD
pxui::PayloadRef impl Eq @HEAD
pxui::PayloadRef impl PartialEq @HEAD
pxui::PayloadRef impl StructuralPartialEq @HEAD
pxui::PayloadRef struct <'a> @HEAD
pxui::PayloadRef::kind field &'a str @HEAD
pxui::PayloadRef::value field &'a str @HEAD
pxui::Pick use pxui_types::ui::Pick @HEAD
pxui::PickArt use pxui_types::ui::PickArt @U2a.4
pxui::PickResponse impl Clone @U1b.1
pxui::PickResponse impl Copy @U1b.1
pxui::PickResponse impl Debug @U1b.1
pxui::PickResponse struct @U1b.1
pxui::PickResponse::chosen field Option<usize> @U1b.1
pxui::PickResponse::chosen_id field Option<i64> @U1b.6
pxui::PickResponse::response field Response @U1b.1
pxui::PickResponse::secondary field Option<usize> @U1b.1
pxui::PickRow use pxui_types::ui::PickRow @U2a.4
pxui::PickRows use pxui_types::ui::PickRows @HEAD
pxui::Picker impl Clone @U1b.1
pxui::Picker impl Copy @U1b.1
pxui::Picker struct <'a> @U1b.1
pxui::Picker::DEFAULT const Picker<'a> @U1b.1
pxui::Picker::at_rest fn const (self, on: bool) -> Picker<'a> @U1b.1
pxui::Picker::limit fn const (self, rows: usize) -> Picker<'a> @U1b.1
pxui::Picker::mark fn const (self, f: &'a dyn Fn(usize) -> Option<Rgba>) -> Picker<'a> @U1b.1
pxui::Picker::image fn const (self, f: &'a dyn Fn(usize) -> Option<Image>) -> Picker<'a> @U2b.4.15
pxui::Picker::off fn const (self, f: &'a dyn Fn(usize) -> bool) -> Picker<'a> @U1b.1
pxui::Picker::slash fn const (self, on: bool) -> Picker<'a> @U1b.1
-pxui::PickerOpts impl Clone @U1b.1
-pxui::PickerOpts impl Copy @U1b.1
-pxui::PickerOpts struct <'a> @U1b.1
-pxui::PickerOpts::DEFAULT const PickerOpts<'a> @U1b.1
-pxui::PickerOpts::at_rest fn const (self, on: bool) -> PickerOpts<'a> @U1b.1
-pxui::PickerOpts::limit fn const (self, rows: usize) -> PickerOpts<'a> @U1b.1
-pxui::PickerOpts::mark fn const (self, f: &'a dyn Fn(usize) -> Option<Rgba>) -> PickerOpts<'a> @U1b.1
-pxui::PickerOpts::off fn const (self, f: &'a dyn Fn(usize) -> bool) -> PickerOpts<'a> @U1b.1
-pxui::PickerOpts::slash fn const (self, on: bool) -> PickerOpts<'a> @U1b.1
pxui::PickerResponse use pxui_types::ui::PickerResponse @HEAD
pxui::Platform enum @HEAD
pxui::Platform impl Clone @HEAD
pxui::Platform impl Copy @HEAD
pxui::Platform impl Debug @HEAD
pxui::Platform impl Default @HEAD
pxui::Platform impl Eq @HEAD
pxui::Platform impl PartialEq @HEAD
pxui::Platform impl StructuralPartialEq @HEAD
pxui::Platform::Apple variant @HEAD
pxui::Platform::Other variant @HEAD
pxui::PlatformOut use pxui_types::ffi::PlatformOut @HEAD
pxui::Point use pxui_types::geom::Point @U1a.1
pxui::PreparedText impl Drop @HEAD
pxui::PreparedText impl Send @HEAD
pxui::PreparedText struct @HEAD
pxui::PreparedText::metrics fn (&self) -> TextMetrics @HEAD
pxui::Presentation use pxui_types::ui::Presentation @U2a.4
pxui::Pressable impl Clone @HEAD
pxui::Pressable impl Copy @HEAD
pxui::Pressable impl Debug @HEAD
pxui::Pressable struct <'a> @HEAD
pxui::Pressable::DEFAULT const Pressable<'a> @HEAD
pxui::Pressable::disabled fn const (self, disabled: bool) -> Pressable<'a> @U1b.6
-pxui::Pressable::enabled fn const (self, enabled: bool) -> Pressable<'a> @U1b.6
pxui::Pressable::focus_on_click fn const (self, focus: bool) -> Pressable<'a> @HEAD
pxui::Pressable::look fn const (self, look: &'a States) -> Pressable<'a> @HEAD
pxui::Pressable::selected fn const (self, selected: bool) -> Pressable<'a> @HEAD
pxui::Pressable::tab_stop fn const (self, tab_stop: bool) -> Pressable<'a> @HEAD
pxui::Progress impl Clone @U1b.5
pxui::Progress impl Copy @U1b.5
pxui::Progress impl Debug @U1b.5
pxui::Progress struct @U1b.5
pxui::Progress::DEFAULT const Progress @U1b.5
pxui::Progress::variant fn const (self, variant: u16) -> Progress @U1b.5
pxui::PxuiPanel use pxui_types::PxuiPanel @HEAD
pxui::PxuiTargetDesc use pxui_types::PxuiTargetDesc @HEAD
pxui::PxuiUiDesc use pxui_types::PxuiUiDesc @HEAD
pxui::Radio impl Clone @U1b.1
pxui::Radio impl Copy @U1b.1
pxui::Radio impl Debug @U1b.1
pxui::Radio struct <'a> @U1b.1
pxui::Radio::DEFAULT const Radio<'a> @U1b.1
pxui::Radio::name fn const (self, name: &'a str) -> Radio<'a> @U1b.1
pxui::Radio::variant fn const (self, variant: u16) -> Radio<'a> @U1b.1
pxui::Recorder type <'a>= &'a mut dyn FnMut(&Image) -> Result<(), Error> @HEAD
pxui::Rect use pxui_types::geom::Rect @U1a.1
pxui::Renderer impl Drop @HEAD
pxui::Renderer struct @HEAD
pxui::Renderer::atlas fn (&self, kind: AtlasKind) -> Result<Image, Error> @HEAD
pxui::Renderer::draw fn (&mut self, pass: &Pass) -> Result<(), Error> @HEAD
pxui::Renderer::draw_calls fn (&self) -> u64 @HEAD
pxui::Renderer::draw_layer fn (&mut self, pass: &Pass, layer: u32, split: &mut dyn FnMut(Recorder<'_>) -> Result<Pass, Error>) -> Result<(), Error> @HEAD
pxui::Renderer::forget_image fn (&mut self, image: &Image, completed: u64) -> Result<(), Error> @HEAD
pxui::Renderer::new fn (ui: &Ui, gpu: Gpu, target: TargetDesc) -> Result<Self, Error> @U1a.3
pxui::Renderer::prepare fn (&mut self, commands: &Commands, frame: u64, completed: u64) -> Result<(), Error> @HEAD
pxui::Renderer::resize fn (&mut self, width: u32, height: u32) -> Result<(), Error> @HEAD
pxui::Repeat use pxui_types::ui::Repeat @HEAD
pxui::Response impl Clone @HEAD
pxui::Response impl Copy @HEAD
pxui::Response impl Debug @HEAD
pxui::Response struct @HEAD
pxui::Response::activated field bool @HEAD
pxui::Response::changed field bool @HEAD
pxui::Response::clicked field bool @HEAD
pxui::Response::focused field bool @HEAD
pxui::Response::handle field Handle @HEAD
pxui::Response::held field bool @HEAD
pxui::Response::hovered field bool @HEAD
pxui::Response::secondary field bool @HEAD
pxui::RetiredText impl Drop @HEAD
pxui::RetiredText impl Send @HEAD
pxui::RetiredText struct (_) @HEAD
pxui::ReturnKey use pxui_types::ReturnKey @HEAD
pxui::Reveal impl Clone @HEAD
pxui::Reveal impl Copy @HEAD
pxui::Reveal impl Debug @HEAD
pxui::Reveal impl Eq @HEAD
pxui::Reveal impl PartialEq @HEAD
pxui::Reveal impl StructuralPartialEq @HEAD
pxui::Reveal struct @HEAD
pxui::Reveal::new fn const (at: usize, request: u64) -> Reveal @HEAD
pxui::RewriteFn type <'a>= dyn Fn(&str, usize) -> (String, usize) + 'a @HEAD
pxui::Rgba use pxui_types::geom::Rgba @U1a.1
pxui::Role use pxui_types::a11y::Role @U1a.1
-pxui::Routed struct <'a> @U1a.3
-pxui::Routed::buttons fn (&self) -> &[u32] @U1a.3
-pxui::Routed::events fn (&self) -> &[Event] @U1a.3
-pxui::Routed::input fn (&self) -> &Input @U1a.3
-pxui::Routed::keys fn (&self) -> &[u32] @U1a.3
-pxui::Routed::touch fn (&self, index: usize) -> Option<Touch> @U1a.3
pxui::Scroll impl Clone @HEAD
pxui::Scroll impl Copy @HEAD
pxui::Scroll impl Debug @HEAD
pxui::Scroll impl Default @HEAD
pxui::Scroll impl PartialEq @HEAD
pxui::Scroll impl StructuralPartialEq @HEAD
pxui::Scroll struct @HEAD
pxui::Scroll::painted field f32 @HEAD
pxui::Scroll::settled field f32 @HEAD
-pxui::Scrub impl Clone @U1b.1
-pxui::Scrub impl Copy @U1b.1
-pxui::Scrub impl Debug @U1b.1
-pxui::Scrub struct @U1b.1
-pxui::Scrub::commit field Option<f32> @U1b.1
-pxui::Scrub::held field bool @U1b.1
-pxui::Scrub::node field Handle @U1b.1
-pxui::Scrub::preview field Option<f32> @U1b.1
pxui::ScrubFn type <'a>= dyn Fn(&str, f32, bool) -> String + 'a @HEAD
pxui::ScrubPhase use text_inputs::ScrubPhase @HEAD
pxui::Scrubber impl Clone @HEAD
pxui::Scrubber impl Copy @HEAD
pxui::Scrubber impl Debug @HEAD
pxui::Scrubber impl PartialEq @HEAD
pxui::Scrubber impl StructuralPartialEq @HEAD
pxui::Scrubber struct <'a> @HEAD
pxui::Scrubber::DEFAULT const Scrubber<'a> @HEAD
pxui::Scrubber::name fn const (self, name: &'a str) -> Scrubber<'a> @HEAD
pxui::Scrubber::step fn const (self, step: f32) -> Scrubber<'a> @HEAD
pxui::SearchField impl Clone @HEAD
pxui::SearchField impl Copy @HEAD
pxui::SearchField impl Debug @HEAD
pxui::SearchField impl Eq @HEAD
pxui::SearchField impl PartialEq @HEAD
pxui::SearchField impl StructuralPartialEq @HEAD
pxui::SearchField struct <'a> @HEAD
pxui::SearchField::DEFAULT const SearchField<'a> @HEAD
pxui::SearchField::action fn const (self, action: ReturnKey) -> SearchField<'a> @U2b.7
pxui::SearchField::icon fn const (self, icon: &'a str) -> SearchField<'a> @HEAD
pxui::SearchField::name fn const (self, name: &'a str) -> SearchField<'a> @HEAD
pxui::SearchField::placeholder fn const (self, text: &'a str) -> SearchField<'a> @HEAD
pxui::SearchField::variant fn const (self, variant: u16) -> SearchField<'a> @U2b.7
pxui::SearchFieldResponse impl Clone @HEAD
pxui::SearchFieldResponse impl Copy @HEAD
pxui::SearchFieldResponse impl Debug @HEAD
pxui::SearchFieldResponse struct <'a> @HEAD
pxui::SearchFieldResponse::changed field Option<&'a str> @HEAD
pxui::SearchFieldResponse::response field Response @HEAD
pxui::SectionIndex impl Clone @U2a.3
pxui::SectionIndex impl Copy @U2a.3
pxui::SectionIndex impl Debug @U2a.3
pxui::SectionIndex struct <'a> @U2a.3
pxui::SectionIndex::DEFAULT const SectionIndex<'a> @U2a.3
pxui::SectionIndex::name fn const (self, name: &'a str) -> SectionIndex<'a> @U2a.3
pxui::SectionIndex::variant fn const (self, variant: u16) -> SectionIndex<'a> @U2a.3
pxui::Segment use pxui_types::ui::Segment @HEAD
pxui::Segmented use author::Segmented @HEAD
-pxui::SegmentedResponse impl Clone @U1b.1
-pxui::SegmentedResponse impl Copy @U1b.1
-pxui::SegmentedResponse impl Debug @U1b.1
-pxui::SegmentedResponse struct @U1b.1
-pxui::SegmentedResponse::chosen field Option<usize> @U1b.1
-pxui::SegmentedResponse::response field Response @U1b.1
-pxui::SegmentedResponse::secondary field Option<usize> @U1b.1
pxui::SelectMode use pxui_types::ui::SelectMode @U2b.4.33
pxui::SelectionList use pxui_types::ui::SelectionList @HEAD
pxui::Signal impl Clone @HEAD
pxui::Signal impl Debug @HEAD
pxui::Signal impl Default @HEAD
pxui::Signal impl PartialEq @HEAD
pxui::Signal impl StructuralPartialEq @HEAD
pxui::Signal struct @HEAD
pxui::Signal::BUTTON_PRIMARY const u8 @HEAD
pxui::Signal::BUTTON_SECONDARY const u8 @HEAD
pxui::Signal::a11y_select field Option<[u32; 2]> @HEAD
pxui::Signal::a11y_value field Option<f64> @HEAD
pxui::Signal::button field Option<u8> @HEAD
pxui::Signal::clicked field bool @HEAD
pxui::Signal::clicks field i64 @HEAD
pxui::Signal::double_clicked field bool @HEAD
pxui::Signal::drag field (f32, f32) @HEAD
pxui::Signal::dragging field bool @HEAD
pxui::Signal::held field bool @HEAD
pxui::Signal::hovered field bool @HEAD
pxui::Signal::pinch field f64 @HEAD
pxui::Signal::pointer field (f32, f32) @HEAD
pxui::Signal::press_point field (f32, f32) @HEAD
pxui::Signal::pressed field bool @HEAD
pxui::Signal::release_point field (f32, f32) @HEAD
pxui::Signal::released field bool @HEAD
pxui::Signal::scroll field (f32, f32) @HEAD
pxui::Signal::subtree_press field Option<i64> @HEAD
pxui::Size use pxui_types::Size @U1a.1
pxui::Slice use pxui_types::ffi::Slice @HEAD
pxui::SlideFn type <'a>= dyn Fn(f32) -> String + 'a @HEAD
pxui::Slider use pxui_types::ui::Slider @HEAD
pxui::SliderResponse impl Clone @U1b.1
pxui::SliderResponse impl Copy @U1b.1
pxui::SliderResponse impl Debug @U1b.1
pxui::SliderResponse struct @U1b.1
pxui::SliderResponse::commit field Option<f32> @U1b.1
pxui::SliderResponse::preview field Option<f32> @U1b.1
pxui::SliderResponse::response field Response @U1b.1
pxui::SliderResponse::value field f32 @U1b.1
pxui::Sort enum @U2b.1
pxui::Sort impl Clone @U2b.1
pxui::Sort impl Copy @U2b.1
pxui::Sort impl Debug @U2b.1
pxui::Sort impl Eq @U2b.1
pxui::Sort impl PartialEq @U2b.1
pxui::Sort impl StructuralPartialEq @U2b.1
pxui::Sort::Ascending variant @U2b.1
pxui::Sort::Descending variant @U2b.1
pxui::Spinner impl Clone @U1b.5
pxui::Spinner impl Copy @U1b.5
pxui::Spinner impl Debug @U1b.5
pxui::Spinner struct @U1b.5
pxui::Spinner::DEFAULT const Spinner @U1b.5
pxui::Spinner::variant fn const (self, variant: u16) -> Spinner @U1b.5
pxui::Splice impl Clone @HEAD
pxui::Splice impl Copy @HEAD
pxui::Splice impl Debug @HEAD
pxui::Splice impl Eq @HEAD
pxui::Splice impl PartialEq @HEAD
pxui::Splice impl StructuralPartialEq @HEAD
pxui::Splice struct <'a> @HEAD
pxui::Splice::new fn const (start: usize, end: usize, text: &'a str) -> Splice<'a> @HEAD
pxui::Split impl Clone @U1b.1
pxui::Split impl Copy @U1b.1
pxui::Split impl Debug @U1b.1
pxui::Split struct @U1b.1
pxui::Split::DEFAULT const Split @U1b.1
pxui::Split::axis fn const (self, axis: Axis) -> Split @U1b.1
pxui::Split::min_a fn const (self, points: f32) -> Split @U1b.1
pxui::Split::min_b fn const (self, points: f32) -> Split @U1b.1
pxui::SplitResponse impl Clone @U1b.1
pxui::SplitResponse impl Copy @U1b.1
pxui::SplitResponse impl Debug @U1b.1
pxui::SplitResponse struct @U1b.1
pxui::SplitResponse::ratio field Option<f32> @U1b.1
pxui::SplitResponse::response field Response @U1b.1
pxui::Str use pxui_types::Str @HEAD
pxui::Stroke impl Clone @HEAD
pxui::Stroke impl Copy @HEAD
pxui::Stroke impl Debug @HEAD
pxui::Stroke impl PartialEq @HEAD
pxui::Stroke impl StructuralPartialEq @HEAD
pxui::Stroke struct @HEAD
pxui::Stroke::DEFAULT const Stroke @HEAD
pxui::Stroke::MAX_WIDTH const f32 @HEAD
pxui::Stroke::new fn const (color: Rgba) -> Stroke @HEAD
pxui::Stroke::width fn const (self, width: f32) -> Stroke @HEAD
pxui::SwipeActions impl Clone @HEAD
pxui::SwipeActions impl Copy @HEAD
pxui::SwipeActions impl Debug @HEAD
pxui::SwipeActions impl Default @HEAD
pxui::SwipeActions impl PartialEq @HEAD
pxui::SwipeActions impl StructuralPartialEq @HEAD
pxui::SwipeActions struct @HEAD
pxui::SwipeActions::DEFAULT const SwipeActions @HEAD
pxui::SwipeActions::full fn const (self, travel: f32) -> SwipeActions @HEAD
pxui::SwipeActions::leading fn const (self, width: f32) -> SwipeActions @HEAD
pxui::SwipeActions::trailing fn const (self, width: f32) -> SwipeActions @HEAD
pxui::Table impl Clone @U2b.1
pxui::Table impl Copy @U2b.1
pxui::Table impl Debug @U2b.1
pxui::Table impl PartialEq @U2b.1
pxui::Table impl StructuralPartialEq @U2b.1
pxui::Table struct @U2b.1
pxui::Table::DEFAULT const Table @U2b.1
pxui::Table::bare fn const (self) -> Table @U2b.4.37
pxui::Table::head fn const (self, head: TableHead) -> Table @U2b.1
pxui::Table::mode fn const (self, mode: SelectMode) -> Table @U2b.4.37
-pxui::Table::multi fn const (self) -> Table @U2b.4.37
pxui::Table::open_on_click fn const (self, on: bool) -> Table @U2b.4.37
pxui::Table::row_h fn const (self, points: f32) -> Table @U2b.1
pxui::Table::variant fn const (self, variant: u16) -> Table @U2b.1
pxui::TableHead impl Clone @U2b.1
pxui::TableHead impl Copy @U2b.1
pxui::TableHead impl Debug @U2b.1
pxui::TableHead impl PartialEq @U2b.1
pxui::TableHead impl StructuralPartialEq @U2b.1
pxui::TableHead struct @U2b.1
pxui::TableHead::DEFAULT const TableHead @U2b.1
pxui::TableHead::h_px fn const (self, points: f32) -> TableHead @U2b.1
pxui::TableHead::variant fn const (self, variant: u16) -> TableHead @U2b.1
pxui::TableResponse impl Clone @U2b.1
pxui::TableResponse impl Copy @U2b.1
pxui::TableResponse impl Debug @U2b.1
pxui::TableResponse struct @U2b.1
-pxui::TableResponse use pxui_types::ui::TableResponse @U2b.1
pxui::TableResponse::head field HeadResponse @U2b.1
pxui::TableResponse::rows field ListResponse @U2b.1
pxui::TableState impl Clone @U2b.1
pxui::TableState impl Debug @U2b.1
pxui::TableState impl Default @U2b.1
pxui::TableState impl PartialEq @U2b.1
pxui::TableState impl StructuralPartialEq @U2b.1
pxui::TableState struct @U2b.1
pxui::TableState::new fn const () -> TableState @U2b.1
pxui::TableState::selection field ListState @U2b.1
pxui::TableState::sort field Option<(i64, Sort)> @U2b.1
pxui::TableState::widths field Vec<(i64, f32)> @U2b.1
-pxui::TableView use pxui_types::ui::TableView @U2b.1
pxui::Tabs impl Clone @U2a.2
pxui::Tabs impl Copy @U2a.2
pxui::Tabs impl Debug @U2a.2
pxui::Tabs struct <'a> @U2a.2
pxui::Tabs::DEFAULT const Tabs<'a> @U2a.2
pxui::Tabs::focus_on_click fn const (self, focus: bool) -> Tabs<'a> @U2a.2
pxui::Tabs::name fn const (self, name: &'a str) -> Tabs<'a> @U2a.2
pxui::Tabs::variant fn const (self, variant: u16) -> Tabs<'a> @U2a.2
pxui::TargetDesc impl Clone @HEAD
pxui::TargetDesc impl Copy @HEAD
pxui::TargetDesc impl Default @HEAD
pxui::TargetDesc struct @HEAD
pxui::TargetDesc::format field u32 @HEAD
pxui::TargetDesc::samples field u8 @HEAD
pxui::Text impl Drop @HEAD
pxui::Text impl Send @HEAD
pxui::Text struct (_) @HEAD
pxui::Text::add_font fn (&mut self, bytes: &[u8]) -> Result<u32, Error> @HEAD
pxui::Text::enable_system_fonts fn (&mut self) -> Result<(), Error> @HEAD
pxui::Text::measure fn (&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, Error> @HEAD
pxui::Text::measure_spans fn (&mut self, value: &str, style: &TextStyle, max_width: f32, spans: &[TextSpan]) -> Result<TextMetrics, Error> @HEAD
pxui::Text::new fn (fonts: &[&[u8]]) -> Result<Self, Error> @HEAD
pxui::Text::prepare fn (&mut self, value: &str, style: &TextStyle, max_width: f32, spans: &[TextSpan]) -> Result<PreparedText, Error> @HEAD
pxui::Text::rasterize_rgba fn (&mut self, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4], out: &mut Vec<u8>) -> Result<(u32, u32), Error> @HEAD
pxui::Text::width fn (&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, Error> @HEAD
pxui::TextAlign use pxui_types::text::TextAlign @U1a.1
pxui::TextArea impl Clone @HEAD
pxui::TextArea impl Copy @HEAD
pxui::TextArea impl Debug @HEAD
pxui::TextArea impl PartialEq @HEAD
pxui::TextArea impl StructuralPartialEq @HEAD
pxui::TextArea struct <'a> @HEAD
pxui::TextArea::DEFAULT const TextArea<'a> @HEAD
pxui::TextArea::chips fn const (self, chips: &'a [(usize, usize, Rgba)]) -> TextArea<'a> @HEAD
pxui::TextArea::errors fn const (self, lines: &'a [usize]) -> TextArea<'a> @HEAD
pxui::TextArea::name fn const (self, name: &'a str) -> TextArea<'a> @HEAD
pxui::TextArea::notes fn const (self, notes: &'a [AreaNote<'a>]) -> TextArea<'a> @HEAD
pxui::TextArea::reveal fn const (self, reveal: Reveal) -> TextArea<'a> @HEAD
pxui::TextArea::select fn const (self, start: usize, end: usize) -> TextArea<'a> @HEAD
pxui::TextArea::spans fn const (self, spans: &'a [(usize, usize)]) -> TextArea<'a> @HEAD
pxui::TextArea::splices fn const (self, splices: &'a [Splice<'a>]) -> TextArea<'a> @HEAD
pxui::TextArea::wrap fn const (self, on: bool) -> TextArea<'a> @HEAD
pxui::TextAreaResponse impl Clone @HEAD
pxui::TextAreaResponse impl Copy @HEAD
pxui::TextAreaResponse impl Debug @HEAD
pxui::TextAreaResponse struct <'a> @HEAD
pxui::TextAreaResponse::changed field Option<&'a str> @HEAD
pxui::TextAreaResponse::response field Response @HEAD
pxui::TextAreaResponse::submitted field bool @HEAD
pxui::TextField impl Clone @HEAD
pxui::TextField impl Copy @HEAD
pxui::TextField impl Debug @HEAD
pxui::TextField impl Eq @HEAD
pxui::TextField impl PartialEq @HEAD
pxui::TextField impl StructuralPartialEq @HEAD
pxui::TextField struct <'a> @HEAD
pxui::TextField::DEFAULT const TextField<'a> @HEAD
pxui::TextField::SEARCH const TextField<'a> @HEAD
pxui::TextField::accessible_name fn const (&self) -> &'a str @HEAD
pxui::TextField::action fn const (self, action: ReturnKey) -> TextField<'a> @HEAD
pxui::TextField::autocorrect fn const (self, on: bool) -> TextField<'a> @HEAD
pxui::TextField::body_variant fn const (&self) -> u16 @HEAD
pxui::TextField::capitalization fn const (self, capitalization: Capitalize) -> TextField<'a> @HEAD
pxui::TextField::name fn const (self, name: &'a str) -> TextField<'a> @HEAD
pxui::TextField::traits fn const (&self) -> TextTraits @HEAD
pxui::TextField::variant fn const (self, variant: u16) -> TextField<'a> @HEAD
pxui::TextFieldResponse impl Clone @HEAD
pxui::TextFieldResponse impl Copy @HEAD
pxui::TextFieldResponse impl Debug @HEAD
pxui::TextFieldResponse struct <'a> @HEAD
pxui::TextFieldResponse::changed field Option<&'a str> @HEAD
pxui::TextFieldResponse::response field Response @HEAD
pxui::TextSpan use pxui_types::TextSpan @HEAD
pxui::TextStyle impl Clone @HEAD
pxui::TextStyle impl Copy @HEAD
pxui::TextStyle impl Debug @HEAD
pxui::TextStyle impl PartialEq @HEAD
pxui::TextStyle impl StructuralPartialEq @HEAD
pxui::TextStyle struct (_, _) @HEAD
pxui::TextStyle::line_height fn (&self) -> f32 @HEAD
pxui::TextStyle::size fn (&self) -> f32 @HEAD
pxui::TextStyle::tracking fn (&self) -> f32 @HEAD
pxui::TextTraits use pxui_types::TextTraits @HEAD
pxui::TextWorker impl Drop @HEAD
pxui::TextWorker impl Send @HEAD
pxui::TextWorker impl Sync @HEAD
pxui::TextWorker struct (_) @HEAD
pxui::TextWorker::new fn () -> Result<TextWorker, Error> @HEAD
-pxui::Theme impl Default @U1a.2
-pxui::Theme impl Drop @U1a.2
-pxui::Theme struct (_) @U1a.2
-pxui::Theme::apply fn (&mut self, ui: &Ui, text: &str) -> Result<(), String> @U1a.2
-pxui::Theme::from_tokens fn (tokens: Tokens) -> Theme @U1a.2
-pxui::Theme::load fn (&mut self, path: &str) -> Result<(), String> @U1a.2
-pxui::Theme::set fn (&mut self, key: &str, value: &str) -> Result<(), String> @U1a.2
pxui::ThemeHandle impl Default @U1a.2
pxui::ThemeHandle impl Drop @U1a.2
pxui::ThemeHandle struct (_) @U1a.2
pxui::ThemeHandle::animate_swap fn (&mut self, on: bool) @HEAD
pxui::ThemeHandle::density fn (&mut self, density: f32) @U1a.2
pxui::ThemeHandle::from_tokens fn (tokens: Tokens) -> ThemeHandle @U1a.2
pxui::ThemeHandle::parse fn (source: &str) -> Result<ThemeHandle, Error> @U1a.2
pxui::ThemeHandle::reduced_motion fn (&mut self, on: bool) @U1a.2
pxui::ThemeHandle::set_color fn (&mut self, token: ColorToken, color: Rgba) @U1a.2
-pxui::ThemeSource use pxui_types::ffi::ThemeSource @U2c.5
pxui::Toast use pxui_types::ui::Toast @HEAD
pxui::Toggle impl Clone @HEAD
pxui::Toggle impl Copy @HEAD
pxui::Toggle struct <'a> @HEAD
pxui::Toggle::DEFAULT const Toggle<'a> @HEAD
pxui::Toggle::detail fn const (self, detail: &'a str) -> Toggle<'a> @HEAD
pxui::Toggle::disabled fn const (self, disabled: bool) -> Toggle<'a> @U1b.6
-pxui::Toggle::enabled fn const (self, enabled: bool) -> Toggle<'a> @U1b.6
pxui::Toggle::inset fn const (self, inset: f32) -> Toggle<'a> @HEAD
pxui::Toggle::knob fn const (self, look: &'a States) -> Toggle<'a> @HEAD
pxui::Toggle::label fn const (self, label: Label) -> Toggle<'a> @HEAD
pxui::Toggle::press_row fn const (self, press_row: bool) -> Toggle<'a> @HEAD
pxui::Toggle::row_h fn const (self, h: f32) -> Toggle<'a> @HEAD
pxui::Toggle::switch fn const (self, w: f32, h: f32) -> Toggle<'a> @HEAD
pxui::Toggle::track fn const (self, look: &'a States) -> Toggle<'a> @HEAD
pxui::Toggle::variant fn const (self, variant: u16) -> Toggle<'a> @HEAD
pxui::Touch use pxui_types::Touch @HEAD
pxui::TreeNode impl Clone @HEAD
pxui::TreeNode impl Copy @HEAD
pxui::TreeNode impl Debug @HEAD
pxui::TreeNode impl Default @HEAD
pxui::TreeNode impl Eq @HEAD
pxui::TreeNode impl PartialEq @HEAD
pxui::TreeNode impl StructuralPartialEq @HEAD
pxui::TreeNode struct @HEAD
pxui::TreeNode::children field u8 @HEAD
pxui::TreeNode::depth field u16 @HEAD
pxui::TreeNode::new fn const (depth: u16, has_children: bool) -> TreeNode @U1b.5
pxui::TreeResponse use pxui_types::ui::TreeResponse @HEAD
pxui::TreeState impl Clone @U2b.4.28
pxui::TreeState impl Debug @U2b.4.28
pxui::TreeState impl Default @U2b.4.28
pxui::TreeState impl Eq @U2b.4.28
pxui::TreeState impl PartialEq @U2b.4.28
pxui::TreeState impl StructuralPartialEq @U2b.4.28
pxui::TreeState struct @U2b.4.28
pxui::TreeState::new fn const () -> TreeState @U2b.4.28
pxui::TreeState::open field Vec<u8> @U2b.4.28
pxui::TreeState::revision field Option<u64> @U2b.4.28
pxui::TreeState::selected field Option<usize> @U2b.4.28
pxui::TreeState::visible field Vec<u32> @U2b.4.28
pxui::Ui impl Drop @HEAD
pxui::Ui struct (_) @HEAD
pxui::Ui::a11y_name fn (&mut self, box_: &Handle, name: &str) -> bool @U1a.3
pxui::Ui::a11y_name_of fn (&self, node: &A11yNode) -> &str @HEAD
pxui::Ui::a11y_nodes fn (&self) -> &[A11yNode] @HEAD
pxui::Ui::a11y_number fn (&mut self, box_: &Handle, now: f64, min: f64, max: f64, step: f64) -> Result<bool, Error> @U1a.3
-pxui::Ui::a11y_tree fn (&self) -> (&[u8], u64, u32) @U1b.4
pxui::Ui::a11y_tree fn (&self) -> A11yTree<'_> @U1b.4
pxui::Ui::a11y_value fn (&mut self, box_: &Handle, value: &str, selection: (u32, u32)) -> Result<bool, Error> @U1a.3
pxui::Ui::a11y_value_of fn (&self, node: &A11yNode) -> Option<&str> @HEAD
pxui::Ui::add_surface fn (&mut self, surface: u64) -> Result<(), Error> @U5.1
-pxui::Ui::anchored fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, anchor: Rect, overlay: &Overlay<'_>, f: F) -> Handle @U1b.1
pxui::Ui::anchored fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, anchor: Rect, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
pxui::Ui::animate fn (&mut self, b: &Handle, target: Target, curve: Curve) -> bool @HEAD
pxui::Ui::animate_keys fn (&mut self, box_: &Handle, keys: &[Keyframe], play: &Keyframes) -> bool @HEAD
pxui::Ui::arranged_nodes fn (&self) -> usize @HEAD
pxui::Ui::background_request fn (&self) -> Option<Rgba> @HEAD
-pxui::Ui::badge fn <K: Into<Key>>(&mut self, key: K, name: &str, text: &str) -> Handle @U1b.5
pxui::Ui::badge fn <K: Into<Key>>(&mut self, key: K, name: &str, text: &str, badge: &Badge) -> Handle @U1b.5
pxui::Ui::box_of_a11y fn (&self, id: u64) -> Option<Handle> @HEAD
-pxui::Ui::box_of_key fn (&self, key: i64) -> Option<Handle> @U1b.2
pxui::Ui::box_of_key fn <K: Into<Key>>(&self, key: K) -> Option<Handle> @U1b.2
pxui::Ui::built_nodes fn (&self) -> usize @HEAD
pxui::Ui::busy_step fn (&mut self, box_: &Handle, now: f64) -> bool @HEAD
-pxui::Ui::button fn (&mut self, text: &str) -> bool @U1b.6
pxui::Ui::button fn <K: Into<Key>>(&mut self, key: K, text: &str, button: &Button<'_>) -> Response @U1b.6
pxui::Ui::button_content fn <K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, button: &Button<'_>, f: F) -> Response @HEAD
-pxui::Ui::button_with fn <K: Into<Key>>(&mut self, key: K, text: &str, button: &Button<'_>) -> Response @U1b.6
pxui::Ui::bytes fn (&self) -> Bytes @HEAD
pxui::Ui::cached_label fn (&mut self, b: &Handle, index: usize, text: &str) -> &str @HEAD
pxui::Ui::cancel_carry fn (&mut self) @HEAD
pxui::Ui::canvas fn <F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) @HEAD
pxui::Ui::canvas_over fn <F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) @HEAD
-pxui::Ui::cap_width fn (&mut self, text: &str) -> f32 @U1b.3
pxui::Ui::captured fn (&self) -> bool @HEAD
pxui::Ui::carry fn (&mut self, from: Option<&Handle>, kind: &str, value: &str) @HEAD
pxui::Ui::carrying fn (&self) -> Option<PayloadRef<'_>> @HEAD
pxui::Ui::checkbox fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, checkbox: &Checkbox<'_>) -> Response @U2a.3
pxui::Ui::clipboard_request fn (&self) -> Option<&str> @HEAD
pxui::Ui::close_swipe fn (&mut self, node: &Handle) @HEAD
pxui::Ui::color fn (&self, token: ColorToken) -> Rgba @HEAD
pxui::Ui::column fn <F: FnMut(&mut Ui)>(&mut self, node: Node, f: F) -> Handle @U1b.6
-pxui::Ui::column fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.6
pxui::Ui::content_rect fn (&self) -> Rect @HEAD
-pxui::Ui::context_at fn (&mut self, b: &Handle) -> Option<(f32, f32)> @U1b.2
pxui::Ui::context_at fn (&mut self, b: &Handle) -> Option<Point> @U1b.2
-pxui::Ui::create fn (platform: Platform, font_size: Option<i64>, fonts: &[&[u8]]) -> Self @U1a.3
pxui::Ui::density fn (&self) -> f32 @U1b.3
-pxui::Ui::density fn (&self) -> i64 @U1b.3
pxui::Ui::dialog fn <K: Into<Key>>(&mut self, key: K, title: &str, buttons: &[DialogButton<'_>], dialog: &Dialog, body: &mut dyn FnMut(&mut Ui)) -> Chosen @HEAD
pxui::Ui::disabled fn <F: FnMut(&mut Ui)>(&mut self, disabled: bool, f: F) @HEAD
pxui::Ui::disclosure fn <K: Into<Key>>(&mut self, key: K, text: &str, open: bool, disclosure: &Disclosure, f: &mut dyn FnMut(&mut Ui)) -> Response @U2a.3
pxui::Ui::dismiss_popup fn (&mut self) @HEAD
pxui::Ui::draw_revision fn (&self) -> u64 @HEAD
pxui::Ui::drawn fn (&mut self, index: usize) @HEAD
pxui::Ui::drop_target fn (&mut self, b: &Handle) -> Option<DropRef<'_>> @HEAD
pxui::Ui::dropdown fn <K: Into<Key>>(&mut self, key: K, text: &str, items: &[Segment<'_>], selected: Option<usize>, dropdown: &Dropdown) -> PickResponse @U1b.1
-pxui::Ui::dropdown fn <K: Into<Key>>(&mut self, key: K, text: &str, options: &[&str], selected: usize, dropdown: &Dropdown) -> (Response, usize) @U1b.1
-pxui::Ui::enable_system_fonts fn (&mut self) -> Result<(), Error> @U1b.3
pxui::Ui::end_refresh fn (&mut self, node: &Handle) @HEAD
-pxui::Ui::enter fn (&mut self, box_: &Handle) -> UiSaved @U1a.3
pxui::Ui::environment fn (&self) -> Environment @HEAD
-pxui::Ui::expanded fn (&self, key: &str) -> Option<bool> @U1b.2
pxui::Ui::expanded fn <K: Into<Key>>(&self, key: K) -> Option<bool> @U1b.2
-pxui::Ui::field_drag fn (&self, field: &Handle) -> Option<(f32, f32)> @U1c.3
pxui::Ui::focus_box fn (&mut self, box_: &Handle) @U1a.3
pxui::Ui::focus_default fn (&mut self, box_: &Handle) @U2b.4.24
-pxui::Ui::focused fn (&self, box_: &Handle) -> bool @U1a.3
pxui::Ui::font_size fn (&self) -> f32 @U1b.3
-pxui::Ui::font_size fn (&self) -> i64 @U1b.3
-pxui::Ui::footer fn (&mut self, right: Option<&str>, hints: &[(&str, &str)]) @U1c.3
pxui::Ui::frame fn (&mut self, input: &FrameIn<'_>, build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> @U1a.3
pxui::Ui::frame_index fn (&self) -> u64 @HEAD
pxui::Ui::frame_reasons fn (&self) -> PxuiFrameReasons @HEAD
pxui::Ui::gesture fn (&mut self, node: &Handle, opts: &GestureOpts) -> GestureState @U1b.2
-pxui::Ui::gesture fn (&mut self, node: &Handle, opts: GestureOpts) -> GestureState @U1b.2
pxui::Ui::gesture_state fn (&mut self, node: &Handle) -> GestureState @HEAD
-pxui::Ui::grid_list fn <K: Into<Key>>(&mut self, key: K, count: usize, cell_w: f32, cell_h: f32, build: &mut dyn FnMut(&mut Ui, usize)) -> Handle @U1b.5
pxui::Ui::grid_list fn <K: Into<Key>>(&mut self, key: K, count: usize, grid: &GridList, build: &mut dyn FnMut(&mut Ui, usize)) -> Handle @U1b.5
-pxui::Ui::grip fn (&mut self, label: Arguments<'_>, axis: Axis, rect: Option<Rect>) -> (Handle, Signal) @U1c.3
-pxui::Ui::hover_delay fn (&mut self, key: &str) -> bool @U1b.2
pxui::Ui::hover_delay fn <K: Into<Key>>(&mut self, key: K) -> bool @U1b.2
pxui::Ui::hovered fn (&self, b: &Handle, within: bool) -> bool @U1b.2
-pxui::Ui::hovered_exactly fn (&self, b: &Handle) -> bool @U1b.2
-pxui::Ui::hovered_within fn (&mut self, b: &Handle) -> bool @U1b.2
pxui::Ui::icon_button fn <K: Into<Key>>(&mut self, key: K, icon: Icon, name: &str, button: &IconButton<'_>) -> Response @HEAD
-pxui::Ui::id fn (&self) -> usize @U1b.6
pxui::Ui::id fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, f: F) @U1b.6
-pxui::Ui::image fn (&mut self, name: &str, image: &Image, style: &ImageStyle) -> Result<Handle, Error> @U1b.2
pxui::Ui::image fn <K: Into<Key>>(&mut self, key: K, name: &str, image: &Image, style: &ImageStyle) -> Result<Handle, Error> @U1b.2
-pxui::Ui::ime fn (&self) -> Option<((i64, i64, i64, i64), i64)> @U1b.2
pxui::Ui::ime fn (&self) -> Option<Ime> @U1b.2
pxui::Ui::inject_a11y fn (&mut self, actions: &[A11yAction]) @HEAD
-pxui::Ui::input_frame fn (&mut self, input: &Input, build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> @U1a.3
-pxui::Ui::inspector_bar fn (&mut self, hints: &[(&str, &str)], count: i64) @U1c.3
-pxui::Ui::inspector_body fn (&mut self, f: &mut dyn FnMut(&mut Ui)) @U1c.3
-pxui::Ui::inspector_button fn (&mut self, key: &str, label: &str) -> bool @U1c.3
-pxui::Ui::inspector_header fn (&mut self, key: &str, kind: Option<&str>, badge: Option<&str>, index: Option<&str>, rename: Option<Rc<dyn Fn(&str) -> bool>>, actions: &[HeadAction], reset: Option<&str>, reset_enabled: bool, title: &str, detail: &str) -> Head @U1c.3
-pxui::Ui::inspector_label_x fn (&self) -> f32 @U1c.3
-pxui::Ui::inspector_message fn (&mut self, key: &str, message: &str) @U1c.3
-pxui::Ui::inspector_readout fn (&mut self, width: Option<f32>, key: &str, label: &str, value: &str) @U1c.3
-pxui::Ui::inspector_row fn (&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (Handle, f32, f32, f32) @U1c.3
-pxui::Ui::inspector_section fn (&mut self, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, build: &mut dyn FnMut(&mut Ui)) -> bool @U1c.3
-pxui::Ui::inspector_toggle fn (&mut self, key: &str, label: &str, value: bool) -> bool @U1c.3
-pxui::Ui::inspector_width fn (&self) -> f32 @U1c.3
pxui::Ui::instance_id fn (&self) -> usize @U1b.6
pxui::Ui::item_text fn (&mut self, text: &str) @HEAD
pxui::Ui::key_down fn (&self, key: KeyCode) -> bool @U1a.4
pxui::Ui::key_pressed fn (&self, key: KeyCode, mods: Mods) -> bool @U1a.4
pxui::Ui::keyboard fn (&self) -> Keyboard @HEAD
pxui::Ui::label fn (&mut self, text: &str, style: Label) -> Handle @HEAD
pxui::Ui::label_at fn (&mut self, key: Key, rect: Rect, text: &str, style: Label) -> Result<Handle, Error> @HEAD
pxui::Ui::label_digits fn (&mut self, args: Arguments<'_>, style: Label) -> Handle @HEAD
pxui::Ui::label_fmt fn (&mut self, args: Arguments<'_>, style: Label) -> Handle @HEAD
pxui::Ui::last_error fn (&self) -> &str @HEAD
pxui::Ui::last_press_within fn (&self, events: &[Event], roots: &[Key]) -> Option<Key> @U1b.2
-pxui::Ui::last_press_within fn (&self, events: &[Event], roots: &[i64]) -> Option<i64> @U1b.2
pxui::Ui::layer fn (&self, index: usize) -> Option<Layer<'_>> @U1a.3
pxui::Ui::layer_count fn (&self) -> usize @HEAD
pxui::Ui::layers fn (&self) -> Layers<'_> @U1a.3
-pxui::Ui::leave fn (&mut self, saved: UiSaved) @U1a.3
pxui::Ui::library_calls fn (&self) -> u64 @HEAD
-pxui::Ui::line_field fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'_> @U1c.3
pxui::Ui::list fn <K: Into<Key>, F: FnMut(&mut Ui, usize)>(&mut self, key: K, count: usize, row_h: f32, row_key: &dyn Fn(usize) -> i64, f: F) -> ListResponse @U2b.2
-pxui::Ui::load_text_edit fn (&mut self, key: i64, text: &str) -> TextEdit @U1b.2
pxui::Ui::load_text_edit fn <K: Into<Key>>(&mut self, key: K, text: &str) -> TextEdit @U1b.2
pxui::Ui::local_rect fn (&self, b: &Handle) -> Rect @HEAD
-pxui::Ui::menu fn <K: Into<Key>>(&mut self, key: K, at: (f32, f32), items: &[MenuItem<'_>], menu: &Menu<'_>) -> Chosen @U1b.3
pxui::Ui::menu fn <K: Into<Key>>(&mut self, key: K, at: Point, items: &[MenuItem<'_>], menu: &Menu<'_>) -> Chosen @U1b.3
-pxui::Ui::menu_bar fn (&mut self, label: &str, menus: &[(&str, &[&str])]) -> Option<(usize, usize)> @U1b.1
pxui::Ui::menu_bar fn <K: Into<Key>>(&mut self, key: K, menus: &[MenuBarMenu<'_>], bar: &MenuBar) -> Chosen @U1b.1
pxui::Ui::modal fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
-pxui::Ui::modal fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> bool @U1b.1
pxui::Ui::new fn (desc: &UiDesc<'_>) -> Result<Self, Error> @U1a.3
-pxui::Ui::node fn (&mut self, key: Key, node: Node) -> Handle @U1b.2
pxui::Ui::node fn <K: Into<Key>>(&mut self, key: K, node: Node) -> Handle @U1b.2
pxui::Ui::node_layout fn (&self, b: &Handle) -> Node @HEAD
pxui::Ui::node_mut fn (&mut self, b: &Handle, edit: &mut dyn FnMut(&mut NodeFields)) @U1c.1
pxui::Ui::now fn (&self) -> f64 @HEAD
pxui::Ui::output fn (&self) -> Output @U1a.3
-pxui::Ui::panel fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) @U1b.1
pxui::Ui::panel fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
pxui::Ui::part fn (&mut self, part: Part, node: Node) -> Handle @HEAD
pxui::Ui::part_box fn (&self, owner: &Handle, part: Part) -> Option<Handle> @U1b.2
-pxui::Ui::part_box fn (&self, owner: i64, part: Part) -> Option<Handle> @U1b.2
pxui::Ui::part_rect fn (&self, owner: &Handle, part: Part) -> Option<Rect> @U1b.2
-pxui::Ui::part_rect fn (&self, owner: i64, part: Part) -> Option<Rect> @U1b.2
pxui::Ui::passed_undo fn (&self) -> Option<UndoKind> @HEAD
pxui::Ui::paste_requested fn (&self) -> bool @HEAD
pxui::Ui::patch_field fn (&mut self, field: &Handle, o: &ValueField<'_>, value_text: &str) -> bool @HEAD
pxui::Ui::patch_look fn (&mut self, b: &Handle, value: &Look) -> bool @HEAD
pxui::Ui::patch_mark fn (&mut self, b: &Handle, value: Mark) -> bool @HEAD
pxui::Ui::patch_rect fn (&mut self, b: &Handle, value: Rect) -> bool @HEAD
pxui::Ui::patch_text fn (&mut self, box_: &Handle, value: &str) -> bool @HEAD
pxui::Ui::phase_cpu_times fn (&self) -> Result<[Duration; 7], Error> @HEAD
pxui::Ui::phase_times fn (&self) -> [Duration; 7] @HEAD
pxui::Ui::picker fn <K: Into<Key>>(&mut self, key: K, placeholder: &str, query: &str, picker: &Picker<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> @U1b.1
-pxui::Ui::picker fn <K: Into<Key>>(&mut self, key: K, placeholder: &str, query: &str, picker: &PickerOpts<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> @U1b.1
pxui::Ui::platform fn (&self) -> Platform @HEAD
pxui::Ui::popup fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
-pxui::Ui::popup fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> bool @U1b.1
pxui::Ui::press_keys fn (&self, b: &Handle) -> &[u32] @HEAD
pxui::Ui::pressable fn <K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, node: Node, pressable: &Pressable<'_>, f: F) -> Response @HEAD
pxui::Ui::prewarm_text fn (&mut self, text: &str, style: &Label, max_width: f32) -> Result<(), Error> @HEAD
-pxui::Ui::progress fn <K: Into<Key>>(&mut self, key: K, text: &str, value: Option<f32>) -> Handle @U1b.5
pxui::Ui::progress fn <K: Into<Key>>(&mut self, key: K, text: &str, value: Option<f32>, progress: &Progress) -> Handle @U1b.5
pxui::Ui::radio fn <K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, radio: &Radio<'_>) -> PickResponse @U1b.1
-pxui::Ui::radio fn <K: Into<Key>>(&mut self, key: K, options: &[&str], selected: usize, choice: &Choice<'_>) -> (Response, usize) @U1b.1
pxui::Ui::rebuilt_named fn (&self, prefix: &str) -> usize @HEAD
pxui::Ui::rebuilt_views fn (&self) -> usize @HEAD
pxui::Ui::rect fn (&self, box_: &Handle) -> Rect @HEAD
pxui::Ui::register_badge_body fn (&mut self, name: &str, body: fn(&mut Ui, &BadgeConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_button_body fn (&mut self, name: &str, body: fn(&mut Ui, &ButtonConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_checkbox_body fn (&mut self, name: &str, body: fn(&mut Ui, &CheckboxConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_dialog_body fn (&mut self, name: &str, body: fn(&mut Ui, &DialogConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_dropdown_body fn (&mut self, name: &str, body: fn(&mut Ui, &DropdownConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_footer_body fn (&mut self, name: &str, body: fn(&mut Ui, &FooterConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_grip_body fn (&mut self, name: &str, body: fn(&mut Ui, &GripConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_icon_button_body fn (&mut self, name: &str, body: fn(&mut Ui, &IconButtonConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_inspector_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &InspectorRowConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_inspector_section_body fn (&mut self, name: &str, body: fn(&mut Ui, &InspectorSectionConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_label_body fn (&mut self, name: &str, body: fn(&mut Ui, &LabelConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_line_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &LineFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_menu_bar_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuBarConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_menu_item_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuItemConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_menu_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuRowConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_modal_body fn (&mut self, name: &str, body: fn(&mut Ui, &ModalConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_panel_body fn (&mut self, name: &str, body: fn(&mut Ui, &PanelConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_picker_body fn (&mut self, name: &str, body: fn(&mut Ui, &PickerConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_picker_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &PickerFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_popup_body fn (&mut self, name: &str, body: fn(&mut Ui, &PopupConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_progress_body fn (&mut self, name: &str, body: fn(&mut Ui, &ProgressConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_radio_body fn (&mut self, name: &str, body: fn(&mut Ui, &RadioConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_scroll_view_body fn (&mut self, name: &str, body: fn(&mut Ui, &ScrollViewConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_search_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &SearchFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_section_index_body fn (&mut self, name: &str, body: fn(&mut Ui, &SectionIndexConfig)) -> Result<BodyId, Error> @U2a.3
pxui::Ui::register_segmented_body fn (&mut self, name: &str, body: fn(&mut Ui, &SegmentedConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_selection_list_body fn (&mut self, name: &str, body: fn(&mut Ui, &SelectionListConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_slider_body fn (&mut self, name: &str, body: fn(&mut Ui, &SliderConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_spinner_body fn (&mut self, name: &str, body: fn(&mut Ui, &SpinnerConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_split_body fn (&mut self, name: &str, body: fn(&mut Ui, &SplitConfig)) -> Result<BodyId, Error> @U1a.3
-pxui::Ui::register_table_cell_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableCellConfig)) -> Result<BodyId, Error> @U2b.1
pxui::Ui::register_table_head_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableHeadConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_table_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableRowConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_tabs_body fn (&mut self, name: &str, body: fn(&mut Ui, &TabsConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_text_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &TextFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_toasts_body fn (&mut self, name: &str, body: fn(&mut Ui, &ToastsConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_toggle_body fn (&mut self, name: &str, body: fn(&mut Ui, &ToggleConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_tooltip_body fn (&mut self, name: &str, body: fn(&mut Ui, &TooltipConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_tree_body fn (&mut self, name: &str, body: fn(&mut Ui, &TreeConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_value_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &ValueFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::Ui::register_window_body fn (&mut self, name: &str, body: fn(&mut Ui, &WindowConfig)) -> Result<BodyId, Error> @U1a.3
-pxui::Ui::remember fn (&mut self, edit: &TextEdit, typing: bool) @U1b.2
pxui::Ui::remember fn <K: Into<Key>>(&mut self, key: K, edit: &TextEdit, typing: bool) @U1b.2
pxui::Ui::remove_surface fn (&mut self, surface: u64) -> Result<(), Error> @U5.1
pxui::Ui::reordering fn (&self) -> bool @U2b.4.12
pxui::Ui::repaint_after fn (&mut self, seconds: f64) @HEAD
pxui::Ui::repaint_at fn (&self) -> Option<f64> @HEAD
pxui::Ui::request_cursor fn (&mut self, shape: Cursor) @HEAD
pxui::Ui::request_frame fn (&mut self) @HEAD
pxui::Ui::step fn (&mut self, time: f64) -> bool @HEAD
pxui::Ui::resolve fn (&self, style: &Label) -> TextStyle @HEAD
pxui::Ui::retain_palette_labels fn (&mut self) @HEAD
pxui::Ui::retain_palette_paint fn (&mut self) @HEAD
pxui::Ui::retain_view_on_resize fn (&mut self) @HEAD
pxui::Ui::reveal fn (&mut self, list: &Handle, index: usize) @HEAD
-pxui::Ui::root_at fn (&self, point: (f32, f32), roots: &[i64]) -> Option<i64> @U1b.2
pxui::Ui::root_at fn (&self, point: Point, roots: &[Key]) -> Option<Key> @U1b.2
-pxui::Ui::routed_input fn (&mut self, owner: Option<i64>) -> Routed<'_> @U1a.3
pxui::Ui::row fn <F: FnMut(&mut Ui)>(&mut self, node: Node, f: F) -> Handle @U1b.6
-pxui::Ui::row fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.6
pxui::Ui::row_height fn (&self) -> f32 @U1b.3
-pxui::Ui::row_height fn (&self) -> i64 @U1b.3
pxui::Ui::scroll fn (&self, b: &Handle) -> Scroll @HEAD
pxui::Ui::scroll_view fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.5
-pxui::Ui::scroll_view fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, w: Size, h: Size, f: F) -> Handle @U1b.5
-pxui::Ui::scrub fn <K: Into<Key>>(&mut self, key: K, value: f32, range: RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> Scrub @U1b.1
pxui::Ui::scrub fn <K: Into<Key>>(&mut self, key: K, value: f32, range: RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> SliderResponse @U1b.1
pxui::Ui::search_field fn <K: Into<Key>>(&mut self, key: K, value: &str, field: &SearchField<'_>) -> SearchFieldResponse<'_> @HEAD
pxui::Ui::section_index fn <K: Into<Key>>(&mut self, key: K, letters: &[&str], index: &SectionIndex<'_>) -> PickResponse @U2a.3
-pxui::Ui::section_list fn <K: Into<Key>>(&mut self, key: K, section: &SectionList, row_key: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, f32)) -> ListResponse @U2b.4.10
pxui::Ui::section_list fn <K: Into<Key>>(&mut self, key: K, section: &SectionList, row_key: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse @U2b.4.10
pxui::Ui::segmented fn <K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, segmented: &Segmented<'_>) -> PickResponse @U1b.1
-pxui::Ui::segmented_with fn <K: Into<Key>>(&mut self, key: K, options: &[Segment<'_>], selected: Option<usize>, segmented: &Segmented<'_>) -> SegmentedResponse @U1b.1
-pxui::Ui::selection_list fn <K: Into<Key>>(&mut self, key: K, count: usize, row_key: &dyn Fn(usize) -> i64, selected: &mut [u8], list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, usize)) -> ListResponse @U2b.1
pxui::Ui::selection_list fn <K: Into<Key>>(&mut self, key: K, count: usize, row_key: &dyn Fn(usize) -> i64, state: &mut ListState, list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse @U2b.1
pxui::Ui::set_a11y fn (&mut self, on: bool) @HEAD
pxui::Ui::set_at fn (&mut self, b: &Handle, at: (f32, f32)) @HEAD
-pxui::Ui::set_colors fn (&mut self, colors: &[(Rgba, Rgba)]) @U1a.2
pxui::Ui::set_dir fn (&mut self, rtl: bool) @HEAD
pxui::Ui::set_font_size fn (&mut self, size: f32) @U1b.3
-pxui::Ui::set_font_size fn (&mut self, size: i64) @U1b.3
-pxui::Ui::set_look fn (&mut self, box_: &Handle, look: Look, state: State) @U1b.4
pxui::Ui::set_node_layout fn (&mut self, b: &Handle, node: &Node) @HEAD
pxui::Ui::set_role fn (&mut self, box_: &Handle, role: Role, states: A11yStates) @U1b.4
-pxui::Ui::set_role fn (&mut self, box_: &Handle, role: Role, states: u32) @U1b.4
pxui::Ui::set_scroll_offset fn (&mut self, b: &Handle, value: f32) @HEAD
pxui::Ui::set_state fn (&mut self, box_: &Handle, value: i64) @HEAD
pxui::Ui::set_text_state fn (&mut self, b: &Handle, value: Option<&str>) @HEAD
pxui::Ui::set_text_worker fn (&mut self, worker: Option<&TextWorker>) -> Result<(), Error> @HEAD
pxui::Ui::set_theme fn (&mut self, theme: &ThemeHandle) -> Result<(), Error> @U1a.2
-pxui::Ui::set_theme_source fn (&mut self, source: &ThemeSource) -> Result<(), Error> @U1a.2
pxui::Ui::set_timing fn (&mut self, on: bool) @HEAD
pxui::Ui::set_view fn (&mut self, b: &Handle, view: &ViewTransform) @HEAD
pxui::Ui::signal fn (&self, box_: &Handle) -> Signal @HEAD
pxui::Ui::signal_keys fn (&self, box_: &Handle) -> &[Event] @HEAD
-pxui::Ui::slider fn <K: Into<Key>>(&mut self, key: K, text: &str, value: f32, range: RangeInclusive<f32>, slider: &Slider) -> (Response, f32) @U1b.1
pxui::Ui::slider fn <K: Into<Key>>(&mut self, key: K, text: &str, value: f32, range: RangeInclusive<f32>, slider: &Slider) -> SliderResponse @U1b.1
pxui::Ui::slot fn (&mut self, slot: PxuiSlot) -> bool @HEAD
-pxui::Ui::spinner fn <K: Into<Key>>(&mut self, key: K, name: &str) -> Handle @U1b.5
pxui::Ui::spinner fn <K: Into<Key>>(&mut self, key: K, name: &str, spinner: &Spinner) -> Handle @U1b.5
-pxui::Ui::split fn (&mut self, label: &str, axis: Axis, ratio: f32, min_a: f32, min_b: f32, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> Option<f32> @U1b.1
pxui::Ui::split fn <K: Into<Key>>(&mut self, key: K, ratio: f32, split: &Split, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> SplitResponse @U1b.1
pxui::Ui::stack fn <F: FnMut(&mut Ui)>(&mut self, node: Node, f: F) -> Handle @U1b.6
-pxui::Ui::stack fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.6
pxui::Ui::state fn (&self, box_: &Handle, default: i64) -> i64 @HEAD
pxui::Ui::style fn (&mut self, box_: &Handle, states: &States, state: State) @HEAD
pxui::Ui::system_appearance fn (&self) -> Appearance @U1a.4
pxui::Ui::table fn <K: Into<Key>>(&mut self, key: K, columns: &[Column<'_>], rows: usize, row_key: &dyn Fn(usize) -> i64, state: &mut TableState, table: &Table, cell: &mut dyn FnMut(&mut Ui, usize, usize, CellState)) -> TableResponse @U2b.1
pxui::Ui::table_head fn <K: Into<Key>>(&mut self, key: K, columns: &[Column<'_>], state: &mut TableState, head: &TableHead) -> HeadResponse @U2b.1
-pxui::Ui::table_view fn <K: Into<Key>>(&mut self, key: K, titles: &[&str], widths: &mut [f32], rows: usize, row_key: &dyn Fn(usize) -> i64, selected: &mut Option<usize>, table: &TableView, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> TableResponse @U2b.1
-pxui::Ui::table_view fn <K: Into<Key>>(&mut self, key: K, titles: &[&str], widths: &mut [f32], rows: usize, selected: &mut Option<usize>, table: &TableView, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> TableResponse @U2b.1
pxui::Ui::tabs fn <K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, tabs: &Tabs<'_>) -> PickResponse @U2a.2
pxui::Ui::text fn (&mut self, box_: &Handle, value: &str) @HEAD
pxui::Ui::text_area fn <K: Into<Key>>(&mut self, key: K, value: &str, node: Node, area: &TextArea<'_>, callbacks: &AreaCallbacks<'_>) -> TextAreaResponse<'_> @U1b.5
-pxui::Ui::text_area fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, area: &TextArea<'_>, callbacks: &AreaCallbacks<'_>) -> TextAreaResponse<'_> @U1b.5
pxui::Ui::text_field fn <K: Into<Key>>(&mut self, key: K, value: &str, field: &TextField<'_>) -> TextFieldResponse<'_> @HEAD
pxui::Ui::text_input_focused fn (&self) -> bool @HEAD
-pxui::Ui::text_input_options fn (&self) -> TextTraits @U1b.2
pxui::Ui::text_layout_count fn (&self) -> u64 @HEAD
-pxui::Ui::text_line_box fn (&mut self, box_: &Handle, text: &str, style: &Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::Ui::text_line_box fn (&mut self, box_: &Handle, text: &str, style: Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::Ui::text_service fn (&mut self) -> UiText<'_> @HEAD
-pxui::Ui::text_spans fn (&mut self, box_: &Handle, text: &str, style: &Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::Ui::text_spans fn (&mut self, box_: &Handle, text: &str, style: Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::Ui::text_state fn (&self, b: &Handle) -> Option<&str> @HEAD
-pxui::Ui::text_top fn (&self, size: Option<i64>, y: f32, h: f32) -> f32 @U1b.3
pxui::Ui::text_traits fn (&self) -> TextTraits @U1b.2
pxui::Ui::text_undo_bytes fn (&self) -> usize @HEAD
-pxui::Ui::text_width fn (&mut self, size: Option<i64>, text: &str) -> f32 @U1b.3
pxui::Ui::theme fn (&self) -> &Theme @HEAD
pxui::Ui::themed fn (&mut self, theme: &ThemeHandle, f: &mut dyn FnMut(&mut Ui)) @U1a.2
pxui::Ui::to_front fn (&mut self, order: i64, b: &Handle) @HEAD
pxui::Ui::toasts fn <K: Into<Key>>(&mut self, key: K, items: &[Toast<'_>], seconds: f64) -> Chosen @U1b.1
-pxui::Ui::toasts fn <K: Into<Key>>(&mut self, key: K, items: &[Toast<'_>], seconds: f64) -> Option<usize> @U1b.1
-pxui::Ui::toggle fn (&mut self, text: &str, on: bool) -> bool @U1b.6
pxui::Ui::toggle fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, toggle: &Toggle<'_>) -> Response @U1b.6
-pxui::Ui::toggle_with fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, toggle: &Toggle<'_>) -> Response @U1b.6
pxui::Ui::tokens fn (&self) -> Tokens @HEAD
pxui::Ui::tooltip fn (&mut self, box_: &Handle, text: &str) @HEAD
pxui::Ui::transfer_carry fn (&mut self, to: &Handle) -> bool @HEAD
-pxui::Ui::tree fn <K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse @HEAD
-pxui::Ui::tree fn <K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], revision: Option<u64>, cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse @U2b.4.28
pxui::Ui::tree fn <K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], state: &mut TreeState, cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse @U2b.4.28
pxui::Ui::unfocus fn (&mut self) @HEAD
-pxui::Ui::use_theme fn (&mut self, theme: &Theme) @U1a.2
-pxui::Ui::value_field fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &ValueField<'_>, callbacks: &ValueCallbacks<'_>) -> ValueFieldResponse<'_> @U1c.3
-pxui::Ui::verify_new_views fn (on: bool) @U1c.2
pxui::Ui::verify_views fn (&mut self, on: bool) @HEAD
pxui::Ui::verifying fn (&self) -> bool @HEAD
-pxui::Ui::view_backdrop_blur fn (&mut self, b: &Handle, sigma: f32) @U1b.4
-pxui::Ui::view_backdrop_saturation fn (&mut self, b: &Handle, saturation: f32) @U1b.4
-pxui::Ui::view_clip fn (&mut self, b: &Handle, clip: Option<Rect>) @U1b.4
pxui::Ui::view_of fn <K: Into<Key>, T: Hash + ?Sized, F: FnMut(&mut Ui)>(&mut self, key: K, inputs: &T, f: F) -> Handle @HEAD
-pxui::Ui::view_size fn (&self) -> (f32, f32) @U1b.3
pxui::Ui::view_size fn (&self) -> ViewSize @U1b.3
pxui::Ui::wants_frame fn (&self, events: &[Event], now: f64) -> bool @HEAD
pxui::Ui::within fn (&mut self, box_: &Handle, f: &mut dyn FnMut(&mut Ui)) @HEAD
pxui::UiDesc impl Clone @U1a.3
pxui::UiDesc impl Copy @U1a.3
pxui::UiDesc struct <'a> @U1a.3
pxui::UiDesc::DEFAULT const UiDesc<'a> @U1a.3
pxui::UiDesc::font_size fn const (self, points: f32) -> UiDesc<'a> @U1a.3
pxui::UiDesc::fonts fn const (self, fonts: &'a [FontData<'a>]) -> UiDesc<'a> @U1a.3
pxui::UiDesc::new fn const (platform: Platform) -> UiDesc<'a> @U1a.3
-pxui::UiSaved type = PxuiSaved @U1a.3
pxui::UiText struct <'ui>(_) @HEAD
pxui::UiText::add_font fn (&mut self, bytes: &[u8]) -> Result<u32, Error> @HEAD
pxui::UiText::enable_system_fonts fn (&mut self) -> Result<(), Error> @U1b.3
pxui::UiText::fork fn (&mut self) -> Result<Text, Error> @HEAD
pxui::UiText::install fn (&mut self, prepared: PreparedText) -> Result<RetiredText, (Error, PreparedText)> @HEAD
-pxui::UiText::measure fn (&mut self, value: &str, style: &Label, max_width: f32) -> Result<TextMetrics, Error> @U1b.3
pxui::UiText::measure fn (&mut self, value: &str, style: Label, max_width: Option<f32>) -> Result<TextMetrics, Error> @U1b.3
-pxui::UiText::measure_spans fn (&mut self, value: &str, style: &Label, max_width: f32, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1b.3
pxui::UiText::measure_spans fn (&mut self, value: &str, style: Label, max_width: Option<f32>, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1b.3
-pxui::UiText::quads fn (&mut self, value: &str, style: &Label, max_width: f32, origin: (f32, f32), scale: f32, out: &mut Vec<TextGlyph>) -> Result<usize, Error> @U1b.3
pxui::UiText::quads fn (&mut self, value: &str, style: Label, max_width: Option<f32>, origin: Point, scale: f32, out: &mut Vec<TextGlyph>) -> Result<usize, Error> @U1b.3
-pxui::UiText::rasterize_rgba fn (&mut self, value: &str, style: &Label, max_width: f32, scale: f32, color: [u8; 4], out: &mut Vec<u8>) -> Result<(u32, u32), Error> @U1b.3
pxui::UiText::rasterize_rgba fn (&mut self, value: &str, style: Label, max_width: Option<f32>, scale: f32, color: [u8; 4], out: &mut Vec<u8>) -> Result<(u32, u32), Error> @U1b.3
-pxui::UiText::width fn (&mut self, value: &str, style: &Label, max_width: f32) -> Result<f32, Error> @U1b.3
pxui::UiText::width fn (&mut self, value: &str, style: Label, max_width: Option<f32>) -> Result<f32, Error> @U1b.3
pxui::ValidFn type <'a>= dyn Fn(&str) -> bool + 'a @HEAD
pxui::ValueCallbacks impl Clone @HEAD
pxui::ValueCallbacks impl Copy @HEAD
pxui::ValueCallbacks struct <'a> @HEAD
pxui::ValueCallbacks::DEFAULT const ValueCallbacks<'a> @HEAD
pxui::ValueCallbacks::scrub fn const (self, scrub: &'a ScrubFn<'a>) -> ValueCallbacks<'a> @HEAD
pxui::ValueCallbacks::slide fn const (self, slide: &'a SlideFn<'a>) -> ValueCallbacks<'a> @HEAD
pxui::ValueCallbacks::valid fn const (self, valid: &'a ValidFn<'a>) -> ValueCallbacks<'a> @HEAD
pxui::ValueField impl Clone @HEAD
pxui::ValueField impl Copy @HEAD
pxui::ValueField impl Debug @HEAD
pxui::ValueField impl PartialEq @HEAD
pxui::ValueField impl StructuralPartialEq @HEAD
pxui::ValueField struct <'a> @HEAD
pxui::ValueField::DEFAULT const ValueField<'a> @HEAD
pxui::ValueField::bare fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ValueField::display fn const (self, text: &'a str) -> ValueField<'a> @HEAD
pxui::ValueField::edit fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ValueField::fraction fn const (self, fraction: f32) -> ValueField<'a> @HEAD
pxui::ValueField::kept_drag fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ValueField::lead fn const (self, text: &'a str, color: Rgba) -> ValueField<'a> @HEAD
pxui::ValueField::left fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ValueField::line fn const (self, color: Rgba) -> ValueField<'a> @HEAD
pxui::ValueField::name fn const (self, name: &'a str) -> ValueField<'a> @HEAD
pxui::ValueField::placeholder fn const (self, text: &'a str) -> ValueField<'a> @HEAD
pxui::ValueField::size fn const (self, points: usize) -> ValueField<'a> @HEAD
pxui::ValueField::tracking fn const (self, tracking: f32) -> ValueField<'a> @HEAD
pxui::ValueField::trail fn const (self, text: &'a str, color: Rgba) -> ValueField<'a> @HEAD
pxui::ValueFieldResponse impl Clone @HEAD
pxui::ValueFieldResponse impl Copy @HEAD
pxui::ValueFieldResponse impl Debug @HEAD
pxui::ValueFieldResponse struct <'a> @HEAD
pxui::ValueFieldResponse::changed field Option<&'a str> @HEAD
pxui::ValueFieldResponse::editing field bool @HEAD
pxui::ValueFieldResponse::response field Response @HEAD
pxui::ViewSize impl Clone @U1b.3
pxui::ViewSize impl Copy @U1b.3
pxui::ViewSize impl Debug @U1b.3
pxui::ViewSize impl Default @U1b.3
pxui::ViewSize impl PartialEq @U1b.3
pxui::ViewSize impl StructuralPartialEq @U1b.3
pxui::ViewSize struct @U1b.3
pxui::ViewSize::height field f32 @U1b.3
pxui::ViewSize::width field f32 @U1b.3
pxui::ViewTransform impl Clone @HEAD
pxui::ViewTransform impl Copy @HEAD
pxui::ViewTransform impl Debug @HEAD
pxui::ViewTransform impl Default @HEAD
pxui::ViewTransform impl PartialEq @HEAD
pxui::ViewTransform impl StructuralPartialEq @HEAD
pxui::ViewTransform struct @HEAD
pxui::ViewTransform::DEFAULT const ViewTransform @HEAD
pxui::ViewTransform::about fn const (self, scale: f32, matrix: [f32; 4], pivot: (f32, f32)) -> ViewTransform @HEAD
pxui::ViewTransform::blur fn const (self, blur: f32) -> ViewTransform @U1b.4
pxui::ViewTransform::clip fn const (self, clip: Option<Rect>) -> ViewTransform @U1b.4
pxui::ViewTransform::offset fn const (self, dx: f32, dy: f32) -> ViewTransform @HEAD
pxui::ViewTransform::opacity fn const (self, opacity: f32) -> ViewTransform @HEAD
pxui::ViewTransform::saturation fn const (self, saturation: f32) -> ViewTransform @U1b.4
pxui::ViewTransform::scale fn const (self, scale: f32) -> ViewTransform @HEAD
pxui::Wrap use pxui_types::text::Wrap @U1a.1
-pxui::a11y mod @U1a.1
-pxui::a11y::A11yAction use pxui_types::ffi::A11yAction @U1a.1
-pxui::a11y::A11yNode use pxui_types::ffi::A11yNode @U1a.1
-pxui::a11y::A11yTree use pxui_types::ffi::A11yTree @U1a.1
-pxui::a11y::Role use pxui_types::a11y::Role @U1a.1
-pxui::a11y::state use pxui_types::a11y::state @U1a.1
pxui::bodies mod @HEAD
pxui::bodies::BadgeConfig impl BodyConfig @HEAD
pxui::bodies::BadgeConfig impl Deref @HEAD
pxui::bodies::BadgeConfig struct (_) @HEAD
pxui::bodies::BodyConfig trait : Sealed @HEAD
pxui::bodies::BodyConfig::curves_of fn (&self, field: &*const Curve) -> Option<&[Curve; 2]> @HEAD
pxui::bodies::BodyConfig::hint_at fn (&self, list: &Slice<PxuiHintDesc>, index: usize) -> Option<(&str, &str)> @HEAD
pxui::bodies::BodyConfig::states_of fn (&self, field: &*const States) -> Option<&States> @HEAD
pxui::bodies::BodyConfig::text_at fn (&self, list: &Slice<Str>, index: usize) -> &str @HEAD
pxui::bodies::BodyConfig::text_of fn (&self, field: &Str) -> &str @HEAD
pxui::bodies::ButtonConfig impl BodyConfig @HEAD
pxui::bodies::ButtonConfig impl Deref @HEAD
pxui::bodies::ButtonConfig struct (_) @HEAD
pxui::bodies::ButtonConfig::accessory_label fn (&self) -> Option<Label> @U2b.4.1
pxui::bodies::ButtonConfig::centered fn (&self) -> bool @U4b
pxui::bodies::ButtonConfig::content_axis fn (&self) -> Option<Axis> @U2b.4.27
pxui::bodies::ButtonConfig::danger fn (&self) -> bool @U2b.4.35
pxui::bodies::ButtonConfig::detail_label fn (&self) -> Option<Label> @U2b.4.1
pxui::bodies::ButtonConfig::height fn (&self) -> Option<Size> @U2b.4.27
pxui::bodies::ButtonConfig::icon fn (&self) -> Option<Icon> @U2b.7
pxui::bodies::ButtonConfig::label fn (&self) -> Option<Label> @U2b.4.1
pxui::bodies::ButtonConfig::width fn (&self) -> Option<Size> @U2b.4.27
pxui::bodies::CheckboxConfig impl BodyConfig @HEAD
pxui::bodies::CheckboxConfig impl Deref @HEAD
pxui::bodies::CheckboxConfig struct (_) @HEAD
pxui::bodies::DialogConfig impl BodyConfig @HEAD
pxui::bodies::DialogConfig impl Deref @HEAD
pxui::bodies::DialogConfig struct (_) @HEAD
pxui::bodies::DropdownConfig impl BodyConfig @HEAD
pxui::bodies::DropdownConfig impl Deref @HEAD
pxui::bodies::DropdownConfig struct (_) @HEAD
pxui::bodies::FooterConfig impl BodyConfig @HEAD
pxui::bodies::FooterConfig impl Deref @HEAD
pxui::bodies::FooterConfig struct (_) @HEAD
pxui::bodies::GripConfig impl BodyConfig @HEAD
pxui::bodies::GripConfig impl Deref @HEAD
pxui::bodies::GripConfig struct (_) @HEAD
pxui::bodies::IconButtonConfig impl BodyConfig @HEAD
pxui::bodies::IconButtonConfig impl Deref @HEAD
pxui::bodies::IconButtonConfig struct (_) @HEAD
pxui::bodies::IconButtonConfig::icon fn (&self) -> Option<Icon> @U2b.7
pxui::bodies::InspectorRowConfig impl BodyConfig @HEAD
pxui::bodies::InspectorRowConfig impl Deref @HEAD
pxui::bodies::InspectorRowConfig struct (_) @HEAD
pxui::bodies::InspectorSectionConfig impl BodyConfig @HEAD
pxui::bodies::InspectorSectionConfig impl Deref @HEAD
pxui::bodies::InspectorSectionConfig struct (_) @HEAD
pxui::bodies::LabelConfig impl BodyConfig @HEAD
pxui::bodies::LabelConfig impl Deref @HEAD
pxui::bodies::LabelConfig struct (_) @HEAD
pxui::bodies::LineFieldConfig impl BodyConfig @HEAD
pxui::bodies::LineFieldConfig impl Deref @HEAD
pxui::bodies::LineFieldConfig struct (_) @HEAD
pxui::bodies::MenuBarConfig impl BodyConfig @HEAD
pxui::bodies::MenuBarConfig impl Deref @HEAD
pxui::bodies::MenuBarConfig struct (_) @HEAD
pxui::bodies::MenuItemConfig impl BodyConfig @HEAD
pxui::bodies::MenuItemConfig impl Deref @HEAD
pxui::bodies::MenuItemConfig struct (_) @HEAD
pxui::bodies::MenuRowConfig impl BodyConfig @HEAD
pxui::bodies::MenuRowConfig impl Deref @HEAD
pxui::bodies::MenuRowConfig struct (_) @HEAD
pxui::bodies::ModalConfig impl BodyConfig @HEAD
pxui::bodies::ModalConfig impl Deref @HEAD
pxui::bodies::ModalConfig struct (_) @HEAD
pxui::bodies::ModalConfig::presentation fn (&self) -> Presentation @U2b.4.27
pxui::bodies::PanelConfig impl BodyConfig @HEAD
pxui::bodies::PanelConfig impl Deref @HEAD
pxui::bodies::PanelConfig struct (_) @HEAD
pxui::bodies::PickerConfig::art fn (&self) -> Option<PickArt> @U2b.4.19
pxui::bodies::PickerConfig::head fn (&self) -> Option<Handle> @U2b.4.27
pxui::bodies::PickerConfig::image fn (&self) -> Option<Image> @U2b.4.27
pxui::bodies::PickerConfig impl BodyConfig @HEAD
pxui::bodies::PickerConfig impl Deref @HEAD
pxui::bodies::PickerConfig struct (_) @HEAD
pxui::bodies::PickerFieldConfig impl BodyConfig @HEAD
pxui::bodies::PickerFieldConfig impl Deref @HEAD
pxui::bodies::PickerFieldConfig struct (_) @HEAD
pxui::bodies::PopupConfig impl BodyConfig @HEAD
pxui::bodies::PopupConfig impl Deref @HEAD
pxui::bodies::PopupConfig struct (_) @HEAD
pxui::bodies::ProgressConfig::value fn (&self) -> Option<f32> @U4a
pxui::bodies::ProgressConfig impl BodyConfig @HEAD
pxui::bodies::ProgressConfig impl Deref @HEAD
pxui::bodies::ProgressConfig struct (_) @HEAD
pxui::bodies::RadioConfig impl BodyConfig @HEAD
pxui::bodies::RadioConfig impl Deref @HEAD
pxui::bodies::RadioConfig struct (_) @HEAD
pxui::bodies::ScrollViewConfig impl BodyConfig @HEAD
pxui::bodies::ScrollViewConfig impl Deref @HEAD
pxui::bodies::ScrollViewConfig struct (_) @HEAD
pxui::bodies::SearchFieldConfig impl BodyConfig @HEAD
pxui::bodies::SearchFieldConfig impl Deref @HEAD
pxui::bodies::SearchFieldConfig struct (_) @HEAD
pxui::bodies::SectionIndexConfig impl BodyConfig @U2a.3
pxui::bodies::SectionIndexConfig impl Deref @U2a.3
pxui::bodies::SectionIndexConfig struct (_) @U2a.3
pxui::bodies::SegmentedConfig::selected fn (&self) -> Option<usize> @U4a
pxui::bodies::SegmentedConfig::stored fn (&self) -> Option<usize> @U4a
pxui::bodies::SegmentedConfig impl BodyConfig @HEAD
pxui::bodies::SegmentedConfig impl Deref @HEAD
pxui::bodies::SegmentedConfig struct (_) @HEAD
pxui::bodies::SelectionListConfig impl BodyConfig @HEAD
pxui::bodies::SelectionListConfig impl Deref @HEAD
pxui::bodies::SelectionListConfig struct (_) @HEAD
pxui::bodies::SliderConfig impl BodyConfig @HEAD
pxui::bodies::SliderConfig impl Deref @HEAD
pxui::bodies::SliderConfig struct (_) @HEAD
pxui::bodies::SpinnerConfig impl BodyConfig @HEAD
pxui::bodies::SpinnerConfig impl Deref @HEAD
pxui::bodies::SpinnerConfig struct (_) @HEAD
pxui::bodies::SplitConfig impl BodyConfig @HEAD
pxui::bodies::SplitConfig impl Deref @HEAD
pxui::bodies::SplitConfig struct (_) @HEAD
-pxui::bodies::TableCellConfig impl BodyConfig @U2b.1
-pxui::bodies::TableCellConfig impl Deref @U2b.1
-pxui::bodies::TableCellConfig struct (_) @U2b.1
pxui::bodies::TableHeadConfig::ascending fn (&self) -> Option<bool> @U4a
pxui::bodies::TableHeadConfig impl BodyConfig @HEAD
pxui::bodies::TableHeadConfig impl Deref @HEAD
pxui::bodies::TableHeadConfig struct (_) @HEAD
pxui::bodies::TableRowConfig impl BodyConfig @HEAD
pxui::bodies::TableRowConfig impl Deref @HEAD
pxui::bodies::TableRowConfig struct (_) @HEAD
pxui::bodies::TabsConfig impl BodyConfig @HEAD
pxui::bodies::TabsConfig impl Deref @HEAD
pxui::bodies::TabsConfig struct (_) @HEAD
pxui::bodies::TextFieldConfig impl BodyConfig @HEAD
pxui::bodies::TextFieldConfig impl Deref @HEAD
pxui::bodies::TextFieldConfig struct (_) @HEAD
pxui::bodies::ToastsConfig impl BodyConfig @HEAD
pxui::bodies::ToastsConfig impl Deref @HEAD
pxui::bodies::ToastsConfig struct (_) @HEAD
pxui::bodies::ToggleConfig impl BodyConfig @HEAD
pxui::bodies::ToggleConfig impl Deref @HEAD
pxui::bodies::ToggleConfig struct (_) @HEAD
pxui::bodies::TooltipConfig impl BodyConfig @HEAD
pxui::bodies::TooltipConfig impl Deref @HEAD
pxui::bodies::TooltipConfig struct (_) @HEAD
pxui::bodies::TreeConfig impl BodyConfig @HEAD
pxui::bodies::TreeConfig impl Deref @HEAD
pxui::bodies::TreeConfig struct (_) @HEAD
pxui::bodies::ValueFieldConfig impl BodyConfig @HEAD
pxui::bodies::ValueFieldConfig impl Deref @HEAD
pxui::bodies::ValueFieldConfig struct (_) @HEAD
pxui::bodies::WindowConfig impl BodyConfig @HEAD
pxui::bodies::WindowConfig impl Deref @HEAD
pxui::bodies::WindowConfig struct (_) @HEAD
-pxui::collections mod @U1a.1
-pxui::collections::TreeNode impl Clone @U1a.1
-pxui::collections::TreeNode impl Copy @U1a.1
-pxui::collections::TreeNode impl Debug @U1a.1
-pxui::collections::TreeNode impl Default @U1a.1
-pxui::collections::TreeNode impl Eq @U1a.1
-pxui::collections::TreeNode impl PartialEq @U1a.1
-pxui::collections::TreeNode impl StructuralPartialEq @U1a.1
-pxui::collections::TreeNode struct @U1a.1
-pxui::collections::TreeNode::children field u8 @U1a.1
-pxui::collections::TreeNode::depth field u16 @U1a.1
-pxui::composite mod @U1a.1
-pxui::doc mod @U1a.1
-pxui::doc::frames fn (n: usize, build: &mut dyn FnMut(&mut Ui)) -> Ui @U1a.1
pxui::draw use pxui_types::draw @U1a.1
pxui::event_text fn (event: &Event) -> &str @HEAD
pxui::ext mod @U1c.3
pxui::ext::field_drag fn (ui: &Ui, field: &Handle) -> Option<(f32, f32)> @U1c.3
pxui::ext::footer fn (ui: &mut Ui, right: Option<&str>, hints: &[(&str, &str)]) @U1c.3
pxui::ext::grip fn (ui: &mut Ui, label: Arguments<'_>, axis: Axis, rect: Option<Rect>) -> (Handle, Signal) @U1c.3
pxui::ext::inspector_bar fn (ui: &mut Ui, hints: &[(&str, &str)], count: i64) @U1c.3
pxui::ext::inspector_body fn (ui: &mut Ui, f: &mut dyn FnMut(&mut Ui)) @U1c.3
pxui::ext::inspector_button fn (ui: &mut Ui, key: &str, label: &str) -> bool @U1c.3
pxui::ext::inspector_header fn <'ui>(ui: &'ui mut Ui, key: &str, kind: Option<&str>, badge: Option<&str>, index: Option<&str>, rename: Option<Rc<dyn Fn(&str) -> bool>>, actions: &[HeadAction], reset: Option<&str>, reset_enabled: bool, title: &str, detail: &str) -> Head<'ui> @U5.9
pxui::ext::inspector_label_x fn (ui: &Ui) -> f32 @U1c.3
pxui::ext::inspector_message fn (ui: &mut Ui, key: &str, message: &str) @U1c.3
pxui::ext::inspector_readout fn (ui: &mut Ui, width: Option<f32>, key: &str, label: &str, value: &str) @U1c.3
pxui::ext::inspector_row fn (ui: &mut Ui, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (Handle, f32, f32, f32) @U1c.3
pxui::ext::inspector_section fn (ui: &mut Ui, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, build: &mut dyn FnMut(&mut Ui)) -> bool @U1c.3
pxui::ext::inspector_toggle fn (ui: &mut Ui, key: &str, label: &str, value: bool) -> bool @U1c.3
pxui::ext::inspector_width fn (ui: &Ui) -> f32 @U1c.3
pxui::ext::line_field fn <'a, K: Into<Key>>(ui: &'a mut Ui, key: K, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'a> @U1c.3
pxui::ext::value_field fn <'a, K: Into<Key>>(ui: &'a mut Ui, key: K, value: &str, rect: Rect, field: &ValueField<'_>, callbacks: &ValueCallbacks<'_>) -> ValueFieldResponse<'a> @U1c.3
pxui::geom use pxui_types::geom @U1a.1
pxui::guide mod @U1a.1
pxui::hash_of fn <T: Hash + ?Sized>(value: &T) -> u64 @HEAD
-pxui::image mod @U1a.1
-pxui::image::ImageStyle impl Clone @U1a.1
-pxui::image::ImageStyle impl Copy @U1a.1
-pxui::image::ImageStyle impl Debug @U1a.1
-pxui::image::ImageStyle struct @U1a.1
-pxui::image::ImageStyle::DEFAULT const ImageStyle @U1a.1
-pxui::image::ImageStyle::fit fn const (self, fit: Fit) -> ImageStyle @U1a.1
-pxui::image::ImageStyle::h fn const (self, size: Size) -> ImageStyle @U1a.1
-pxui::image::ImageStyle::h_px fn const (self, points: f32) -> ImageStyle @U1a.1
-pxui::image::ImageStyle::w fn const (self, size: Size) -> ImageStyle @U1a.1
-pxui::image::ImageStyle::w_px fn const (self, points: f32) -> ImageStyle @U1a.1
pxui::images mod @HEAD
pxui::images::BYTE_LIMIT const usize @HEAD
pxui::images::CAPACITY const usize @HEAD
pxui::images::Images impl Default @HEAD
pxui::images::Images struct @HEAD
pxui::images::Images::cancel_requests fn (&self) @HEAD
pxui::images::Images::clear fn (&mut self) @HEAD
pxui::images::Images::configure fn (&mut self, density: f32, max_pixels: u32) -> bool @HEAD
pxui::images::Images::epoch fn (&self) -> u64 @HEAD
pxui::images::Images::get fn (&self, id: usize, path: &str, points: f32, node: Option<&Handle>) -> Option<Image> @HEAD
pxui::images::Images::oldest_image fn (&self) -> Option<usize> @HEAD
pxui::images::Images::quality fn (&self, id: usize) -> Option<u32> @HEAD
pxui::images::Images::remove fn (&mut self, id: usize) -> bool @HEAD
pxui::images::Images::requests fn (&self, ui: Option<&Ui>, out: &mut Request) @HEAD
pxui::images::Images::revision fn (&self) -> u64 @HEAD
pxui::images::Images::store fn (&mut self, want: Want, image: Option<Image>) -> Option<usize> @HEAD
pxui::images::PATH_ARENA_BYTES const usize @HEAD
pxui::images::PATH_LIMIT const usize @HEAD
pxui::images::REQUEST_CAPACITY const usize @HEAD
pxui::images::REQUEST_PATH_BYTES const usize @HEAD
pxui::images::Request impl Clone @HEAD
pxui::images::Request impl Debug @HEAD
pxui::images::Request impl Default @HEAD
pxui::images::Request impl Eq @HEAD
pxui::images::Request impl PartialEq @HEAD
pxui::images::Request struct @HEAD
pxui::images::Request::clear fn (&mut self) @HEAD
pxui::images::Request::epoch fn (&self) -> u64 @HEAD
pxui::images::Request::insert fn (&mut self, want: Want, path: &str) -> bool @HEAD
pxui::images::Request::path fn (&self, index: usize) -> &str @HEAD
pxui::images::Request::wants fn (&self) -> &[Want] @HEAD
pxui::images::Want impl Clone @HEAD
pxui::images::Want impl Copy @HEAD
pxui::images::Want impl Debug @HEAD
pxui::images::Want impl Eq @HEAD
pxui::images::Want impl PartialEq @HEAD
pxui::images::Want impl StructuralPartialEq @HEAD
pxui::images::Want struct @HEAD
pxui::images::Want::id fn (&self) -> usize @HEAD
pxui::images::Want::max_pixels fn (&self) -> u32 @HEAD
pxui::images::Want::new fn (id: usize, max_pixels: u32) -> Option<Want> @HEAD
-pxui::kit mod @U1a.1
pxui::library_counter fn (which: u32) -> u64 @HEAD
-pxui::overlays mod @U1a.1
-pxui::paint mod @U1a.1
-pxui::paint::Fill impl Clone @U1a.1
-pxui::paint::Fill impl Copy @U1a.1
-pxui::paint::Fill impl Debug @U1a.1
-pxui::paint::Fill impl PartialEq @U1a.1
-pxui::paint::Fill impl StructuralPartialEq @U1a.1
-pxui::paint::Fill struct @U1a.1
-pxui::paint::Fill::DEFAULT const Fill @U1a.1
-pxui::paint::Fill::fractional fn const (self) -> Fill @U1a.1
-pxui::paint::Fill::new fn const (color: Rgba) -> Fill @U1a.1
-pxui::paint::Paint struct <'a> @U1a.1
-pxui::paint::Paint::brackets fn (&mut self, rect: Rect, offset: f32, length: f32, stroke: Stroke) @U1a.1
-pxui::paint::Paint::chevron fn (&mut self, at: (f32, f32), direction: Dir, stroke: Stroke) @U1a.1
-pxui::paint::Paint::circle fn (&mut self, at: (f32, f32), radius: f32, fill: Option<Fill>, stroke: Option<Stroke>) @U1a.1
-pxui::paint::Paint::clip_radius fn (&self) -> f32 @U1a.1
-pxui::paint::Paint::color fn (&self, token: ColorToken) -> Rgba @U1a.1
-pxui::paint::Paint::cubic fn (&mut self, points: [(f32, f32); 4], stroke: Stroke) @U1a.1
-pxui::paint::Paint::curve fn (&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32), stroke: Stroke) @U1a.1
-pxui::paint::Paint::dashed fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @U1a.1
-pxui::paint::Paint::dashed_rect fn (&mut self, rect: Rect, stroke: Stroke) @U1a.1
-pxui::paint::Paint::density fn (&self) -> i64 @U1a.1
-pxui::paint::Paint::fill fn (&mut self, rect: Rect, fill: Fill) @U1a.1
-pxui::paint::Paint::flag fn (&mut self, at: (f32, f32), round: bool, on: bool) @U1a.1
-pxui::paint::Paint::frame fn (&mut self, rect: Rect, stroke: Stroke) @U1a.1
-pxui::paint::Paint::grid fn (&mut self, rect: Rect, origin: (f32, f32), spacing: f32, dot: f32, fill: Fill) @U1a.1
-pxui::paint::Paint::icon fn (&mut self, at: (f32, f32), font: u16, size: f32, ch: char, color: Rgba) -> f32 @U1a.1
-pxui::paint::Paint::image fn (&mut self, rect: Rect, image: &Image, fit_mode: Fit, tint: Rgba) -> Result<(), Error> @U1a.1
-pxui::paint::Paint::line fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @U1a.1
-pxui::paint::Paint::rtl fn (&self) -> bool @U1a.1
-pxui::paint::Paint::scroll_y fn (&mut self, slot: u64) -> f32 @U1a.1
-pxui::paint::Paint::stroke fn (&mut self, rect: Rect, stroke: Stroke) @U1a.1
-pxui::paint::Paint::text fn (&mut self, at: (f32, f32), style: &Label, width: f32, s: &str) -> TextMetrics @U1a.1
-pxui::paint::Paint::text_chars fn (&mut self, at: (f32, f32), style: &Label, s: &str) -> f32 @U1a.1
-pxui::paint::Paint::text_metrics fn (&mut self, style: &TextStyle, width: f32, s: &str) -> TextMetrics @U1a.1
-pxui::paint::Paint::text_spans fn (&mut self, at: (f32, f32), style: &TextStyle, width: f32, text: &str, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1a.1
-pxui::paint::Paint::theme fn (&self) -> Tokens @U1a.1
-pxui::paint::Paint::triangle fn (&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), fill: Fill) @U1a.1
-pxui::paint::Stroke impl Clone @U1a.1
-pxui::paint::Stroke impl Copy @U1a.1
-pxui::paint::Stroke impl Debug @U1a.1
-pxui::paint::Stroke impl PartialEq @U1a.1
-pxui::paint::Stroke impl StructuralPartialEq @U1a.1
-pxui::paint::Stroke struct @U1a.1
-pxui::paint::Stroke::DEFAULT const Stroke @U1a.1
-pxui::paint::Stroke::MAX_WIDTH const f32 @U1a.1
-pxui::paint::Stroke::new fn const (color: Rgba) -> Stroke @U1a.1
-pxui::paint::Stroke::width fn const (self, width: f32) -> Stroke @U1a.1
pxui::pragmasevka fn () -> &'static [u8] @U1a.1
pxui::supports fn (feature: Feature) -> bool @HEAD
pxui::testing mod @HEAD
pxui::testing::click fn (ui: &mut Ui, x: f64, y: f64, build: &mut dyn FnMut(&mut Ui)) @HEAD
pxui::testing::click_node fn (ui: &mut Ui, node: &Handle, build: &mut dyn FnMut(&mut Ui)) @HEAD
pxui::testing::find fn (ui: &Ui, role: Role, name: &str) -> Option<Rect> @HEAD
pxui::testing::frame fn (ui: &mut Ui, events: &[Event], build: &mut dyn FnMut(&mut Ui)) @HEAD
pxui::testing::key fn (ui: &mut Ui, key: KeyCode, build: &mut dyn FnMut(&mut Ui)) @U1a.4
-pxui::text mod @U1a.1
-pxui::text::Ellipsis use pxui_types::text::Ellipsis @U1a.1
-pxui::text::PreparedText impl Drop @U1a.1
-pxui::text::PreparedText impl Send @U1a.1
-pxui::text::PreparedText struct @U1a.1
-pxui::text::PreparedText::metrics fn (&self) -> TextMetrics @U1a.1
-pxui::text::RetiredText impl Drop @U1a.1
-pxui::text::RetiredText impl Send @U1a.1
-pxui::text::RetiredText struct (_) @U1a.1
-pxui::text::Text impl Drop @U1a.1
-pxui::text::Text impl Send @U1a.1
-pxui::text::Text struct (_) @U1a.1
-pxui::text::Text::add_font fn (&mut self, bytes: &[u8]) -> Result<u32, Error> @U1a.1
-pxui::text::Text::enable_system_fonts fn (&mut self) -> Result<(), Error> @U1a.1
-pxui::text::Text::measure fn (&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<TextMetrics, Error> @U1a.1
-pxui::text::Text::measure_spans fn (&mut self, value: &str, style: &TextStyle, max_width: f32, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1a.1
-pxui::text::Text::new fn (fonts: &[&[u8]]) -> Result<Self, Error> @U1a.1
-pxui::text::Text::prepare fn (&mut self, value: &str, style: &TextStyle, max_width: f32, spans: &[TextSpan]) -> Result<PreparedText, Error> @U1a.1
-pxui::text::Text::rasterize_rgba fn (&mut self, value: &str, style: &TextStyle, max_width: f32, scale: f32, color: [u8; 4], out: &mut Vec<u8>) -> Result<(u32, u32), Error> @U1a.1
-pxui::text::Text::width fn (&mut self, value: &str, style: &TextStyle, max_width: f32) -> Result<f32, Error> @U1a.1
-pxui::text::TextAlign use pxui_types::text::TextAlign @U1a.1
-pxui::text::TextStyle impl Clone @U1a.1
-pxui::text::TextStyle impl Copy @U1a.1
-pxui::text::TextStyle impl Debug @U1a.1
-pxui::text::TextStyle impl PartialEq @U1a.1
-pxui::text::TextStyle impl StructuralPartialEq @U1a.1
-pxui::text::TextStyle struct (_, _) @U1a.1
-pxui::text::TextStyle::line_height fn (&self) -> f32 @U1a.1
-pxui::text::TextStyle::size fn (&self) -> f32 @U1a.1
-pxui::text::TextStyle::tracking fn (&self) -> f32 @U1a.1
-pxui::text::TextWorker impl Drop @U1a.1
-pxui::text::TextWorker impl Send @U1a.1
-pxui::text::TextWorker impl Sync @U1a.1
-pxui::text::TextWorker struct (_) @U1a.1
-pxui::text::TextWorker::new fn () -> Result<TextWorker, Error> @U1a.1
-pxui::text::UiText struct <'ui>(_) @U1a.1
-pxui::text::UiText::add_font fn (&mut self, bytes: &[u8]) -> Result<u32, Error> @U1a.1
-pxui::text::UiText::fork fn (&mut self) -> Result<Text, Error> @U1a.1
-pxui::text::UiText::install fn (&mut self, prepared: PreparedText) -> Result<RetiredText, (Error, PreparedText)> @U1a.1
-pxui::text::UiText::measure fn (&mut self, value: &str, style: &Label, max_width: f32) -> Result<TextMetrics, Error> @U1a.1
-pxui::text::UiText::measure_spans fn (&mut self, value: &str, style: &Label, max_width: f32, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1a.1
-pxui::text::UiText::quads fn (&mut self, value: &str, style: &Label, max_width: f32, origin: (f32, f32), scale: f32, out: &mut Vec<TextGlyph>) -> Result<usize, Error> @U1a.1
-pxui::text::UiText::rasterize_rgba fn (&mut self, value: &str, style: &Label, max_width: f32, scale: f32, color: [u8; 4], out: &mut Vec<u8>) -> Result<(u32, u32), Error> @U1a.1
-pxui::text::UiText::width fn (&mut self, value: &str, style: &Label, max_width: f32) -> Result<f32, Error> @U1a.1
-pxui::text::Wrap use pxui_types::text::Wrap @U1a.1
-pxui::text::pragmasevka fn () -> &'static [u8] @U1a.1
pxui::theme use pxui_types::theme @U1a.1
pxui::ui mod @HEAD
pxui::ui::* use pxui_types::draw::* @HEAD
pxui::ui::* use pxui_types::geom::* @HEAD
pxui::ui::* use pxui_types::layout::* @HEAD
-pxui::ui::* use pxui_types::ui::* @U1a.1
pxui::ui::Axis use pxui_types::node::Axis @HEAD
pxui::ui::Bar use pxui_types::node::Bar @HEAD
pxui::ui::Checkbox use pxui_types::ui::Checkbox @U1a.1
-pxui::ui::Choice use pxui_types::ui::Choice @U2a.2
-pxui::ui::Chosen use crate::Chosen @U1a.1
pxui::ui::Chosen use pxui_types::ui::Chosen @HEAD
pxui::ui::CodeContext use pxui_types::ui::CodeContext @HEAD
pxui::ui::Completion use pxui_types::ui::Completion @HEAD
pxui::ui::Cursor use pxui_types::ui::Cursor @U1a.1
pxui::ui::Curve use pxui_types::theme::Curve @HEAD
pxui::ui::DRAG_THRESHOLD use pxui_types::ui::DRAG_THRESHOLD @U1a.1
pxui::ui::Dialog use crate::Dialog @HEAD
pxui::ui::DialogButton use crate::DialogButton @HEAD
pxui::ui::Dir use pxui_types::node::Dir @HEAD
-pxui::ui::DismissOpts impl Clone @U2a.4
-pxui::ui::DismissOpts impl Copy @U2a.4
-pxui::ui::DismissOpts impl Debug @U2a.4
-pxui::ui::DismissOpts impl Default @U2a.4
-pxui::ui::DismissOpts impl PartialEq @U2a.4
-pxui::ui::DismissOpts impl StructuralPartialEq @U2a.4
-pxui::ui::DismissOpts struct @U2a.4
pxui::ui::DismissOpts use crate::DismissOpts @U2a.4
-pxui::ui::DismissOpts::DEFAULT const DismissOpts @U2a.4
-pxui::ui::DismissOpts::fraction fn const (self, fraction: f32) -> DismissOpts @U2a.4
-pxui::ui::DismissOpts::velocity fn const (self, velocity: f32) -> DismissOpts @U2a.4
pxui::ui::Dropdown use pxui_types::ui::Dropdown @U1a.1
pxui::ui::Fill impl Clone @HEAD
pxui::ui::Fill impl Copy @HEAD
pxui::ui::Fill impl Debug @HEAD
pxui::ui::Fill impl PartialEq @HEAD
pxui::ui::Fill impl StructuralPartialEq @HEAD
pxui::ui::Fill struct @HEAD
pxui::ui::Fill::DEFAULT const Fill @HEAD
pxui::ui::Fill::fractional fn const (self) -> Fill @HEAD
pxui::ui::Fill::new fn const (color: Rgba) -> Fill @HEAD
pxui::ui::Flags use pxui_types::Flags @HEAD
pxui::ui::Font use pxui_types::theme::Font @HEAD
pxui::ui::GestureOpts impl Clone @HEAD
pxui::ui::GestureOpts impl Copy @HEAD
pxui::ui::GestureOpts impl Debug @HEAD
pxui::ui::GestureOpts impl Default @HEAD
pxui::ui::GestureOpts impl PartialEq @HEAD
pxui::ui::GestureOpts impl StructuralPartialEq @HEAD
pxui::ui::GestureOpts struct @HEAD
pxui::ui::GestureOpts::DEFAULT const GestureOpts @HEAD
pxui::ui::GestureOpts::dismiss fn const (self, dismiss: DismissOpts) -> GestureOpts @HEAD
pxui::ui::GestureOpts::edge_back fn const (self) -> GestureOpts @HEAD
pxui::ui::GestureOpts::long_press fn const (self) -> GestureOpts @HEAD
pxui::ui::GestureOpts::pull_to_refresh fn const (self) -> GestureOpts @HEAD
pxui::ui::GestureOpts::refresh fn const (self, points: f32) -> GestureOpts @HEAD
pxui::ui::GestureOpts::row_swipe fn const (self, width: f32) -> GestureOpts @HEAD
pxui::ui::GestureOpts::swipe_actions fn const (self, actions: SwipeActions) -> GestureOpts @HEAD
pxui::ui::GestureOpts::touch_drag fn const (self) -> GestureOpts @HEAD
pxui::ui::GestureState impl Clone @HEAD
pxui::ui::GestureState impl Copy @HEAD
pxui::ui::GestureState impl Debug @HEAD
pxui::ui::GestureState impl Default @HEAD
pxui::ui::GestureState impl PartialEq @HEAD
pxui::ui::GestureState impl StructuralPartialEq @HEAD
pxui::ui::GestureState struct @HEAD
pxui::ui::GestureState::back field f32 @HEAD
pxui::ui::GestureState::back_active field bool @HEAD
pxui::ui::GestureState::back_cancelled field bool @HEAD
pxui::ui::GestureState::back_committed field bool @HEAD
pxui::ui::GestureState::dismiss field f32 @HEAD
pxui::ui::GestureState::dismiss_cancelled field bool @HEAD
pxui::ui::GestureState::dismiss_committed field bool @HEAD
pxui::ui::GestureState::dismissing field bool @HEAD
pxui::ui::GestureState::highlighted field bool @HEAD
pxui::ui::GestureState::long_pressed field bool @HEAD
pxui::ui::GestureState::pull field f32 @HEAD
pxui::ui::GestureState::refresh_fired field bool @HEAD
pxui::ui::GestureState::refreshing field bool @HEAD
pxui::ui::GestureState::swipe field f32 @HEAD
pxui::ui::GestureState::swipe_commit_leading field bool @HEAD
pxui::ui::GestureState::swipe_commit_trailing field bool @HEAD
pxui::ui::GestureState::swipe_open field bool @HEAD
pxui::ui::GestureState::swipe_open_leading field bool @HEAD
pxui::ui::GestureState::swipe_open_trailing field bool @HEAD
pxui::ui::GestureState::swiping field bool @HEAD
pxui::ui::Handle type = PxuiBox @HEAD
pxui::ui::Head use pxui_types::ui::Head @U1a.1
pxui::ui::HeadAction use pxui_types::ui::HeadAction @U1a.1
pxui::ui::Image use pxui_types::Image @HEAD
pxui::ui::ImageStyle impl Clone @HEAD
pxui::ui::ImageStyle impl Copy @HEAD
pxui::ui::ImageStyle impl Debug @HEAD
pxui::ui::ImageStyle struct @HEAD
pxui::ui::ImageStyle::DEFAULT const ImageStyle @HEAD
pxui::ui::ImageStyle::fit fn const (self, fit: Fit) -> ImageStyle @HEAD
pxui::ui::ImageStyle::h fn const (self, size: Size) -> ImageStyle @HEAD
pxui::ui::ImageStyle::h_px fn const (self, points: f32) -> ImageStyle @HEAD
pxui::ui::ImageStyle::tint fn const (self, tint: Rgba) -> ImageStyle @U1b.2
pxui::ui::ImageStyle::w fn const (self, size: Size) -> ImageStyle @HEAD
pxui::ui::ImageStyle::w_px fn const (self, points: f32) -> ImageStyle @HEAD
pxui::ui::Key use pxui_types::ui::Key @U1a.1
pxui::ui::Keyframe use pxui_types::ui::Keyframe @U1a.1
pxui::ui::Keyframes use pxui_types::ui::Keyframes @U1a.1
pxui::ui::Language impl Clone @HEAD
pxui::ui::Language impl Copy @HEAD
pxui::ui::Language struct <'a> @HEAD
pxui::ui::Language::DEFAULT const Language<'a> @HEAD
pxui::ui::Language::brackets fn const (self, f: &'a BracketsFn<'a>) -> Language<'a> @HEAD
pxui::ui::Language::colorize fn const (self, f: &'a ColorizeFn<'a>) -> Language<'a> @HEAD
pxui::ui::Language::comment fn const (self, prefix: &'a str) -> Language<'a> @HEAD
pxui::ui::Language::complete fn const (self, f: &'a CompleteFn<'a>) -> Language<'a> @HEAD
pxui::ui::Language::context_at fn const (self, f: &'a ContextAtFn<'a>) -> Language<'a> @HEAD
pxui::ui::Language::describe fn const (self, f: &'a DescribeFn<'a>) -> Language<'a> @HEAD
pxui::ui::Language::indent fn const (self, f: &'a IndentFn<'a>) -> Language<'a> @HEAD
pxui::ui::Language::number_at fn const (self, f: &'a NumberAtFn<'a>) -> Language<'a> @HEAD
pxui::ui::Language::pairs fn const (self, pairs: &'a [(char, char)]) -> Language<'a> @HEAD
pxui::ui::Language::rewrite fn const (self, f: &'a RewriteFn<'a>) -> Language<'a> @HEAD
pxui::ui::LineStep use pxui_types::ui::LineStep @U1a.1
pxui::ui::ListResponse use pxui_types::ui::ListResponse @U1a.1
pxui::ui::Look use pxui_types::theme::Look @HEAD
pxui::ui::Mark use pxui_types::node::Mark @HEAD
pxui::ui::Menu use pxui_types::ui::Menu @HEAD
pxui::ui::MenuItem use pxui_types::ui::MenuItem @HEAD
pxui::ui::Node use pxui_types::node::Node @HEAD
pxui::ui::Overlay use pxui_types::ui::Overlay @U1a.1
pxui::ui::Paint struct <'a> @HEAD
pxui::ui::Paint::brackets fn (&mut self, rect: Rect, offset: f32, length: f32, stroke: Stroke) @HEAD
pxui::ui::Paint::chevron fn (&mut self, at: (f32, f32), direction: Dir, stroke: Stroke) @HEAD
pxui::ui::Paint::circle fn (&mut self, at: (f32, f32), radius: f32, fill: Option<Fill>, stroke: Option<Stroke>) @HEAD
pxui::ui::Paint::clip_radius fn (&self) -> f32 @HEAD
pxui::ui::Paint::color fn (&self, token: ColorToken) -> Rgba @HEAD
pxui::ui::Paint::cubic fn (&mut self, points: [(f32, f32); 4], stroke: Stroke) @HEAD
pxui::ui::Paint::curve fn (&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32), stroke: Stroke) @HEAD
pxui::ui::Paint::dashed fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @HEAD
pxui::ui::Paint::dashed_rect fn (&mut self, rect: Rect, stroke: Stroke) @HEAD
pxui::ui::Paint::density fn (&self) -> f32 @U1b.3
-pxui::ui::Paint::density fn (&self) -> i64 @U1b.3
pxui::ui::Paint::fill fn (&mut self, rect: Rect, fill: Fill) @HEAD
pxui::ui::Paint::flag fn (&mut self, at: (f32, f32), round: bool, on: bool) @HEAD
pxui::ui::Paint::frame fn (&mut self, rect: Rect, stroke: Stroke) @HEAD
pxui::ui::Paint::grid fn (&mut self, rect: Rect, origin: (f32, f32), spacing: f32, dot: f32, fill: Fill) @HEAD
pxui::ui::Paint::icon fn (&mut self, at: (f32, f32), font: u16, size: f32, ch: char, color: Rgba) -> f32 @HEAD
-pxui::ui::Paint::image fn (&mut self, rect: Rect, image: &Image, fit_mode: Fit, tint: Rgba) -> Result<(), Error> @U1b.2
pxui::ui::Paint::image fn (&mut self, rect: Rect, image: &Image, style: &ImageStyle) -> Result<(), Error> @U1b.2
pxui::ui::Paint::line fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @HEAD
pxui::ui::Paint::rtl fn (&self) -> bool @HEAD
pxui::ui::Paint::scroll_y fn (&mut self, slot: u64) -> f32 @HEAD
pxui::ui::Paint::stroke fn (&mut self, rect: Rect, stroke: Stroke) @HEAD
-pxui::ui::Paint::text fn (&mut self, at: (f32, f32), style: &Label, width: f32, s: &str) -> TextMetrics @U1b.3
pxui::ui::Paint::text fn (&mut self, at: Point, style: Label, width: f32, s: &str) -> TextMetrics @U1b.3
-pxui::ui::Paint::text_chars fn (&mut self, at: (f32, f32), style: &Label, s: &str) -> f32 @U1b.3
pxui::ui::Paint::text_chars fn (&mut self, at: Point, style: Label, s: &str) -> f32 @U1b.3
-pxui::ui::Paint::text_metrics fn (&mut self, style: &TextStyle, width: f32, s: &str) -> TextMetrics @U1b.3
pxui::ui::Paint::text_service fn (&mut self) -> UiText<'_> @U1b.3
-pxui::ui::Paint::text_spans fn (&mut self, at: (f32, f32), style: &TextStyle, width: f32, text: &str, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1b.3
pxui::ui::Paint::text_spans fn (&mut self, at: Point, style: Label, width: f32, text: &str, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1b.3
-pxui::ui::Paint::theme fn (&self) -> Tokens @U1b.4
pxui::ui::Paint::tokens fn (&self) -> Tokens @U1b.4
pxui::ui::Paint::triangle fn (&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), fill: Fill) @HEAD
pxui::ui::PanelOpts use pxui_types::ui::PanelOpts @U1a.1
pxui::ui::Pick use pxui_types::ui::Pick @HEAD
pxui::ui::PickArt use pxui_types::ui::PickArt @U2a.4
pxui::ui::PickRow use pxui_types::ui::PickRow @U2a.4
pxui::ui::PickRows use pxui_types::ui::PickRows @U1a.1
pxui::ui::PickerResponse use pxui_types::ui::PickerResponse @U1a.1
pxui::ui::Presentation use pxui_types::ui::Presentation @U2a.4
pxui::ui::ROOT_KEY use pxui_types::ui::ROOT_KEY @U1a.1
pxui::ui::Repeat use pxui_types::ui::Repeat @U1a.1
pxui::ui::Ring use pxui_types::theme::Ring @HEAD
pxui::ui::Role use pxui_types::a11y::Role @HEAD
pxui::ui::Scroll impl Clone @HEAD
pxui::ui::Scroll impl Copy @HEAD
pxui::ui::Scroll impl Debug @HEAD
pxui::ui::Scroll impl Default @HEAD
pxui::ui::Scroll impl PartialEq @HEAD
pxui::ui::Scroll impl StructuralPartialEq @HEAD
pxui::ui::Scroll struct @HEAD
pxui::ui::Scroll::painted field f32 @HEAD
pxui::ui::Scroll::settled field f32 @HEAD
-pxui::ui::Scrub impl Clone @U1b.1
-pxui::ui::Scrub impl Copy @U1b.1
-pxui::ui::Scrub impl Debug @U1b.1
-pxui::ui::Scrub struct @U1b.1
-pxui::ui::Scrub::commit field Option<f32> @U1b.1
-pxui::ui::Scrub::held field bool @U1b.1
-pxui::ui::Scrub::node field Handle @U1b.1
-pxui::ui::Scrub::preview field Option<f32> @U1b.1
pxui::ui::ScrubPhase use crate::ScrubPhase @HEAD
pxui::ui::SectionList use pxui_types::ui::SectionList @U1a.1
pxui::ui::SectionPart use pxui_types::ui::SectionPart @U1a.1
pxui::ui::Segment use crate::Segment @HEAD
pxui::ui::Segmented use pxui_types::ui::Segmented @U1a.1
pxui::ui::SelectionList use pxui_types::ui::SelectionList @U1a.1
pxui::ui::Shadow use pxui_types::theme::Shadow @HEAD
pxui::ui::Signal impl Clone @HEAD
pxui::ui::Signal impl Debug @HEAD
pxui::ui::Signal impl Default @HEAD
pxui::ui::Signal impl PartialEq @HEAD
pxui::ui::Signal impl StructuralPartialEq @HEAD
pxui::ui::Signal struct @HEAD
pxui::ui::Signal::BUTTON_PRIMARY const u8 @HEAD
pxui::ui::Signal::BUTTON_SECONDARY const u8 @HEAD
pxui::ui::Signal::a11y_select field Option<[u32; 2]> @HEAD
pxui::ui::Signal::a11y_value field Option<f64> @HEAD
pxui::ui::Signal::button field Option<u8> @HEAD
pxui::ui::Signal::clicked field bool @HEAD
pxui::ui::Signal::clicks field i64 @HEAD
pxui::ui::Signal::double_clicked field bool @HEAD
pxui::ui::Signal::drag field (f32, f32) @HEAD
pxui::ui::Signal::dragging field bool @HEAD
pxui::ui::Signal::held field bool @HEAD
pxui::ui::Signal::hovered field bool @HEAD
pxui::ui::Signal::pinch field f64 @HEAD
pxui::ui::Signal::pointer field (f32, f32) @HEAD
pxui::ui::Signal::press_point field (f32, f32) @HEAD
pxui::ui::Signal::pressed field bool @HEAD
pxui::ui::Signal::release_point field (f32, f32) @HEAD
pxui::ui::Signal::released field bool @HEAD
pxui::ui::Signal::scroll field (f32, f32) @HEAD
pxui::ui::Signal::subtree_press field Option<i64> @HEAD
pxui::ui::Size use pxui_types::Size @HEAD
pxui::ui::Slider use pxui_types::ui::Slider @U1a.1
pxui::ui::State use pxui_types::theme::State @HEAD
pxui::ui::States use pxui_types::theme::States @HEAD
pxui::ui::Stroke impl Clone @HEAD
pxui::ui::Stroke impl Copy @HEAD
pxui::ui::Stroke impl Debug @HEAD
pxui::ui::Stroke impl PartialEq @HEAD
pxui::ui::Stroke impl StructuralPartialEq @HEAD
pxui::ui::Stroke struct @HEAD
pxui::ui::Stroke::DEFAULT const Stroke @HEAD
pxui::ui::Stroke::MAX_WIDTH const f32 @HEAD
pxui::ui::Stroke::new fn const (color: Rgba) -> Stroke @HEAD
pxui::ui::Stroke::width fn const (self, width: f32) -> Stroke @HEAD
pxui::ui::SwipeActions impl Clone @HEAD
pxui::ui::SwipeActions impl Copy @HEAD
pxui::ui::SwipeActions impl Debug @HEAD
pxui::ui::SwipeActions impl Default @HEAD
pxui::ui::SwipeActions impl PartialEq @HEAD
pxui::ui::SwipeActions impl StructuralPartialEq @HEAD
pxui::ui::SwipeActions struct @HEAD
pxui::ui::SwipeActions::DEFAULT const SwipeActions @HEAD
pxui::ui::SwipeActions::full fn const (self, travel: f32) -> SwipeActions @HEAD
pxui::ui::SwipeActions::leading fn const (self, width: f32) -> SwipeActions @HEAD
pxui::ui::SwipeActions::trailing fn const (self, width: f32) -> SwipeActions @HEAD
-pxui::ui::TableResponse use pxui_types::ui::TableResponse @U2b.1
-pxui::ui::TableView use pxui_types::ui::TableView @U2b.1
pxui::ui::Target use pxui_types::ui::Target @U1a.1
pxui::ui::TextArea impl Clone @HEAD
pxui::ui::TextArea impl Copy @HEAD
pxui::ui::TextArea impl Debug @HEAD
pxui::ui::TextArea impl PartialEq @HEAD
pxui::ui::TextArea impl StructuralPartialEq @HEAD
pxui::ui::TextArea struct <'a> @HEAD
pxui::ui::TextArea::DEFAULT const TextArea<'a> @HEAD
pxui::ui::TextArea::chips fn const (self, chips: &'a [(usize, usize, Rgba)]) -> TextArea<'a> @HEAD
pxui::ui::TextArea::errors fn const (self, lines: &'a [usize]) -> TextArea<'a> @HEAD
pxui::ui::TextArea::name fn const (self, name: &'a str) -> TextArea<'a> @HEAD
pxui::ui::TextArea::notes fn const (self, notes: &'a [AreaNote<'a>]) -> TextArea<'a> @HEAD
pxui::ui::TextArea::reveal fn const (self, reveal: Reveal) -> TextArea<'a> @HEAD
pxui::ui::TextArea::select fn const (self, start: usize, end: usize) -> TextArea<'a> @HEAD
pxui::ui::TextArea::spans fn const (self, spans: &'a [(usize, usize)]) -> TextArea<'a> @HEAD
pxui::ui::TextArea::splices fn const (self, splices: &'a [Splice<'a>]) -> TextArea<'a> @HEAD
pxui::ui::TextArea::wrap fn const (self, on: bool) -> TextArea<'a> @HEAD
pxui::ui::TextEdit use pxui_types::ui::TextEdit @U1a.1
pxui::ui::Toast use pxui_types::ui::Toast @U1a.1
pxui::ui::Tokens use pxui_types::theme::Tokens @HEAD
pxui::ui::Transition use pxui_types::node::Transition @U1b.5
pxui::ui::TreeNode impl Clone @HEAD
pxui::ui::TreeNode impl Copy @HEAD
pxui::ui::TreeNode impl Debug @HEAD
pxui::ui::TreeNode impl Default @HEAD
pxui::ui::TreeNode impl Eq @HEAD
pxui::ui::TreeNode impl PartialEq @HEAD
pxui::ui::TreeNode impl StructuralPartialEq @HEAD
pxui::ui::TreeNode struct @HEAD
pxui::ui::TreeNode::children field u8 @HEAD
pxui::ui::TreeNode::depth field u16 @HEAD
pxui::ui::TreeNode::new fn const (depth: u16, has_children: bool) -> TreeNode @U1b.5
pxui::ui::TreeResponse use pxui_types::ui::TreeResponse @U1a.1
pxui::ui::Ui impl Drop @HEAD
pxui::ui::Ui struct (_) @HEAD
pxui::ui::Ui::a11y_name fn (&mut self, box_: &Handle, name: &str) -> bool @U1a.3
pxui::ui::Ui::a11y_name_of fn (&self, node: &A11yNode) -> &str @HEAD
pxui::ui::Ui::a11y_nodes fn (&self) -> &[A11yNode] @HEAD
pxui::ui::Ui::a11y_number fn (&mut self, box_: &Handle, now: f64, min: f64, max: f64, step: f64) -> Result<bool, Error> @U1a.3
-pxui::ui::Ui::a11y_tree fn (&self) -> (&[u8], u64, u32) @U1b.4
pxui::ui::Ui::a11y_tree fn (&self) -> A11yTree<'_> @U1b.4
pxui::ui::Ui::a11y_value fn (&mut self, box_: &Handle, value: &str, selection: (u32, u32)) -> Result<bool, Error> @U1a.3
pxui::ui::Ui::a11y_value_of fn (&self, node: &A11yNode) -> Option<&str> @HEAD
pxui::ui::Ui::add_surface fn (&mut self, surface: u64) -> Result<(), Error> @U5.1
-pxui::ui::Ui::anchored fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, anchor: Rect, overlay: &Overlay<'_>, f: F) -> Handle @U1b.1
pxui::ui::Ui::anchored fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, anchor: Rect, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
pxui::ui::Ui::animate fn (&mut self, b: &Handle, target: Target, curve: Curve) -> bool @HEAD
pxui::ui::Ui::animate_keys fn (&mut self, box_: &Handle, keys: &[Keyframe], play: &Keyframes) -> bool @HEAD
pxui::ui::Ui::arranged_nodes fn (&self) -> usize @HEAD
pxui::ui::Ui::background_request fn (&self) -> Option<Rgba> @HEAD
-pxui::ui::Ui::badge fn <K: Into<Key>>(&mut self, key: K, name: &str, text: &str) -> Handle @U1b.5
pxui::ui::Ui::badge fn <K: Into<Key>>(&mut self, key: K, name: &str, text: &str, badge: &Badge) -> Handle @U1b.5
pxui::ui::Ui::box_of_a11y fn (&self, id: u64) -> Option<Handle> @HEAD
-pxui::ui::Ui::box_of_key fn (&self, key: i64) -> Option<Handle> @U1b.2
pxui::ui::Ui::box_of_key fn <K: Into<Key>>(&self, key: K) -> Option<Handle> @U1b.2
pxui::ui::Ui::built_nodes fn (&self) -> usize @HEAD
pxui::ui::Ui::busy_step fn (&mut self, box_: &Handle, now: f64) -> bool @HEAD
-pxui::ui::Ui::button fn (&mut self, text: &str) -> bool @U1b.6
pxui::ui::Ui::button fn <K: Into<Key>>(&mut self, key: K, text: &str, button: &Button<'_>) -> Response @U1b.6
pxui::ui::Ui::button_content fn <K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, button: &Button<'_>, f: F) -> Response @HEAD
-pxui::ui::Ui::button_with fn <K: Into<Key>>(&mut self, key: K, text: &str, button: &Button<'_>) -> Response @U1b.6
pxui::ui::Ui::bytes fn (&self) -> Bytes @HEAD
pxui::ui::Ui::cached_label fn (&mut self, b: &Handle, index: usize, text: &str) -> &str @HEAD
pxui::ui::Ui::cancel_carry fn (&mut self) @HEAD
pxui::ui::Ui::canvas fn <F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) @HEAD
pxui::ui::Ui::canvas_over fn <F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) @HEAD
-pxui::ui::Ui::cap_width fn (&mut self, text: &str) -> f32 @U1b.3
pxui::ui::Ui::captured fn (&self) -> bool @HEAD
pxui::ui::Ui::carry fn (&mut self, from: Option<&Handle>, kind: &str, value: &str) @HEAD
pxui::ui::Ui::carrying fn (&self) -> Option<PayloadRef<'_>> @HEAD
pxui::ui::Ui::checkbox fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, checkbox: &Checkbox<'_>) -> Response @U2a.3
pxui::ui::Ui::clipboard_request fn (&self) -> Option<&str> @HEAD
pxui::ui::Ui::close_swipe fn (&mut self, node: &Handle) @HEAD
pxui::ui::Ui::color fn (&self, token: ColorToken) -> Rgba @HEAD
pxui::ui::Ui::column fn <F: FnMut(&mut Ui)>(&mut self, node: Node, f: F) -> Handle @U1b.6
-pxui::ui::Ui::column fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.6
pxui::ui::Ui::content_rect fn (&self) -> Rect @HEAD
-pxui::ui::Ui::context_at fn (&mut self, b: &Handle) -> Option<(f32, f32)> @U1b.2
pxui::ui::Ui::context_at fn (&mut self, b: &Handle) -> Option<Point> @U1b.2
-pxui::ui::Ui::create fn (platform: Platform, font_size: Option<i64>, fonts: &[&[u8]]) -> Self @U1a.3
pxui::ui::Ui::density fn (&self) -> f32 @U1b.3
-pxui::ui::Ui::density fn (&self) -> i64 @U1b.3
pxui::ui::Ui::dialog fn <K: Into<Key>>(&mut self, key: K, title: &str, buttons: &[DialogButton<'_>], dialog: &Dialog, body: &mut dyn FnMut(&mut Ui)) -> Chosen @HEAD
pxui::ui::Ui::disabled fn <F: FnMut(&mut Ui)>(&mut self, disabled: bool, f: F) @HEAD
pxui::ui::Ui::disclosure fn <K: Into<Key>>(&mut self, key: K, text: &str, open: bool, disclosure: &Disclosure, f: &mut dyn FnMut(&mut Ui)) -> Response @U2a.3
pxui::ui::Ui::dismiss_popup fn (&mut self) @HEAD
pxui::ui::Ui::draw_revision fn (&self) -> u64 @HEAD
pxui::ui::Ui::drawn fn (&mut self, index: usize) @HEAD
pxui::ui::Ui::drop_target fn (&mut self, b: &Handle) -> Option<DropRef<'_>> @HEAD
pxui::ui::Ui::dropdown fn <K: Into<Key>>(&mut self, key: K, text: &str, items: &[Segment<'_>], selected: Option<usize>, dropdown: &Dropdown) -> PickResponse @U1b.1
-pxui::ui::Ui::dropdown fn <K: Into<Key>>(&mut self, key: K, text: &str, options: &[&str], selected: usize, dropdown: &Dropdown) -> (Response, usize) @U1b.1
-pxui::ui::Ui::enable_system_fonts fn (&mut self) -> Result<(), Error> @U1b.3
pxui::ui::Ui::end_refresh fn (&mut self, node: &Handle) @HEAD
-pxui::ui::Ui::enter fn (&mut self, box_: &Handle) -> UiSaved @U1a.3
pxui::ui::Ui::environment fn (&self) -> Environment @HEAD
-pxui::ui::Ui::expanded fn (&self, key: &str) -> Option<bool> @U1b.2
pxui::ui::Ui::expanded fn <K: Into<Key>>(&self, key: K) -> Option<bool> @U1b.2
-pxui::ui::Ui::field_drag fn (&self, field: &Handle) -> Option<(f32, f32)> @U1c.3
pxui::ui::Ui::focus_box fn (&mut self, box_: &Handle) @U1a.3
pxui::ui::Ui::focus_default fn (&mut self, box_: &Handle) @U2b.4.24
-pxui::ui::Ui::focused fn (&self, box_: &Handle) -> bool @U1a.3
pxui::ui::Ui::font_size fn (&self) -> f32 @U1b.3
-pxui::ui::Ui::font_size fn (&self) -> i64 @U1b.3
-pxui::ui::Ui::footer fn (&mut self, right: Option<&str>, hints: &[(&str, &str)]) @U1c.3
pxui::ui::Ui::frame fn (&mut self, input: &FrameIn<'_>, build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> @U1a.3
pxui::ui::Ui::frame_index fn (&self) -> u64 @HEAD
pxui::ui::Ui::gesture fn (&mut self, node: &Handle, opts: &GestureOpts) -> GestureState @U1b.2
-pxui::ui::Ui::gesture fn (&mut self, node: &Handle, opts: GestureOpts) -> GestureState @U1b.2
pxui::ui::Ui::gesture_state fn (&mut self, node: &Handle) -> GestureState @HEAD
-pxui::ui::Ui::grid_list fn <K: Into<Key>>(&mut self, key: K, count: usize, cell_w: f32, cell_h: f32, build: &mut dyn FnMut(&mut Ui, usize)) -> Handle @U1b.5
pxui::ui::Ui::grid_list fn <K: Into<Key>>(&mut self, key: K, count: usize, grid: &GridList, build: &mut dyn FnMut(&mut Ui, usize)) -> Handle @U1b.5
-pxui::ui::Ui::grip fn (&mut self, label: Arguments<'_>, axis: Axis, rect: Option<Rect>) -> (Handle, Signal) @U1c.3
-pxui::ui::Ui::hover_delay fn (&mut self, key: &str) -> bool @U1b.2
pxui::ui::Ui::hover_delay fn <K: Into<Key>>(&mut self, key: K) -> bool @U1b.2
pxui::ui::Ui::hovered fn (&self, b: &Handle, within: bool) -> bool @U1b.2
-pxui::ui::Ui::hovered_exactly fn (&self, b: &Handle) -> bool @U1b.2
-pxui::ui::Ui::hovered_within fn (&mut self, b: &Handle) -> bool @U1b.2
pxui::ui::Ui::icon_button fn <K: Into<Key>>(&mut self, key: K, icon: Icon, name: &str, button: &IconButton<'_>) -> Response @HEAD
-pxui::ui::Ui::id fn (&self) -> usize @U1b.6
pxui::ui::Ui::id fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, f: F) @U1b.6
-pxui::ui::Ui::image fn (&mut self, name: &str, image: &Image, style: &ImageStyle) -> Result<Handle, Error> @U1b.2
pxui::ui::Ui::image fn <K: Into<Key>>(&mut self, key: K, name: &str, image: &Image, style: &ImageStyle) -> Result<Handle, Error> @U1b.2
-pxui::ui::Ui::ime fn (&self) -> Option<((i64, i64, i64, i64), i64)> @U1b.2
pxui::ui::Ui::ime fn (&self) -> Option<Ime> @U1b.2
pxui::ui::Ui::inject_a11y fn (&mut self, actions: &[A11yAction]) @HEAD
-pxui::ui::Ui::input_frame fn (&mut self, input: &Input, build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> @U1a.3
-pxui::ui::Ui::inspector_bar fn (&mut self, hints: &[(&str, &str)], count: i64) @U1c.3
-pxui::ui::Ui::inspector_body fn (&mut self, f: &mut dyn FnMut(&mut Ui)) @U1c.3
-pxui::ui::Ui::inspector_button fn (&mut self, key: &str, label: &str) -> bool @U1c.3
-pxui::ui::Ui::inspector_header fn (&mut self, key: &str, kind: Option<&str>, badge: Option<&str>, index: Option<&str>, rename: Option<Rc<dyn Fn(&str) -> bool>>, actions: &[HeadAction], reset: Option<&str>, reset_enabled: bool, title: &str, detail: &str) -> Head @U1c.3
-pxui::ui::Ui::inspector_label_x fn (&self) -> f32 @U1c.3
-pxui::ui::Ui::inspector_message fn (&mut self, key: &str, message: &str) @U1c.3
-pxui::ui::Ui::inspector_readout fn (&mut self, width: Option<f32>, key: &str, label: &str, value: &str) @U1c.3
-pxui::ui::Ui::inspector_row fn (&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (Handle, f32, f32, f32) @U1c.3
-pxui::ui::Ui::inspector_section fn (&mut self, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, build: &mut dyn FnMut(&mut Ui)) -> bool @U1c.3
-pxui::ui::Ui::inspector_toggle fn (&mut self, key: &str, label: &str, value: bool) -> bool @U1c.3
-pxui::ui::Ui::inspector_width fn (&self) -> f32 @U1c.3
pxui::ui::Ui::instance_id fn (&self) -> usize @U1b.6
pxui::ui::Ui::item_text fn (&mut self, text: &str) @HEAD
pxui::ui::Ui::key_down fn (&self, key: KeyCode) -> bool @U1a.4
pxui::ui::Ui::key_pressed fn (&self, key: KeyCode, mods: Mods) -> bool @U1a.4
pxui::ui::Ui::keyboard fn (&self) -> Keyboard @HEAD
pxui::ui::Ui::label fn (&mut self, text: &str, style: Label) -> Handle @HEAD
pxui::ui::Ui::label_at fn (&mut self, key: Key, rect: Rect, text: &str, style: Label) -> Result<Handle, Error> @HEAD
pxui::ui::Ui::label_digits fn (&mut self, args: Arguments<'_>, style: Label) -> Handle @HEAD
pxui::ui::Ui::label_fmt fn (&mut self, args: Arguments<'_>, style: Label) -> Handle @HEAD
pxui::ui::Ui::last_error fn (&self) -> &str @HEAD
pxui::ui::Ui::last_press_within fn (&self, events: &[Event], roots: &[Key]) -> Option<Key> @U1b.2
-pxui::ui::Ui::last_press_within fn (&self, events: &[Event], roots: &[i64]) -> Option<i64> @U1b.2
pxui::ui::Ui::layer fn (&self, index: usize) -> Option<Layer<'_>> @U1a.3
pxui::ui::Ui::layer_count fn (&self) -> usize @HEAD
pxui::ui::Ui::layers fn (&self) -> Layers<'_> @U1a.3
-pxui::ui::Ui::leave fn (&mut self, saved: UiSaved) @U1a.3
pxui::ui::Ui::library_calls fn (&self) -> u64 @HEAD
-pxui::ui::Ui::line_field fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'_> @U1c.3
pxui::ui::Ui::list fn <K: Into<Key>, F: FnMut(&mut Ui, usize)>(&mut self, key: K, count: usize, row_h: f32, row_key: &dyn Fn(usize) -> i64, f: F) -> ListResponse @U2b.2
-pxui::ui::Ui::load_text_edit fn (&mut self, key: i64, text: &str) -> TextEdit @U1b.2
pxui::ui::Ui::load_text_edit fn <K: Into<Key>>(&mut self, key: K, text: &str) -> TextEdit @U1b.2
pxui::ui::Ui::local_rect fn (&self, b: &Handle) -> Rect @HEAD
-pxui::ui::Ui::menu fn <K: Into<Key>>(&mut self, key: K, at: (f32, f32), items: &[MenuItem<'_>], menu: &Menu<'_>) -> Chosen @U1b.3
pxui::ui::Ui::menu fn <K: Into<Key>>(&mut self, key: K, at: Point, items: &[MenuItem<'_>], menu: &Menu<'_>) -> Chosen @U1b.3
-pxui::ui::Ui::menu_bar fn (&mut self, label: &str, menus: &[(&str, &[&str])]) -> Option<(usize, usize)> @U1b.1
pxui::ui::Ui::menu_bar fn <K: Into<Key>>(&mut self, key: K, menus: &[MenuBarMenu<'_>], bar: &MenuBar) -> Chosen @U1b.1
pxui::ui::Ui::modal fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
-pxui::ui::Ui::modal fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> bool @U1b.1
pxui::ui::Ui::new fn (desc: &UiDesc<'_>) -> Result<Self, Error> @U1a.3
-pxui::ui::Ui::node fn (&mut self, key: Key, node: Node) -> Handle @U1b.2
pxui::ui::Ui::node fn <K: Into<Key>>(&mut self, key: K, node: Node) -> Handle @U1b.2
pxui::ui::Ui::node_layout fn (&self, b: &Handle) -> Node @HEAD
pxui::ui::Ui::node_mut fn (&mut self, b: &Handle, edit: &mut dyn FnMut(&mut NodeFields)) @U1c.1
pxui::ui::Ui::now fn (&self) -> f64 @HEAD
pxui::ui::Ui::output fn (&self) -> Output @U1a.3
-pxui::ui::Ui::panel fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) @U1b.1
pxui::ui::Ui::panel fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
pxui::ui::Ui::part fn (&mut self, part: Part, node: Node) -> Handle @HEAD
pxui::ui::Ui::part_box fn (&self, owner: &Handle, part: Part) -> Option<Handle> @U1b.2
-pxui::ui::Ui::part_box fn (&self, owner: i64, part: Part) -> Option<Handle> @U1b.2
pxui::ui::Ui::part_rect fn (&self, owner: &Handle, part: Part) -> Option<Rect> @U1b.2
-pxui::ui::Ui::part_rect fn (&self, owner: i64, part: Part) -> Option<Rect> @U1b.2
pxui::ui::Ui::passed_undo fn (&self) -> Option<UndoKind> @HEAD
pxui::ui::Ui::paste_requested fn (&self) -> bool @HEAD
pxui::ui::Ui::patch_field fn (&mut self, field: &Handle, o: &ValueField<'_>, value_text: &str) -> bool @HEAD
pxui::ui::Ui::patch_look fn (&mut self, b: &Handle, value: &Look) -> bool @HEAD
pxui::ui::Ui::patch_mark fn (&mut self, b: &Handle, value: Mark) -> bool @HEAD
pxui::ui::Ui::patch_rect fn (&mut self, b: &Handle, value: Rect) -> bool @HEAD
pxui::ui::Ui::patch_text fn (&mut self, box_: &Handle, value: &str) -> bool @HEAD
pxui::ui::Ui::phase_cpu_times fn (&self) -> Result<[Duration; 7], Error> @HEAD
pxui::ui::Ui::phase_times fn (&self) -> [Duration; 7] @HEAD
pxui::ui::Ui::picker fn <K: Into<Key>>(&mut self, key: K, placeholder: &str, query: &str, picker: &Picker<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> @U1b.1
-pxui::ui::Ui::picker fn <K: Into<Key>>(&mut self, key: K, placeholder: &str, query: &str, picker: &PickerOpts<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> @U1b.1
pxui::ui::Ui::platform fn (&self) -> Platform @HEAD
pxui::ui::Ui::popup fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> OverlayResponse @U1b.1
-pxui::ui::Ui::popup fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> bool @U1b.1
pxui::ui::Ui::press_keys fn (&self, b: &Handle) -> &[u32] @HEAD
pxui::ui::Ui::pressable fn <K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, node: Node, pressable: &Pressable<'_>, f: F) -> Response @HEAD
pxui::ui::Ui::prewarm_text fn (&mut self, text: &str, style: &Label, max_width: f32) -> Result<(), Error> @HEAD
-pxui::ui::Ui::progress fn <K: Into<Key>>(&mut self, key: K, text: &str, value: Option<f32>) -> Handle @U1b.5
pxui::ui::Ui::progress fn <K: Into<Key>>(&mut self, key: K, text: &str, value: Option<f32>, progress: &Progress) -> Handle @U1b.5
pxui::ui::Ui::radio fn <K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, radio: &Radio<'_>) -> PickResponse @U1b.1
-pxui::ui::Ui::radio fn <K: Into<Key>>(&mut self, key: K, options: &[&str], selected: usize, choice: &Choice<'_>) -> (Response, usize) @U1b.1
pxui::ui::Ui::rebuilt_named fn (&self, prefix: &str) -> usize @HEAD
pxui::ui::Ui::rebuilt_views fn (&self) -> usize @HEAD
pxui::ui::Ui::rect fn (&self, box_: &Handle) -> Rect @HEAD
pxui::ui::Ui::register_badge_body fn (&mut self, name: &str, body: fn(&mut Ui, &BadgeConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_button_body fn (&mut self, name: &str, body: fn(&mut Ui, &ButtonConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_checkbox_body fn (&mut self, name: &str, body: fn(&mut Ui, &CheckboxConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_dialog_body fn (&mut self, name: &str, body: fn(&mut Ui, &DialogConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_dropdown_body fn (&mut self, name: &str, body: fn(&mut Ui, &DropdownConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_footer_body fn (&mut self, name: &str, body: fn(&mut Ui, &FooterConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_grip_body fn (&mut self, name: &str, body: fn(&mut Ui, &GripConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_icon_button_body fn (&mut self, name: &str, body: fn(&mut Ui, &IconButtonConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_inspector_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &InspectorRowConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_inspector_section_body fn (&mut self, name: &str, body: fn(&mut Ui, &InspectorSectionConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_label_body fn (&mut self, name: &str, body: fn(&mut Ui, &LabelConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_line_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &LineFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_menu_bar_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuBarConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_menu_item_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuItemConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_menu_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuRowConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_modal_body fn (&mut self, name: &str, body: fn(&mut Ui, &ModalConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_panel_body fn (&mut self, name: &str, body: fn(&mut Ui, &PanelConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_picker_body fn (&mut self, name: &str, body: fn(&mut Ui, &PickerConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_picker_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &PickerFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_popup_body fn (&mut self, name: &str, body: fn(&mut Ui, &PopupConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_progress_body fn (&mut self, name: &str, body: fn(&mut Ui, &ProgressConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_radio_body fn (&mut self, name: &str, body: fn(&mut Ui, &RadioConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_scroll_view_body fn (&mut self, name: &str, body: fn(&mut Ui, &ScrollViewConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_search_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &SearchFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_section_index_body fn (&mut self, name: &str, body: fn(&mut Ui, &SectionIndexConfig)) -> Result<BodyId, Error> @U2a.3
pxui::ui::Ui::register_segmented_body fn (&mut self, name: &str, body: fn(&mut Ui, &SegmentedConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_selection_list_body fn (&mut self, name: &str, body: fn(&mut Ui, &SelectionListConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_slider_body fn (&mut self, name: &str, body: fn(&mut Ui, &SliderConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_spinner_body fn (&mut self, name: &str, body: fn(&mut Ui, &SpinnerConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_split_body fn (&mut self, name: &str, body: fn(&mut Ui, &SplitConfig)) -> Result<BodyId, Error> @U1a.3
-pxui::ui::Ui::register_table_cell_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableCellConfig)) -> Result<BodyId, Error> @U2b.1
pxui::ui::Ui::register_table_head_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableHeadConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_table_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableRowConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_tabs_body fn (&mut self, name: &str, body: fn(&mut Ui, &TabsConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_text_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &TextFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_toasts_body fn (&mut self, name: &str, body: fn(&mut Ui, &ToastsConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_toggle_body fn (&mut self, name: &str, body: fn(&mut Ui, &ToggleConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_tooltip_body fn (&mut self, name: &str, body: fn(&mut Ui, &TooltipConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_tree_body fn (&mut self, name: &str, body: fn(&mut Ui, &TreeConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_value_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &ValueFieldConfig)) -> Result<BodyId, Error> @U1a.3
pxui::ui::Ui::register_window_body fn (&mut self, name: &str, body: fn(&mut Ui, &WindowConfig)) -> Result<BodyId, Error> @U1a.3
-pxui::ui::Ui::remember fn (&mut self, edit: &TextEdit, typing: bool) @U1b.2
pxui::ui::Ui::remember fn <K: Into<Key>>(&mut self, key: K, edit: &TextEdit, typing: bool) @U1b.2
pxui::ui::Ui::remove_surface fn (&mut self, surface: u64) -> Result<(), Error> @U5.1
pxui::ui::Ui::reordering fn (&self) -> bool @U2b.4.12
pxui::ui::Ui::repaint_after fn (&mut self, seconds: f64) @HEAD
pxui::ui::Ui::repaint_at fn (&self) -> Option<f64> @HEAD
pxui::ui::Ui::request_cursor fn (&mut self, shape: Cursor) @HEAD
pxui::ui::Ui::request_frame fn (&mut self) @HEAD
pxui::ui::Ui::step fn (&mut self, time: f64) -> bool @HEAD
pxui::ui::Ui::resolve fn (&self, style: &Label) -> TextStyle @HEAD
pxui::ui::Ui::retain_palette_labels fn (&mut self) @HEAD
pxui::ui::Ui::retain_palette_paint fn (&mut self) @HEAD
pxui::ui::Ui::retain_view_on_resize fn (&mut self) @HEAD
pxui::ui::Ui::reveal fn (&mut self, list: &Handle, index: usize) @HEAD
-pxui::ui::Ui::root_at fn (&self, point: (f32, f32), roots: &[i64]) -> Option<i64> @U1b.2
pxui::ui::Ui::root_at fn (&self, point: Point, roots: &[Key]) -> Option<Key> @U1b.2
-pxui::ui::Ui::routed_input fn (&mut self, owner: Option<i64>) -> Routed<'_> @U1a.3
pxui::ui::Ui::row fn <F: FnMut(&mut Ui)>(&mut self, node: Node, f: F) -> Handle @U1b.6
-pxui::ui::Ui::row fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.6
pxui::ui::Ui::row_height fn (&self) -> f32 @U1b.3
-pxui::ui::Ui::row_height fn (&self) -> i64 @U1b.3
pxui::ui::Ui::scroll fn (&self, b: &Handle) -> Scroll @HEAD
pxui::ui::Ui::scroll_view fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.5
-pxui::ui::Ui::scroll_view fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, w: Size, h: Size, f: F) -> Handle @U1b.5
-pxui::ui::Ui::scrub fn <K: Into<Key>>(&mut self, key: K, value: f32, range: RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> Scrub @U1b.1
pxui::ui::Ui::scrub fn <K: Into<Key>>(&mut self, key: K, value: f32, range: RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> SliderResponse @U1b.1
pxui::ui::Ui::search_field fn <K: Into<Key>>(&mut self, key: K, value: &str, field: &SearchField<'_>) -> SearchFieldResponse<'_> @HEAD
pxui::ui::Ui::section_index fn <K: Into<Key>>(&mut self, key: K, letters: &[&str], index: &SectionIndex<'_>) -> PickResponse @U2a.3
-pxui::ui::Ui::section_list fn <K: Into<Key>>(&mut self, key: K, section: &SectionList, row_key: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, f32)) -> ListResponse @U2b.4.10
pxui::ui::Ui::section_list fn <K: Into<Key>>(&mut self, key: K, section: &SectionList, row_key: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse @U2b.4.10
pxui::ui::Ui::segmented fn <K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, segmented: &Segmented<'_>) -> PickResponse @U1b.1
-pxui::ui::Ui::segmented_with fn <K: Into<Key>>(&mut self, key: K, options: &[Segment<'_>], selected: Option<usize>, segmented: &Segmented<'_>) -> SegmentedResponse @U1b.1
-pxui::ui::Ui::selection_list fn <K: Into<Key>>(&mut self, key: K, count: usize, row_key: &dyn Fn(usize) -> i64, selected: &mut [u8], list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, usize)) -> ListResponse @U2b.1
pxui::ui::Ui::selection_list fn <K: Into<Key>>(&mut self, key: K, count: usize, row_key: &dyn Fn(usize) -> i64, state: &mut ListState, list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, SectionPart, CellState)) -> ListResponse @U2b.1
pxui::ui::Ui::set_a11y fn (&mut self, on: bool) @HEAD
pxui::ui::Ui::set_at fn (&mut self, b: &Handle, at: (f32, f32)) @HEAD
-pxui::ui::Ui::set_colors fn (&mut self, colors: &[(Rgba, Rgba)]) @U1a.2
pxui::ui::Ui::set_dir fn (&mut self, rtl: bool) @HEAD
pxui::ui::Ui::set_font_size fn (&mut self, size: f32) @U1b.3
-pxui::ui::Ui::set_font_size fn (&mut self, size: i64) @U1b.3
-pxui::ui::Ui::set_look fn (&mut self, box_: &Handle, look: Look, state: State) @U1b.4
pxui::ui::Ui::set_node_layout fn (&mut self, b: &Handle, node: &Node) @HEAD
pxui::ui::Ui::set_role fn (&mut self, box_: &Handle, role: Role, states: A11yStates) @U1b.4
-pxui::ui::Ui::set_role fn (&mut self, box_: &Handle, role: Role, states: u32) @U1b.4
pxui::ui::Ui::set_scroll_offset fn (&mut self, b: &Handle, value: f32) @HEAD
pxui::ui::Ui::set_state fn (&mut self, box_: &Handle, value: i64) @HEAD
pxui::ui::Ui::set_text_state fn (&mut self, b: &Handle, value: Option<&str>) @HEAD
pxui::ui::Ui::set_text_worker fn (&mut self, worker: Option<&TextWorker>) -> Result<(), Error> @HEAD
pxui::ui::Ui::set_theme fn (&mut self, theme: &ThemeHandle) -> Result<(), Error> @U1a.2
-pxui::ui::Ui::set_theme_source fn (&mut self, source: &ThemeSource) -> Result<(), Error> @U1a.2
pxui::ui::Ui::set_timing fn (&mut self, on: bool) @HEAD
pxui::ui::Ui::set_view fn (&mut self, b: &Handle, view: &ViewTransform) @HEAD
pxui::ui::Ui::signal fn (&self, box_: &Handle) -> Signal @HEAD
pxui::ui::Ui::signal_keys fn (&self, box_: &Handle) -> &[Event] @HEAD
-pxui::ui::Ui::slider fn <K: Into<Key>>(&mut self, key: K, text: &str, value: f32, range: RangeInclusive<f32>, slider: &Slider) -> (Response, f32) @U1b.1
pxui::ui::Ui::slider fn <K: Into<Key>>(&mut self, key: K, text: &str, value: f32, range: RangeInclusive<f32>, slider: &Slider) -> SliderResponse @U1b.1
pxui::ui::Ui::slot fn (&mut self, slot: PxuiSlot) -> bool @HEAD
-pxui::ui::Ui::spinner fn <K: Into<Key>>(&mut self, key: K, name: &str) -> Handle @U1b.5
pxui::ui::Ui::spinner fn <K: Into<Key>>(&mut self, key: K, name: &str, spinner: &Spinner) -> Handle @U1b.5
-pxui::ui::Ui::split fn (&mut self, label: &str, axis: Axis, ratio: f32, min_a: f32, min_b: f32, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> Option<f32> @U1b.1
pxui::ui::Ui::split fn <K: Into<Key>>(&mut self, key: K, ratio: f32, split: &Split, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> SplitResponse @U1b.1
pxui::ui::Ui::stack fn <F: FnMut(&mut Ui)>(&mut self, node: Node, f: F) -> Handle @U1b.6
-pxui::ui::Ui::stack fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1b.6
pxui::ui::Ui::state fn (&self, box_: &Handle, default: i64) -> i64 @HEAD
pxui::ui::Ui::style fn (&mut self, box_: &Handle, states: &States, state: State) @HEAD
pxui::ui::Ui::system_appearance fn (&self) -> Appearance @U1a.4
pxui::ui::Ui::table fn <K: Into<Key>>(&mut self, key: K, columns: &[Column<'_>], rows: usize, row_key: &dyn Fn(usize) -> i64, state: &mut TableState, table: &Table, cell: &mut dyn FnMut(&mut Ui, usize, usize, CellState)) -> TableResponse @U2b.1
pxui::ui::Ui::table_head fn <K: Into<Key>>(&mut self, key: K, columns: &[Column<'_>], state: &mut TableState, head: &TableHead) -> HeadResponse @U2b.1
-pxui::ui::Ui::table_view fn <K: Into<Key>>(&mut self, key: K, titles: &[&str], widths: &mut [f32], rows: usize, row_key: &dyn Fn(usize) -> i64, selected: &mut Option<usize>, table: &TableView, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> TableResponse @U2b.1
-pxui::ui::Ui::table_view fn <K: Into<Key>>(&mut self, key: K, titles: &[&str], widths: &mut [f32], rows: usize, selected: &mut Option<usize>, table: &TableView, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> TableResponse @U2b.1
pxui::ui::Ui::tabs fn <K: Into<Key>>(&mut self, key: K, items: &[Segment<'_>], selected: Option<usize>, tabs: &Tabs<'_>) -> PickResponse @U2a.2
pxui::ui::Ui::text fn (&mut self, box_: &Handle, value: &str) @HEAD
pxui::ui::Ui::text_area fn <K: Into<Key>>(&mut self, key: K, value: &str, node: Node, area: &TextArea<'_>, callbacks: &AreaCallbacks<'_>) -> TextAreaResponse<'_> @U1b.5
-pxui::ui::Ui::text_area fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, area: &TextArea<'_>, callbacks: &AreaCallbacks<'_>) -> TextAreaResponse<'_> @U1b.5
pxui::ui::Ui::text_field fn <K: Into<Key>>(&mut self, key: K, value: &str, field: &TextField<'_>) -> TextFieldResponse<'_> @HEAD
pxui::ui::Ui::text_input_focused fn (&self) -> bool @HEAD
-pxui::ui::Ui::text_input_options fn (&self) -> TextTraits @U1b.2
pxui::ui::Ui::text_layout_count fn (&self) -> u64 @HEAD
-pxui::ui::Ui::text_line_box fn (&mut self, box_: &Handle, text: &str, style: &Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::ui::Ui::text_line_box fn (&mut self, box_: &Handle, text: &str, style: Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::ui::Ui::text_service fn (&mut self) -> UiText<'_> @HEAD
-pxui::ui::Ui::text_spans fn (&mut self, box_: &Handle, text: &str, style: &Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::ui::Ui::text_spans fn (&mut self, box_: &Handle, text: &str, style: Label, spans: &[TextSpan]) -> Result<(), Error> @U1b.3
pxui::ui::Ui::text_state fn (&self, b: &Handle) -> Option<&str> @HEAD
-pxui::ui::Ui::text_top fn (&self, size: Option<i64>, y: f32, h: f32) -> f32 @U1b.3
pxui::ui::Ui::text_traits fn (&self) -> TextTraits @U1b.2
pxui::ui::Ui::text_undo_bytes fn (&self) -> usize @HEAD
-pxui::ui::Ui::text_width fn (&mut self, size: Option<i64>, text: &str) -> f32 @U1b.3
pxui::ui::Ui::theme fn (&self) -> &Theme @HEAD
pxui::ui::Ui::themed fn (&mut self, theme: &ThemeHandle, f: &mut dyn FnMut(&mut Ui)) @U1a.2
pxui::ui::Ui::to_front fn (&mut self, order: i64, b: &Handle) @HEAD
pxui::ui::Ui::toasts fn <K: Into<Key>>(&mut self, key: K, items: &[Toast<'_>], seconds: f64) -> Chosen @U1b.1
-pxui::ui::Ui::toasts fn <K: Into<Key>>(&mut self, key: K, items: &[Toast<'_>], seconds: f64) -> Option<usize> @U1b.1
-pxui::ui::Ui::toggle fn (&mut self, text: &str, on: bool) -> bool @U1b.6
pxui::ui::Ui::toggle fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, toggle: &Toggle<'_>) -> Response @U1b.6
-pxui::ui::Ui::toggle_with fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, toggle: &Toggle<'_>) -> Response @U1b.6
pxui::ui::Ui::tokens fn (&self) -> Tokens @HEAD
pxui::ui::Ui::tooltip fn (&mut self, box_: &Handle, text: &str) @HEAD
pxui::ui::Ui::transfer_carry fn (&mut self, to: &Handle) -> bool @HEAD
-pxui::ui::Ui::tree fn <K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse @HEAD
-pxui::ui::Ui::tree fn <K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], revision: Option<u64>, cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse @U2b.4.28
pxui::ui::Ui::tree fn <K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], state: &mut TreeState, cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse @U2b.4.28
pxui::ui::Ui::unfocus fn (&mut self) @HEAD
-pxui::ui::Ui::use_theme fn (&mut self, theme: &Theme) @U1a.2
-pxui::ui::Ui::value_field fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &ValueField<'_>, callbacks: &ValueCallbacks<'_>) -> ValueFieldResponse<'_> @U1c.3
-pxui::ui::Ui::verify_new_views fn (on: bool) @U1c.2
pxui::ui::Ui::verify_views fn (&mut self, on: bool) @HEAD
pxui::ui::Ui::verifying fn (&self) -> bool @HEAD
-pxui::ui::Ui::view_backdrop_blur fn (&mut self, b: &Handle, sigma: f32) @U1b.4
-pxui::ui::Ui::view_backdrop_saturation fn (&mut self, b: &Handle, saturation: f32) @U1b.4
-pxui::ui::Ui::view_clip fn (&mut self, b: &Handle, clip: Option<Rect>) @U1b.4
pxui::ui::Ui::view_of fn <K: Into<Key>, T: Hash + ?Sized, F: FnMut(&mut Ui)>(&mut self, key: K, inputs: &T, f: F) -> Handle @HEAD
-pxui::ui::Ui::view_size fn (&self) -> (f32, f32) @U1b.3
pxui::ui::Ui::view_size fn (&self) -> ViewSize @U1b.3
pxui::ui::Ui::wants_frame fn (&self, events: &[Event], now: f64) -> bool @HEAD
pxui::ui::Ui::within fn (&mut self, box_: &Handle, f: &mut dyn FnMut(&mut Ui)) @HEAD
pxui::ui::UndoKind use pxui_types::ui::UndoKind @U1a.1
pxui::ui::ValueField impl Clone @HEAD
pxui::ui::ValueField impl Copy @HEAD
pxui::ui::ValueField impl Debug @HEAD
pxui::ui::ValueField impl PartialEq @HEAD
pxui::ui::ValueField impl StructuralPartialEq @HEAD
pxui::ui::ValueField struct <'a> @HEAD
pxui::ui::ValueField::DEFAULT const ValueField<'a> @HEAD
pxui::ui::ValueField::bare fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ui::ValueField::display fn const (self, text: &'a str) -> ValueField<'a> @HEAD
pxui::ui::ValueField::edit fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ui::ValueField::fraction fn const (self, fraction: f32) -> ValueField<'a> @HEAD
pxui::ui::ValueField::kept_drag fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ui::ValueField::lead fn const (self, text: &'a str, color: Rgba) -> ValueField<'a> @HEAD
pxui::ui::ValueField::left fn const (self, on: bool) -> ValueField<'a> @HEAD
pxui::ui::ValueField::line fn const (self, color: Rgba) -> ValueField<'a> @HEAD
pxui::ui::ValueField::name fn const (self, name: &'a str) -> ValueField<'a> @HEAD
pxui::ui::ValueField::placeholder fn const (self, text: &'a str) -> ValueField<'a> @HEAD
pxui::ui::ValueField::size fn const (self, points: usize) -> ValueField<'a> @HEAD
pxui::ui::ValueField::tracking fn const (self, tracking: f32) -> ValueField<'a> @HEAD
pxui::ui::ValueField::trail fn const (self, text: &'a str, color: Rgba) -> ValueField<'a> @HEAD
pxui::ui::ViewTransform impl Clone @HEAD
pxui::ui::ViewTransform impl Copy @HEAD
pxui::ui::ViewTransform impl Debug @HEAD
pxui::ui::ViewTransform impl Default @HEAD
pxui::ui::ViewTransform impl PartialEq @HEAD
pxui::ui::ViewTransform impl StructuralPartialEq @HEAD
pxui::ui::ViewTransform struct @HEAD
pxui::ui::ViewTransform::DEFAULT const ViewTransform @HEAD
pxui::ui::ViewTransform::about fn const (self, scale: f32, matrix: [f32; 4], pivot: (f32, f32)) -> ViewTransform @HEAD
pxui::ui::ViewTransform::blur fn const (self, blur: f32) -> ViewTransform @U1b.4
pxui::ui::ViewTransform::clip fn const (self, clip: Option<Rect>) -> ViewTransform @U1b.4
pxui::ui::ViewTransform::offset fn const (self, dx: f32, dy: f32) -> ViewTransform @HEAD
pxui::ui::ViewTransform::opacity fn const (self, opacity: f32) -> ViewTransform @HEAD
pxui::ui::ViewTransform::saturation fn const (self, saturation: f32) -> ViewTransform @U1b.4
pxui::ui::ViewTransform::scale fn const (self, scale: f32) -> ViewTransform @HEAD
pxui::ui::context_clicked fn (signal: &Signal) -> bool @HEAD
pxui::ui::fuzzy_match use pxui_types::text_edit::fuzzy_match @HEAD
pxui::ui::fuzzy_score use pxui_types::text_edit::fuzzy_score @HEAD
pxui::ui::int_key use pxui_types::ui::int_key @U1a.1
-pxui::ui::key fn (b: &Handle) -> i64 @U1a.1
pxui::ui::key_of use pxui_types::ui::key_of @U1a.1
pxui::ui::line_end use pxui_types::text_edit::line_end @HEAD
pxui::ui::line_start use pxui_types::text_edit::line_start @HEAD
pxui::ui::scrubbed use pxui_types::text_edit::scrubbed @HEAD
-pxui::ui::state use pxui_types::a11y::state @U1b.4
-pxui::ui_core mod @U1a.1
-pxui::ui_core::* use pxui_types::ui::* @U1a.1
-pxui::ui_core::Axis use pxui_types::node::Axis @U1a.1
-pxui::ui_core::Bar use pxui_types::node::Bar @U1a.1
-pxui::ui_core::Dir use pxui_types::node::Dir @U1a.1
-pxui::ui_core::DismissOpts impl Clone @U1a.1
-pxui::ui_core::DismissOpts impl Copy @U1a.1
-pxui::ui_core::DismissOpts impl Debug @U1a.1
-pxui::ui_core::DismissOpts impl Default @U1a.1
-pxui::ui_core::DismissOpts impl PartialEq @U1a.1
-pxui::ui_core::DismissOpts impl StructuralPartialEq @U1a.1
-pxui::ui_core::DismissOpts struct @U1a.1
-pxui::ui_core::DismissOpts::DEFAULT const DismissOpts @U1a.1
-pxui::ui_core::DismissOpts::fraction fn const (self, fraction: f32) -> DismissOpts @U1a.1
-pxui::ui_core::DismissOpts::velocity fn const (self, velocity: f32) -> DismissOpts @U1a.1
-pxui::ui_core::Flags use pxui_types::Flags @U1a.1
-pxui::ui_core::GestureOpts impl Clone @U1a.1
-pxui::ui_core::GestureOpts impl Copy @U1a.1
-pxui::ui_core::GestureOpts impl Debug @U1a.1
-pxui::ui_core::GestureOpts impl Default @U1a.1
-pxui::ui_core::GestureOpts impl PartialEq @U1a.1
-pxui::ui_core::GestureOpts impl StructuralPartialEq @U1a.1
-pxui::ui_core::GestureOpts struct @U1a.1
-pxui::ui_core::GestureOpts::DEFAULT const GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::dismiss fn const (self, dismiss: DismissOpts) -> GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::edge_back fn const (self) -> GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::long_press fn const (self) -> GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::pull_to_refresh fn const (self) -> GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::refresh fn const (self, points: f32) -> GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::row_swipe fn const (self, width: f32) -> GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::swipe_actions fn const (self, actions: SwipeActions) -> GestureOpts @U1a.1
-pxui::ui_core::GestureOpts::touch_drag fn const (self) -> GestureOpts @U1a.1
-pxui::ui_core::GestureState impl Clone @U1a.1
-pxui::ui_core::GestureState impl Copy @U1a.1
-pxui::ui_core::GestureState impl Debug @U1a.1
-pxui::ui_core::GestureState impl Default @U1a.1
-pxui::ui_core::GestureState impl PartialEq @U1a.1
-pxui::ui_core::GestureState impl StructuralPartialEq @U1a.1
-pxui::ui_core::GestureState struct @U1a.1
-pxui::ui_core::GestureState::back field f32 @U1a.1
-pxui::ui_core::GestureState::back_active field bool @U1a.1
-pxui::ui_core::GestureState::back_cancelled field bool @U1a.1
-pxui::ui_core::GestureState::back_committed field bool @U1a.1
-pxui::ui_core::GestureState::dismiss field f32 @U1a.1
-pxui::ui_core::GestureState::dismiss_cancelled field bool @U1a.1
-pxui::ui_core::GestureState::dismiss_committed field bool @U1a.1
-pxui::ui_core::GestureState::dismissing field bool @U1a.1
-pxui::ui_core::GestureState::highlighted field bool @U1a.1
-pxui::ui_core::GestureState::long_pressed field bool @U1a.1
-pxui::ui_core::GestureState::pull field f32 @U1a.1
-pxui::ui_core::GestureState::refresh_fired field bool @U1a.1
-pxui::ui_core::GestureState::refreshing field bool @U1a.1
-pxui::ui_core::GestureState::swipe field f32 @U1a.1
-pxui::ui_core::GestureState::swipe_commit_leading field bool @U1a.1
-pxui::ui_core::GestureState::swipe_commit_trailing field bool @U1a.1
-pxui::ui_core::GestureState::swipe_open field bool @U1a.1
-pxui::ui_core::GestureState::swipe_open_leading field bool @U1a.1
-pxui::ui_core::GestureState::swipe_open_trailing field bool @U1a.1
-pxui::ui_core::GestureState::swiping field bool @U1a.1
-pxui::ui_core::Mark use pxui_types::node::Mark @U1a.1
-pxui::ui_core::Node use pxui_types::node::Node @U1a.1
-pxui::ui_core::Paint struct <'a> @U1a.1
-pxui::ui_core::Paint::brackets fn (&mut self, rect: Rect, offset: f32, length: f32, stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::chevron fn (&mut self, at: (f32, f32), direction: Dir, stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::circle fn (&mut self, at: (f32, f32), radius: f32, fill: Option<Fill>, stroke: Option<Stroke>) @U1a.1
-pxui::ui_core::Paint::clip_radius fn (&self) -> f32 @U1a.1
-pxui::ui_core::Paint::color fn (&self, token: ColorToken) -> Rgba @U1a.1
-pxui::ui_core::Paint::cubic fn (&mut self, points: [(f32, f32); 4], stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::curve fn (&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32), stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::dashed fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::dashed_rect fn (&mut self, rect: Rect, stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::density fn (&self) -> i64 @U1a.1
-pxui::ui_core::Paint::fill fn (&mut self, rect: Rect, fill: Fill) @U1a.1
-pxui::ui_core::Paint::flag fn (&mut self, at: (f32, f32), round: bool, on: bool) @U1a.1
-pxui::ui_core::Paint::frame fn (&mut self, rect: Rect, stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::grid fn (&mut self, rect: Rect, origin: (f32, f32), spacing: f32, dot: f32, fill: Fill) @U1a.1
-pxui::ui_core::Paint::icon fn (&mut self, at: (f32, f32), font: u16, size: f32, ch: char, color: Rgba) -> f32 @U1a.1
-pxui::ui_core::Paint::image fn (&mut self, rect: Rect, image: &Image, fit_mode: Fit, tint: Rgba) -> Result<(), Error> @U1a.1
-pxui::ui_core::Paint::line fn (&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::rtl fn (&self) -> bool @U1a.1
-pxui::ui_core::Paint::scroll_y fn (&mut self, slot: u64) -> f32 @U1a.1
-pxui::ui_core::Paint::stroke fn (&mut self, rect: Rect, stroke: Stroke) @U1a.1
-pxui::ui_core::Paint::text fn (&mut self, at: (f32, f32), style: &Label, width: f32, s: &str) -> TextMetrics @U1a.1
-pxui::ui_core::Paint::text_chars fn (&mut self, at: (f32, f32), style: &Label, s: &str) -> f32 @U1a.1
-pxui::ui_core::Paint::text_metrics fn (&mut self, style: &TextStyle, width: f32, s: &str) -> TextMetrics @U1a.1
-pxui::ui_core::Paint::text_spans fn (&mut self, at: (f32, f32), style: &TextStyle, width: f32, text: &str, spans: &[TextSpan]) -> Result<TextMetrics, Error> @U1a.1
-pxui::ui_core::Paint::theme fn (&self) -> Tokens @U1a.1
-pxui::ui_core::Paint::triangle fn (&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), fill: Fill) @U1a.1
-pxui::ui_core::Scroll impl Clone @U1a.1
-pxui::ui_core::Scroll impl Copy @U1a.1
-pxui::ui_core::Scroll impl Debug @U1a.1
-pxui::ui_core::Scroll impl Default @U1a.1
-pxui::ui_core::Scroll impl PartialEq @U1a.1
-pxui::ui_core::Scroll impl StructuralPartialEq @U1a.1
-pxui::ui_core::Scroll struct @U1a.1
-pxui::ui_core::Scroll::painted field f32 @U1a.1
-pxui::ui_core::Scroll::settled field f32 @U1a.1
-pxui::ui_core::Size use pxui_types::Size @U1a.1
-pxui::ui_core::SwipeActions impl Clone @U1a.1
-pxui::ui_core::SwipeActions impl Copy @U1a.1
-pxui::ui_core::SwipeActions impl Debug @U1a.1
-pxui::ui_core::SwipeActions impl Default @U1a.1
-pxui::ui_core::SwipeActions impl PartialEq @U1a.1
-pxui::ui_core::SwipeActions impl StructuralPartialEq @U1a.1
-pxui::ui_core::SwipeActions struct @U1a.1
-pxui::ui_core::SwipeActions::DEFAULT const SwipeActions @U1a.1
-pxui::ui_core::SwipeActions::full fn const (self, travel: f32) -> SwipeActions @U1a.1
-pxui::ui_core::SwipeActions::leading fn const (self, width: f32) -> SwipeActions @U1a.1
-pxui::ui_core::SwipeActions::trailing fn const (self, width: f32) -> SwipeActions @U1a.1
-pxui::ui_core::Ui impl Drop @U1a.1
-pxui::ui_core::Ui struct (_) @U1a.1
-pxui::ui_core::Ui::a11y_name fn (&mut self, box_: &Handle, name: &str) @U1a.1
-pxui::ui_core::Ui::a11y_name_of fn (&self, node: &A11yNode) -> &str @U1a.1
-pxui::ui_core::Ui::a11y_nodes fn (&self) -> &[A11yNode] @U1a.1
-pxui::ui_core::Ui::a11y_number fn (&mut self, box_: &Handle, now: f64, min: f64, max: f64, step: f64) @U1a.1
-pxui::ui_core::Ui::a11y_tree fn (&self) -> (&[u8], u64, u32) @U1a.1
-pxui::ui_core::Ui::a11y_value fn (&mut self, box_: &Handle, value: &str, selection: (u32, u32)) @U1a.1
-pxui::ui_core::Ui::a11y_value_of fn (&self, node: &A11yNode) -> Option<&str> @U1a.1
-pxui::ui_core::Ui::anchored fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, anchor: Rect, overlay: &Overlay<'_>, f: F) -> Handle @U1a.1
-pxui::ui_core::Ui::animate fn (&mut self, b: &Handle, target: Target, curve: Curve) -> bool @U1a.1
-pxui::ui_core::Ui::animate_keys fn (&mut self, box_: &Handle, keys: &[Keyframe], play: &Keyframes) -> bool @U1a.1
-pxui::ui_core::Ui::arranged_nodes fn (&self) -> usize @U1a.1
-pxui::ui_core::Ui::background_request fn (&self) -> Option<Rgba> @U1a.1
-pxui::ui_core::Ui::badge fn <K: Into<Key>>(&mut self, key: K, name: &str, text: &str) -> Handle @U1a.1
-pxui::ui_core::Ui::box_of_a11y fn (&self, id: u64) -> Option<Handle> @U1a.1
-pxui::ui_core::Ui::box_of_key fn (&self, key: i64) -> Option<Handle> @U1a.1
-pxui::ui_core::Ui::built_nodes fn (&self) -> usize @U1a.1
-pxui::ui_core::Ui::busy_step fn (&mut self, box_: &Handle, now: f64) -> bool @U1a.1
-pxui::ui_core::Ui::button fn (&mut self, text: &str) -> bool @U1a.1
-pxui::ui_core::Ui::button_content fn <K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, button: &Button<'_>, f: F) -> Response @U1a.1
-pxui::ui_core::Ui::button_with fn <K: Into<Key>>(&mut self, key: K, text: &str, button: &Button<'_>) -> Response @U1a.1
-pxui::ui_core::Ui::bytes fn (&self) -> Bytes @U1a.1
-pxui::ui_core::Ui::cached_label fn (&mut self, b: &Handle, index: usize, text: &str) -> &str @U1a.1
-pxui::ui_core::Ui::cancel_carry fn (&mut self) @U1a.1
-pxui::ui_core::Ui::canvas fn <F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) @U1a.1
-pxui::ui_core::Ui::canvas_over fn <F: FnMut(&mut Paint<'_>, Rect) + 'static>(&mut self, b: &Handle, f: F) @U1a.1
-pxui::ui_core::Ui::cap_width fn (&mut self, text: &str) -> f32 @U1a.1
-pxui::ui_core::Ui::captured fn (&self) -> bool @U1a.1
-pxui::ui_core::Ui::carry fn (&mut self, from: Option<&Handle>, kind: &str, value: &str) @U1a.1
-pxui::ui_core::Ui::carrying fn (&self) -> Option<PayloadRef<'_>> @U1a.1
-pxui::ui_core::Ui::checkbox fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, checkbox: &Checkbox) -> Response @U1a.1
-pxui::ui_core::Ui::clipboard_request fn (&self) -> Option<&str> @U1a.1
-pxui::ui_core::Ui::close_swipe fn (&mut self, node: &Handle) @U1a.1
-pxui::ui_core::Ui::color fn (&self, token: ColorToken) -> Rgba @U1a.1
-pxui::ui_core::Ui::column fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1a.1
-pxui::ui_core::Ui::content_rect fn (&self) -> Rect @U1a.1
-pxui::ui_core::Ui::context_at fn (&mut self, b: &Handle) -> Option<(f32, f32)> @U1a.1
-pxui::ui_core::Ui::create fn (platform: Platform, font_size: Option<i64>, fonts: &[&[u8]]) -> Self @U1a.1
-pxui::ui_core::Ui::density fn (&self) -> i64 @U1a.1
-pxui::ui_core::Ui::dialog fn <K: Into<Key>>(&mut self, key: K, title: &str, buttons: &[DialogButton<'_>], dialog: &Dialog, body: &mut dyn FnMut(&mut Ui)) -> Chosen @U1a.1
-pxui::ui_core::Ui::disabled fn <F: FnMut(&mut Ui)>(&mut self, disabled: bool, f: F) @U1a.1
-pxui::ui_core::Ui::dismiss_popup fn (&mut self) @U1a.1
-pxui::ui_core::Ui::draw_revision fn (&self) -> u64 @U1a.1
-pxui::ui_core::Ui::drawn fn (&mut self, index: usize) @U1a.1
-pxui::ui_core::Ui::drop_target fn (&mut self, b: &Handle) -> Option<DropRef<'_>> @U1a.1
-pxui::ui_core::Ui::dropdown fn <K: Into<Key>>(&mut self, key: K, text: &str, options: &[&str], selected: usize, dropdown: &Dropdown) -> (Response, usize) @U1a.1
-pxui::ui_core::Ui::enable_system_fonts fn (&mut self) -> Result<(), Error> @U1a.1
-pxui::ui_core::Ui::end_refresh fn (&mut self, node: &Handle) @U1a.1
-pxui::ui_core::Ui::enter fn (&mut self, box_: &Handle) -> UiSaved @U1a.1
-pxui::ui_core::Ui::environment fn (&self) -> Environment @U1a.1
-pxui::ui_core::Ui::expanded fn (&self, key: &str) -> Option<bool> @U1a.1
-pxui::ui_core::Ui::field_drag fn (&self, field: &Handle) -> Option<(f32, f32)> @U1a.1
-pxui::ui_core::Ui::focus_box fn (&mut self, box_: &Handle) @U1a.1
-pxui::ui_core::Ui::focused fn (&self, box_: &Handle) -> bool @U1a.1
-pxui::ui_core::Ui::font_size fn (&self) -> i64 @U1a.1
-pxui::ui_core::Ui::footer fn (&mut self, right: Option<&str>, hints: &[(&str, &str)]) @U1a.1
-pxui::ui_core::Ui::frame fn (&mut self, input: &FrameIn, build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> @U1a.1
-pxui::ui_core::Ui::frame_index fn (&self) -> u64 @U1a.1
-pxui::ui_core::Ui::gesture fn (&mut self, node: &Handle, opts: GestureOpts) -> GestureState @U1a.1
-pxui::ui_core::Ui::gesture_state fn (&mut self, node: &Handle) -> GestureState @U1a.1
-pxui::ui_core::Ui::grid_list fn <K: Into<Key>>(&mut self, key: K, count: usize, cell_w: f32, cell_h: f32, build: &mut dyn FnMut(&mut Ui, usize)) -> Handle @U1a.1
-pxui::ui_core::Ui::grip fn (&mut self, label: Arguments<'_>, axis: Axis, rect: Option<Rect>) -> (Handle, Signal) @U1a.1
-pxui::ui_core::Ui::hover_delay fn (&mut self, key: &str) -> bool @U1a.1
-pxui::ui_core::Ui::hovered_exactly fn (&self, b: &Handle) -> bool @U1a.1
-pxui::ui_core::Ui::hovered_within fn (&mut self, b: &Handle) -> bool @U1a.1
-pxui::ui_core::Ui::icon_button fn <K: Into<Key>>(&mut self, key: K, icon: Icon, name: &str, button: &IconButton<'_>) -> Response @U1a.1
-pxui::ui_core::Ui::id fn (&self) -> usize @U1a.1
-pxui::ui_core::Ui::image fn (&mut self, name: &str, image: &Image, style: &ImageStyle) -> Result<Handle, Error> @U1a.1
-pxui::ui_core::Ui::ime fn (&self) -> Option<((i64, i64, i64, i64), i64)> @U1a.1
-pxui::ui_core::Ui::inject_a11y fn (&mut self, actions: &[A11yAction]) @U1a.1
-pxui::ui_core::Ui::input_frame fn (&mut self, input: &Input, build: &mut dyn FnMut(&mut Ui)) -> Result<Frame<'_>, Error> @U1a.1
-pxui::ui_core::Ui::inspector_bar fn (&mut self, hints: &[(&str, &str)], count: i64) @U1a.1
-pxui::ui_core::Ui::inspector_body fn (&mut self, f: &mut dyn FnMut(&mut Ui)) @U1a.1
-pxui::ui_core::Ui::inspector_button fn (&mut self, key: &str, label: &str) -> bool @U1a.1
-pxui::ui_core::Ui::inspector_header fn (&mut self, key: &str, kind: Option<&str>, badge: Option<&str>, index: Option<&str>, rename: Option<Rc<dyn Fn(&str) -> bool>>, actions: &[HeadAction], reset: Option<&str>, reset_enabled: bool, title: &str, detail: &str) -> Head @U1a.1
-pxui::ui_core::Ui::inspector_label_x fn (&self) -> f32 @U1a.1
-pxui::ui_core::Ui::inspector_message fn (&mut self, key: &str, message: &str) @U1a.1
-pxui::ui_core::Ui::inspector_readout fn (&mut self, width: Option<f32>, key: &str, label: &str, value: &str) @U1a.1
-pxui::ui_core::Ui::inspector_row fn (&mut self, width: Option<f32>, pin: Option<bool>, key: &str, label: &str) -> (Handle, f32, f32, f32) @U1a.1
-pxui::ui_core::Ui::inspector_section fn (&mut self, key: &str, expanded: bool, set_expanded: Option<bool>, label: &str, build: &mut dyn FnMut(&mut Ui)) -> bool @U1a.1
-pxui::ui_core::Ui::inspector_toggle fn (&mut self, key: &str, label: &str, value: bool) -> bool @U1a.1
-pxui::ui_core::Ui::inspector_width fn (&self) -> f32 @U1a.1
-pxui::ui_core::Ui::item_text fn (&mut self, text: &str) @U1a.1
-pxui::ui_core::Ui::key_down fn (&self, key: u32) -> bool @U1a.1
-pxui::ui_core::Ui::key_pressed fn (&self, key: impl Into<u32>, modifier: Option<u32>) -> bool @U1a.1
-pxui::ui_core::Ui::keyboard fn (&self) -> Keyboard @U1a.1
-pxui::ui_core::Ui::label fn (&mut self, text: &str, style: Label) -> Handle @U1a.1
-pxui::ui_core::Ui::label_digits fn (&mut self, args: Arguments<'_>, style: Label) -> Handle @U1a.1
-pxui::ui_core::Ui::label_fmt fn (&mut self, args: Arguments<'_>, style: Label) -> Handle @U1a.1
-pxui::ui_core::Ui::last_error fn (&self) -> &str @U1a.1
-pxui::ui_core::Ui::last_press_within fn (&self, events: &[Event], roots: &[i64]) -> Option<i64> @U1a.1
-pxui::ui_core::Ui::layer fn (&self, index: usize) -> Layer<'_> @U1a.1
-pxui::ui_core::Ui::layer_count fn (&self) -> usize @U1a.1
-pxui::ui_core::Ui::layers fn (&self) -> impl Iterator<Item = Layer<'_>> + '_ @U1a.1
-pxui::ui_core::Ui::leave fn (&mut self, saved: UiSaved) @U1a.1
-pxui::ui_core::Ui::library_calls fn (&self) -> u64 @U1a.1
-pxui::ui_core::Ui::line_field fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &LineField<'_>) -> LineFieldResponse<'_> @U1a.1
-pxui::ui_core::Ui::list fn <K: Into<Key>, F: FnMut(&mut Ui, usize)>(&mut self, key: K, count: usize, row_h: f32, row_key: &dyn Fn(usize) -> i64, f: F) -> Handle @U1a.1
-pxui::ui_core::Ui::load_text_edit fn (&mut self, key: i64, text: &str) -> TextEdit @U1a.1
-pxui::ui_core::Ui::local_rect fn (&self, b: &Handle) -> Rect @U1a.1
-pxui::ui_core::Ui::menu fn <K: Into<Key>>(&mut self, key: K, at: (f32, f32), items: &[MenuItem<'_>], menu: &Menu<'_>) -> Chosen @U1a.1
-pxui::ui_core::Ui::menu_bar fn (&mut self, label: &str, menus: &[(&str, &[&str])]) -> Option<(usize, usize)> @U1a.1
-pxui::ui_core::Ui::modal fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> bool @U1a.1
-pxui::ui_core::Ui::new fn () -> Option<Self> @U1a.1
-pxui::ui_core::Ui::node fn (&mut self, key: Key, node: Node) -> Handle @U1a.1
-pxui::ui_core::Ui::node_layout fn (&self, b: &Handle) -> Node @U1a.1
-pxui::ui_core::Ui::node_mut fn <R>(&mut self, b: &Handle, edit: impl FnOnce(&mut NodeFields) -> R) -> R @U1a.1
-pxui::ui_core::Ui::now fn (&self) -> f64 @U1a.1
-pxui::ui_core::Ui::output fn (&self) -> &Output @U1a.1
-pxui::ui_core::Ui::panel fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) @U1a.1
-pxui::ui_core::Ui::part fn (&mut self, part: Part, node: Node) -> Handle @U1a.1
-pxui::ui_core::Ui::part_box fn (&self, owner: i64, part: Part) -> Option<Handle> @U1a.1
-pxui::ui_core::Ui::part_rect fn (&self, owner: i64, part: Part) -> Option<Rect> @U1a.1
-pxui::ui_core::Ui::passed_undo fn (&self) -> Option<UndoKind> @U1a.1
-pxui::ui_core::Ui::paste_requested fn (&self) -> bool @U1a.1
-pxui::ui_core::Ui::patch_field fn (&mut self, field: &Handle, o: &ValueField<'_>, value_text: &str) -> bool @U1a.1
-pxui::ui_core::Ui::patch_look fn (&mut self, b: &Handle, value: &Look) -> bool @U1a.1
-pxui::ui_core::Ui::patch_mark fn (&mut self, b: &Handle, value: Mark) -> bool @U1a.1
-pxui::ui_core::Ui::patch_rect fn (&mut self, b: &Handle, value: Rect) -> bool @U1a.1
-pxui::ui_core::Ui::patch_text fn (&mut self, box_: &Handle, value: &str) -> bool @U1a.1
-pxui::ui_core::Ui::phase_cpu_times fn (&self) -> Result<[Duration; 7], Error> @U1a.1
-pxui::ui_core::Ui::phase_times fn (&self) -> [Duration; 7] @U1a.1
-pxui::ui_core::Ui::picker fn <K: Into<Key>>(&mut self, key: K, placeholder: &str, query: &str, picker: &PickerOpts<'_>, fill: &mut dyn FnMut(&str, &mut PickRows)) -> PickerResponse<'_> @U1a.1
-pxui::ui_core::Ui::platform fn (&self) -> Platform @U1a.1
-pxui::ui_core::Ui::popup fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, overlay: &Overlay<'_>, f: F) -> bool @U1a.1
-pxui::ui_core::Ui::press_keys fn (&self, b: &Handle) -> &[u32] @U1a.1
-pxui::ui_core::Ui::pressable fn <K: Into<Key>, F: FnMut(&mut Ui, State)>(&mut self, key: K, node: Node, pressable: &Pressable<'_>, f: F) -> Response @U1a.1
-pxui::ui_core::Ui::prewarm_text fn (&mut self, text: &str, style: &Label, max_width: f32) -> Result<(), Error> @U1a.1
-pxui::ui_core::Ui::progress fn <K: Into<Key>>(&mut self, key: K, text: &str, value: Option<f32>) -> Handle @U1a.1
-pxui::ui_core::Ui::radio fn <K: Into<Key>>(&mut self, key: K, options: &[&str], selected: usize, choice: &Choice<'_>) -> (Response, usize) @U1a.1
-pxui::ui_core::Ui::rebuilt_named fn (&self, prefix: &str) -> usize @U1a.1
-pxui::ui_core::Ui::rebuilt_views fn (&self) -> usize @U1a.1
-pxui::ui_core::Ui::rect fn (&self, box_: &Handle) -> Rect @U1a.1
-pxui::ui_core::Ui::register_badge_body fn (&mut self, name: &str, body: fn(&mut Ui, &BadgeConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_button_body fn (&mut self, name: &str, body: fn(&mut Ui, &ButtonConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_checkbox_body fn (&mut self, name: &str, body: fn(&mut Ui, &CheckboxConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_dialog_body fn (&mut self, name: &str, body: fn(&mut Ui, &DialogConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_dropdown_body fn (&mut self, name: &str, body: fn(&mut Ui, &DropdownConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_footer_body fn (&mut self, name: &str, body: fn(&mut Ui, &FooterConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_grip_body fn (&mut self, name: &str, body: fn(&mut Ui, &GripConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_icon_button_body fn (&mut self, name: &str, body: fn(&mut Ui, &IconButtonConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_inspector_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &InspectorRowConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_inspector_section_body fn (&mut self, name: &str, body: fn(&mut Ui, &InspectorSectionConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_label_body fn (&mut self, name: &str, body: fn(&mut Ui, &LabelConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_line_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &LineFieldConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_menu_bar_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuBarConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_menu_item_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuItemConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_menu_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &MenuRowConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_modal_body fn (&mut self, name: &str, body: fn(&mut Ui, &ModalConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_panel_body fn (&mut self, name: &str, body: fn(&mut Ui, &PanelConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_picker_body fn (&mut self, name: &str, body: fn(&mut Ui, &PickerConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_picker_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &PickerFieldConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_popup_body fn (&mut self, name: &str, body: fn(&mut Ui, &PopupConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_progress_body fn (&mut self, name: &str, body: fn(&mut Ui, &ProgressConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_radio_body fn (&mut self, name: &str, body: fn(&mut Ui, &RadioConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_scroll_view_body fn (&mut self, name: &str, body: fn(&mut Ui, &ScrollViewConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_search_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &SearchFieldConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_segmented_body fn (&mut self, name: &str, body: fn(&mut Ui, &SegmentedConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_selection_list_body fn (&mut self, name: &str, body: fn(&mut Ui, &SelectionListConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_slider_body fn (&mut self, name: &str, body: fn(&mut Ui, &SliderConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_spinner_body fn (&mut self, name: &str, body: fn(&mut Ui, &SpinnerConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_split_body fn (&mut self, name: &str, body: fn(&mut Ui, &SplitConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_table_cell_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableCellConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_table_head_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableHeadConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_table_row_body fn (&mut self, name: &str, body: fn(&mut Ui, &TableRowConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_tabs_body fn (&mut self, name: &str, body: fn(&mut Ui, &TabsConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_text_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &TextFieldConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_toasts_body fn (&mut self, name: &str, body: fn(&mut Ui, &ToastsConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_toggle_body fn (&mut self, name: &str, body: fn(&mut Ui, &ToggleConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_tooltip_body fn (&mut self, name: &str, body: fn(&mut Ui, &TooltipConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_tree_body fn (&mut self, name: &str, body: fn(&mut Ui, &TreeConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_value_field_body fn (&mut self, name: &str, body: fn(&mut Ui, &ValueFieldConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::register_window_body fn (&mut self, name: &str, body: fn(&mut Ui, &WindowConfig)) -> Result<BodyId, String> @U1a.1
-pxui::ui_core::Ui::remember fn (&mut self, edit: &TextEdit, typing: bool) @U1a.1
-pxui::ui_core::Ui::repaint_after fn (&mut self, seconds: f64) @U1a.1
-pxui::ui_core::Ui::repaint_at fn (&self) -> Option<f64> @U1a.1
-pxui::ui_core::Ui::request_cursor fn (&mut self, shape: Cursor) @U1a.1
-pxui::ui_core::Ui::request_frame fn (&mut self) @U1a.1
-pxui::ui_core::Ui::resolve fn (&self, style: &Label) -> TextStyle @U1a.1
-pxui::ui_core::Ui::reveal fn (&mut self, list: &Handle, index: usize) @U1a.1
-pxui::ui_core::Ui::root_at fn (&self, point: (f32, f32), roots: &[i64]) -> Option<i64> @U1a.1
-pxui::ui_core::Ui::routed_input fn (&mut self, owner: Option<i64>) -> Routed<'_> @U1a.1
-pxui::ui_core::Ui::row fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1a.1
-pxui::ui_core::Ui::row_height fn (&self) -> i64 @U1a.1
-pxui::ui_core::Ui::scroll fn (&self, b: &Handle) -> Scroll @U1a.1
-pxui::ui_core::Ui::scroll_view fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, w: Size, h: Size, f: F) -> Handle @U1a.1
-pxui::ui_core::Ui::scrub fn <K: Into<Key>>(&mut self, key: K, value: f32, range: RangeInclusive<f32>, scrubber: &Scrubber<'_>) -> Scrub @U1a.1
-pxui::ui_core::Ui::search_field fn <K: Into<Key>>(&mut self, key: K, value: &str, field: &SearchField<'_>) -> SearchFieldResponse<'_> @U1a.1
-pxui::ui_core::Ui::section_list fn <K: Into<Key>>(&mut self, key: K, section: &SectionList, row_key: &dyn Fn(usize) -> i64, build: &mut dyn FnMut(&mut Ui, SectionPart, f32)) -> Handle @U1a.1
-pxui::ui_core::Ui::segmented_with fn <K: Into<Key>>(&mut self, key: K, options: &[Segment<'_>], selected: Option<usize>, segmented: &Segmented<'_>) -> SegmentedResponse @U1a.1
-pxui::ui_core::Ui::selection_list fn <K: Into<Key>>(&mut self, key: K, count: usize, selected: &mut [u8], list: &SelectionList, cell: &mut dyn FnMut(&mut Ui, usize)) -> ListResponse @U1a.1
-pxui::ui_core::Ui::set_a11y fn (&mut self, on: bool) @U1a.1
-pxui::ui_core::Ui::set_at fn (&mut self, b: &Handle, at: (f32, f32)) @U1a.1
-pxui::ui_core::Ui::set_colors fn (&mut self, colors: &[(Rgba, Rgba)]) @U1a.1
-pxui::ui_core::Ui::set_dir fn (&mut self, rtl: bool) @U1a.1
-pxui::ui_core::Ui::set_font_size fn (&mut self, size: i64) @U1a.1
-pxui::ui_core::Ui::set_look fn (&mut self, box_: &Handle, look: Look, state: State) @U1a.1
-pxui::ui_core::Ui::set_node_layout fn (&mut self, b: &Handle, node: &Node) @U1a.1
-pxui::ui_core::Ui::set_role fn (&mut self, box_: &Handle, role: Role, states: u32) @U1a.1
-pxui::ui_core::Ui::set_scroll_offset fn (&mut self, b: &Handle, value: f32) @U1a.1
-pxui::ui_core::Ui::set_state fn (&mut self, box_: &Handle, value: i64) @U1a.1
-pxui::ui_core::Ui::set_text_state fn (&mut self, b: &Handle, value: Option<&str>) @U1a.1
-pxui::ui_core::Ui::set_text_worker fn (&mut self, worker: Option<&TextWorker>) -> Result<(), Error> @U1a.1
-pxui::ui_core::Ui::set_theme fn (&mut self, source: &ThemeSource) -> Result<(), Error> @U1a.1
-pxui::ui_core::Ui::set_theme_source fn (&mut self, source: &ThemeSource) -> Result<(), Error> @U1a.1
-pxui::ui_core::Ui::set_timing fn (&mut self, on: bool) @U1a.1
-pxui::ui_core::Ui::set_view fn (&mut self, b: &Handle, view: &ViewTransform) @U1a.1
-pxui::ui_core::Ui::signal fn (&self, box_: &Handle) -> Signal @U1a.1
-pxui::ui_core::Ui::signal_keys fn (&self, box_: &Handle) -> &[Event] @U1a.1
-pxui::ui_core::Ui::slider fn <K: Into<Key>>(&mut self, key: K, text: &str, value: f32, range: RangeInclusive<f32>, slider: &Slider) -> (Response, f32) @U1a.1
-pxui::ui_core::Ui::slot fn (&mut self, slot: PxuiSlot) -> bool @U1a.1
-pxui::ui_core::Ui::spinner fn <K: Into<Key>>(&mut self, key: K, name: &str) -> Handle @U1a.1
-pxui::ui_core::Ui::split fn (&mut self, label: &str, axis: Axis, ratio: f32, min_a: f32, min_b: f32, a: &mut dyn FnMut(&mut Ui), b: &mut dyn FnMut(&mut Ui)) -> Option<f32> @U1a.1
-pxui::ui_core::Ui::stack fn <K: Into<Key>, F: FnMut(&mut Ui)>(&mut self, key: K, node: Node, f: F) -> Handle @U1a.1
-pxui::ui_core::Ui::state fn (&self, box_: &Handle, default: i64) -> i64 @U1a.1
-pxui::ui_core::Ui::style fn (&mut self, box_: &Handle, states: &States, state: State) @U1a.1
-pxui::ui_core::Ui::system_appearance fn (&self) -> u32 @U1a.1
-pxui::ui_core::Ui::table_view fn <K: Into<Key>>(&mut self, key: K, titles: &[&str], widths: &mut [f32], rows: usize, selected: &mut Option<usize>, table: &TableView, cell: &mut dyn FnMut(&mut Ui, usize, usize)) -> TableResponse @U1a.1
-pxui::ui_core::Ui::tabs fn <K: Into<Key>>(&mut self, key: K, tabs: &[&str], selected: usize, choice: &Choice<'_>) -> (Response, usize) @U1a.1
-pxui::ui_core::Ui::text fn (&mut self, box_: &Handle, value: &str) @U1a.1
-pxui::ui_core::Ui::text_area fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, area: &TextArea<'_>, callbacks: &AreaCallbacks<'_>) -> TextAreaResponse<'_> @U1a.1
-pxui::ui_core::Ui::text_field fn <K: Into<Key>>(&mut self, key: K, value: &str, field: &TextField<'_>) -> TextFieldResponse<'_> @U1a.1
-pxui::ui_core::Ui::text_input_focused fn (&self) -> bool @U1a.1
-pxui::ui_core::Ui::text_input_options fn (&self) -> TextTraits @U1a.1
-pxui::ui_core::Ui::text_layout_count fn (&self) -> u64 @U1a.1
-pxui::ui_core::Ui::text_line_box fn (&mut self, box_: &Handle, text: &str, style: &Label, spans: &[TextSpan]) -> Result<(), Error> @U1a.1
-pxui::ui_core::Ui::text_service fn (&mut self) -> UiText<'_> @U1a.1
-pxui::ui_core::Ui::text_spans fn (&mut self, box_: &Handle, text: &str, style: &Label, spans: &[TextSpan]) -> Result<(), Error> @U1a.1
-pxui::ui_core::Ui::text_state fn (&self, b: &Handle) -> Option<&str> @U1a.1
-pxui::ui_core::Ui::text_top fn (&self, size: Option<i64>, y: f32, h: f32) -> f32 @U1a.1
-pxui::ui_core::Ui::text_undo_bytes fn (&self) -> usize @U1a.1
-pxui::ui_core::Ui::text_width fn (&mut self, size: Option<i64>, text: &str) -> f32 @U1a.1
-pxui::ui_core::Ui::theme fn (&self) -> &Theme @U1a.1
-pxui::ui_core::Ui::themed fn (&mut self, theme: &Theme, f: &mut dyn FnMut(&mut Ui)) @U1a.1
-pxui::ui_core::Ui::to_front fn (&mut self, order: i64, b: &Handle) @U1a.1
-pxui::ui_core::Ui::toasts fn <K: Into<Key>>(&mut self, key: K, items: &[Toast<'_>], seconds: f64) -> Option<usize> @U1a.1
-pxui::ui_core::Ui::toggle fn (&mut self, text: &str, on: bool) -> bool @U1a.1
-pxui::ui_core::Ui::toggle_with fn <K: Into<Key>>(&mut self, key: K, text: &str, on: bool, toggle: &Toggle<'_>) -> Response @U1a.1
-pxui::ui_core::Ui::tokens fn (&self) -> Tokens @U1a.1
-pxui::ui_core::Ui::tooltip fn (&mut self, box_: &Handle, text: &str) @U1a.1
-pxui::ui_core::Ui::transfer_carry fn (&mut self, to: &Handle) -> bool @U1a.1
-pxui::ui_core::Ui::tree fn <K: Into<Key>>(&mut self, key: K, nodes: &[TreeNode], open: &mut [u8], selected: &mut Option<usize>, visible: &mut [u32], cell: &mut dyn FnMut(&mut Ui, usize)) -> TreeResponse @U1a.1
-pxui::ui_core::Ui::unfocus fn (&mut self) @U1a.1
-pxui::ui_core::Ui::use_theme fn (&mut self, theme: &Theme) @U1a.1
-pxui::ui_core::Ui::value_field fn <K: Into<Key>>(&mut self, key: K, value: &str, rect: Rect, field: &ValueField<'_>, callbacks: &ValueCallbacks<'_>) -> ValueFieldResponse<'_> @U1a.1
-pxui::ui_core::Ui::verify_new_views fn (on: bool) @U1a.1
-pxui::ui_core::Ui::verify_views fn (&mut self, on: bool) @U1a.1
-pxui::ui_core::Ui::verifying fn (&self) -> bool @U1a.1
-pxui::ui_core::Ui::view_backdrop_blur fn (&mut self, b: &Handle, sigma: f32) @U1a.1
-pxui::ui_core::Ui::view_backdrop_saturation fn (&mut self, b: &Handle, saturation: f32) @U1a.1
-pxui::ui_core::Ui::view_clip fn (&mut self, b: &Handle, clip: Option<Rect>) @U1a.1
-pxui::ui_core::Ui::view_of fn <K: Into<Key>, T: Hash + ?Sized, F: FnMut(&mut Ui)>(&mut self, key: K, inputs: &T, f: F) -> Handle @U1a.1
-pxui::ui_core::Ui::view_size fn (&self) -> (f32, f32) @U1a.1
-pxui::ui_core::Ui::wants_frame fn (&self, events: &[Event], now: f64) -> bool @U1a.1
-pxui::ui_core::Ui::within fn (&mut self, box_: &Handle, f: &mut dyn FnMut(&mut Ui)) @U1a.1
-pxui::ui_core::ViewTransform impl Clone @U1a.1
-pxui::ui_core::ViewTransform impl Copy @U1a.1
-pxui::ui_core::ViewTransform impl Debug @U1a.1
-pxui::ui_core::ViewTransform impl Default @U1a.1
-pxui::ui_core::ViewTransform impl PartialEq @U1a.1
-pxui::ui_core::ViewTransform impl StructuralPartialEq @U1a.1
-pxui::ui_core::ViewTransform struct @U1a.1
-pxui::ui_core::ViewTransform::DEFAULT const ViewTransform @U1a.1
-pxui::ui_core::ViewTransform::about fn const (self, scale: f32, matrix: [f32; 4], pivot: (f32, f32)) -> ViewTransform @U1a.1
-pxui::ui_core::ViewTransform::offset fn const (self, dx: f32, dy: f32) -> ViewTransform @U1a.1
-pxui::ui_core::ViewTransform::opacity fn const (self, opacity: f32) -> ViewTransform @U1a.1
-pxui::ui_core::ViewTransform::scale fn const (self, scale: f32) -> ViewTransform @U1a.1
-pxui::ui_core::key fn (b: &Handle) -> i64 @U1a.1
-pxui::ui_text mod @U1a.1
-pxui::ui_text::Chosen use pxui_types::ui::Chosen @U1a.1
-pxui::ui_text::CodeContext use pxui_types::ui::CodeContext @U1a.1
-pxui::ui_text::Completion use pxui_types::ui::Completion @U1a.1
-pxui::ui_text::Language impl Clone @U1a.1
-pxui::ui_text::Language impl Copy @U1a.1
-pxui::ui_text::Language struct <'a> @U1a.1
-pxui::ui_text::Language::DEFAULT const Language<'a> @U1a.1
-pxui::ui_text::Language::brackets fn const (self, f: &'a BracketsFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Language::colorize fn const (self, f: &'a ColorizeFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Language::comment fn const (self, prefix: &'a str) -> Language<'a> @U1a.1
-pxui::ui_text::Language::complete fn const (self, f: &'a CompleteFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Language::context_at fn const (self, f: &'a ContextAtFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Language::describe fn const (self, f: &'a DescribeFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Language::indent fn const (self, f: &'a IndentFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Language::number_at fn const (self, f: &'a NumberAtFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Language::pairs fn const (self, pairs: &'a [(char, char)]) -> Language<'a> @U1a.1
-pxui::ui_text::Language::rewrite fn const (self, f: &'a RewriteFn<'a>) -> Language<'a> @U1a.1
-pxui::ui_text::Menu use pxui_types::ui::Menu @U1a.1
-pxui::ui_text::MenuItem use pxui_types::ui::MenuItem @U1a.1
-pxui::ui_text::Pick use pxui_types::ui::Pick @U1a.1
-pxui::ui_text::ScrubPhase use crate::ScrubPhase @U1a.1
-pxui::ui_text::TextArea impl Clone @U1a.1
-pxui::ui_text::TextArea impl Copy @U1a.1
-pxui::ui_text::TextArea impl Debug @U1a.1
-pxui::ui_text::TextArea impl PartialEq @U1a.1
-pxui::ui_text::TextArea impl StructuralPartialEq @U1a.1
-pxui::ui_text::TextArea struct <'a> @U1a.1
-pxui::ui_text::TextArea::DEFAULT const TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::chips fn const (self, chips: &'a [(usize, usize, Rgba)]) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::errors fn const (self, lines: &'a [usize]) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::name fn const (self, name: &'a str) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::notes fn const (self, notes: &'a [AreaNote<'a>]) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::reveal fn const (self, reveal: Reveal) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::select fn const (self, start: usize, end: usize) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::spans fn const (self, spans: &'a [(usize, usize)]) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::splices fn const (self, splices: &'a [Splice<'a>]) -> TextArea<'a> @U1a.1
-pxui::ui_text::TextArea::wrap fn const (self, on: bool) -> TextArea<'a> @U1a.1
-pxui::ui_text::ValueField impl Clone @U1a.1
-pxui::ui_text::ValueField impl Copy @U1a.1
-pxui::ui_text::ValueField impl Debug @U1a.1
-pxui::ui_text::ValueField impl PartialEq @U1a.1
-pxui::ui_text::ValueField impl StructuralPartialEq @U1a.1
-pxui::ui_text::ValueField struct <'a> @U1a.1
-pxui::ui_text::ValueField::DEFAULT const ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::bare fn const (self, on: bool) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::display fn const (self, text: &'a str) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::edit fn const (self, on: bool) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::fraction fn const (self, fraction: f32) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::kept_drag fn const (self, on: bool) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::lead fn const (self, text: &'a str, color: Rgba) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::left fn const (self, on: bool) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::line fn const (self, color: Rgba) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::name fn const (self, name: &'a str) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::placeholder fn const (self, text: &'a str) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::size fn const (self, points: usize) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::tracking fn const (self, tracking: f32) -> ValueField<'a> @U1a.1
-pxui::ui_text::ValueField::trail fn const (self, text: &'a str, color: Rgba) -> ValueField<'a> @U1a.1
-pxui::ui_text::context_clicked fn (signal: &Signal) -> bool @U1a.1
-pxui::ui_text::fuzzy_match use pxui_types::text_edit::fuzzy_match @U1a.1
-pxui::ui_text::fuzzy_score use pxui_types::text_edit::fuzzy_score @U1a.1
-pxui::ui_text::line_end use pxui_types::text_edit::line_end @U1a.1
-pxui::ui_text::line_start use pxui_types::text_edit::line_start @U1a.1
-pxui::ui_text::scrubbed use pxui_types::text_edit::scrubbed @U1a.1
-pxui::ui_widgets mod @U1a.1
-pxui::ui_widgets::Chosen use crate::Chosen @U1a.1
-pxui::ui_widgets::Dialog use crate::Dialog @U1a.1
-pxui::ui_widgets::DialogButton use crate::DialogButton @U1a.1
-pxui::ui_widgets::Scrub impl Clone @U1a.1
-pxui::ui_widgets::Scrub impl Copy @U1a.1
-pxui::ui_widgets::Scrub impl Debug @U1a.1
-pxui::ui_widgets::Scrub struct @U1a.1
-pxui::ui_widgets::Scrub::commit field Option<f32> @U1a.1
-pxui::ui_widgets::Scrub::held field bool @U1a.1
-pxui::ui_widgets::Scrub::node field Handle @U1a.1
-pxui::ui_widgets::Scrub::preview field Option<f32> @U1a.1
-pxui::ui_widgets::Segment use crate::Segment @U1a.1
-pxui::ui_widgets::TreeNode impl Clone @U1a.1
-pxui::ui_widgets::TreeNode impl Copy @U1a.1
-pxui::ui_widgets::TreeNode impl Debug @U1a.1
-pxui::ui_widgets::TreeNode impl Default @U1a.1
-pxui::ui_widgets::TreeNode impl Eq @U1a.1
-pxui::ui_widgets::TreeNode impl PartialEq @U1a.1
-pxui::ui_widgets::TreeNode impl StructuralPartialEq @U1a.1
-pxui::ui_widgets::TreeNode struct @U1a.1
-pxui::ui_widgets::TreeNode::children field u8 @U1a.1
-pxui::ui_widgets::TreeNode::depth field u16 @U1a.1
-pxui::widgets mod @U1a.1
-pxui::widgets::Scrub impl Clone @U1a.1
-pxui::widgets::Scrub impl Copy @U1a.1
-pxui::widgets::Scrub impl Debug @U1a.1
-pxui::widgets::Scrub struct @U1a.1
-pxui::widgets::Scrub::commit field Option<f32> @U1a.1
-pxui::widgets::Scrub::held field bool @U1a.1
-pxui::widgets::Scrub::node field Handle @U1a.1
-pxui::widgets::Scrub::preview field Option<f32> @U1a.1
pxui_types mod @HEAD
pxui_types::ABI_BUILT const u32 @HEAD
pxui_types::Appearance enum @U1a.3
pxui_types::Appearance impl Clone @U1a.3
pxui_types::Appearance impl Copy @U1a.3
pxui_types::Appearance impl Debug @U1a.3
pxui_types::Appearance impl Eq @U1a.3
pxui_types::Appearance impl PartialEq @U1a.3
pxui_types::Appearance impl StructuralPartialEq @U1a.3
pxui_types::Appearance::Dark variant = FrameIn::APPEARANCE_DARK @U1a.3
pxui_types::Appearance::Light variant = FrameIn::APPEARANCE_LIGHT @U1a.3
pxui_types::Appearance::Unknown variant = FrameIn::APPEARANCE_UNKNOWN @U1a.3
pxui_types::Axis enum @HEAD
pxui_types::Axis impl Clone @HEAD
pxui_types::Axis impl Copy @HEAD
pxui_types::Axis impl Debug @HEAD
pxui_types::Axis impl Eq @HEAD
pxui_types::Axis impl PartialEq @HEAD
pxui_types::Axis impl StructuralPartialEq @HEAD
pxui_types::Axis::Column variant @HEAD
pxui_types::Axis::Row variant @HEAD
pxui_types::Axis::Stack variant @HEAD
pxui_types::Bar impl Clone @HEAD
pxui_types::Bar impl Copy @HEAD
pxui_types::Bar impl Debug @HEAD
pxui_types::Bar impl PartialEq @HEAD
pxui_types::Bar impl StructuralPartialEq @HEAD
pxui_types::Bar struct @HEAD
pxui_types::Bar::color field Rgba @HEAD
pxui_types::Bar::gap field f32 @HEAD
pxui_types::Bar::gap fn const (self, points: f32) -> Bar @HEAD
pxui_types::Bar::margin field f32 @HEAD
pxui_types::Bar::margin fn const (self, points: f32) -> Bar @HEAD
pxui_types::Bar::new fn const (color: Rgba, width: f32) -> Bar @HEAD
pxui_types::Bar::radius field f32 @HEAD
pxui_types::Bar::radius fn const (self, points: f32) -> Bar @HEAD
pxui_types::Bar::whole field bool @HEAD
pxui_types::Bar::whole fn const (self) -> Bar @HEAD
pxui_types::Bar::width field f32 @HEAD
pxui_types::Capitalize enum @HEAD
pxui_types::Capitalize impl Clone @HEAD
pxui_types::Capitalize impl Copy @HEAD
pxui_types::Capitalize impl Debug @HEAD
pxui_types::Capitalize impl Default @HEAD
pxui_types::Capitalize impl Eq @HEAD
pxui_types::Capitalize impl PartialEq @HEAD
pxui_types::Capitalize impl StructuralPartialEq @HEAD
pxui_types::Capitalize::Letters variant = 3 @HEAD
pxui_types::Capitalize::None variant = 0 @HEAD
pxui_types::Capitalize::Sentences variant = 1 @HEAD
pxui_types::Capitalize::Words variant = 2 @HEAD
pxui_types::Commands impl Clone @HEAD
pxui_types::Commands impl Copy @HEAD
pxui_types::Commands impl Debug @HEAD
pxui_types::Commands impl Default @HEAD
pxui_types::Commands struct @HEAD
pxui_types::Commands::mtl_command_buffer field *mut c_void @HEAD
pxui_types::Commands::size field u32 @HEAD
pxui_types::Commands::vk_command_buffer field *mut c_void @HEAD
pxui_types::Curve enum @HEAD
pxui_types::Curve impl Clone @HEAD
pxui_types::Curve impl Copy @HEAD
pxui_types::Curve impl Debug @HEAD
pxui_types::Curve impl From<Curve> @HEAD
pxui_types::Curve impl PartialEq @HEAD
pxui_types::Curve impl StructuralPartialEq @HEAD
pxui_types::Curve::Bezier variant { duration: f32, points: [f32; 4] } @HEAD
pxui_types::Curve::Spring variant { omega: f32 } @HEAD
pxui_types::Curve::Steps variant { duration: f32, count: u32 } @HEAD
pxui_types::Curve::Table variant { duration: f32, count: u32, positions: [f32; 16], values: [f32; 16] } @HEAD
pxui_types::Curve::at fn (&self, t: f32) -> f32 @HEAD
pxui_types::Curve::bezier_valid fn (duration: f32, points: &[f32; 4]) -> bool @HEAD
pxui_types::Curve::finished fn (&self, t: f32) -> bool @HEAD
pxui_types::Curve::instant fn (&self) -> bool @HEAD
pxui_types::Curve::spring_valid fn (omega: f32) -> bool @HEAD
pxui_types::Curve::valid fn (&self) -> bool @HEAD
pxui_types::Curve::with_duration fn (self, seconds: f32) -> Self @HEAD
pxui_types::Dir enum @HEAD
pxui_types::Dir impl Clone @HEAD
pxui_types::Dir impl Copy @HEAD
pxui_types::Dir impl Debug @HEAD
pxui_types::Dir impl Eq @HEAD
pxui_types::Dir impl PartialEq @HEAD
pxui_types::Dir impl StructuralPartialEq @HEAD
pxui_types::Dir::Down variant @HEAD
pxui_types::Dir::Left variant @HEAD
pxui_types::Dir::Right variant @HEAD
pxui_types::Dir::Up variant @HEAD
pxui_types::Environment impl Clone @HEAD
pxui_types::Environment impl Copy @HEAD
pxui_types::Environment impl Debug @HEAD
pxui_types::Environment impl Default @HEAD
pxui_types::Environment impl PartialEq @HEAD
pxui_types::Environment impl StructuralPartialEq @HEAD
pxui_types::Environment struct @HEAD
pxui_types::Environment::keyboard_animation fn const (self, frame: [f32; 4], from: [f32; 4], started: f64, duration: f64, curve: u32, visible: bool) -> Environment @U5.4
pxui_types::Environment::keyboard_curve field u32 @HEAD
pxui_types::Environment::keyboard_duration field f64 @HEAD
pxui_types::Environment::keyboard_frame field [f32; 4] @HEAD
pxui_types::Environment::keyboard_from field [f32; 4] @HEAD
pxui_types::Environment::keyboard_started field f64 @HEAD
pxui_types::Environment::keyboard_visible field u32 @HEAD
pxui_types::Environment::new fn const (safe_area: [f32; 4]) -> Environment @U1b.5
pxui_types::Environment::overlay_scrollers fn const (self, on: bool) -> Environment @V1
pxui_types::Environment::OVERLAY_SCROLLERS const u32 @V1
pxui_types::Environment::SCROLLER_HOLD const f64 @V1
pxui_types::Environment::scrollers field u32 @V1
pxui_types::Environment::safe_area field [f32; 4] @HEAD
pxui_types::Environment::size field u32 @U5.4
pxui_types::Event impl Clone @HEAD
pxui_types::Event impl Copy @HEAD
pxui_types::Event impl Debug @HEAD
pxui_types::Event impl Default @HEAD
pxui_types::Event struct @HEAD
pxui_types::Event::CLOSED const u32 @HEAD
pxui_types::Event::DIALOG_OPENED const u32 @HEAD
pxui_types::Event::ENVIRONMENT_CHANGED const u32 @HEAD
pxui_types::Event::FILES_DROPPED const u32 @HEAD
pxui_types::Event::FILE_DIALOG const u32 @HEAD
pxui_types::Event::FILE_DRAG_ENDED const u32 @HEAD
pxui_types::Event::FILE_DRAG_MOVED const u32 @HEAD
pxui_types::Event::FOCUS_GAINED const u32 @HEAD
pxui_types::Event::FOCUS_LOST const u32 @HEAD
pxui_types::Event::KEY_PRESSED const u32 @HEAD
pxui_types::Event::KEY_RELEASED const u32 @HEAD
pxui_types::Event::PASTE const u32 @HEAD
pxui_types::Event::PINCHED const u32 @HEAD
pxui_types::Event::PIXEL_SIZE_CHANGED const u32 @HEAD
pxui_types::Event::POINTER_CANCELLED const u32 @HEAD
pxui_types::Event::POINTER_MOVED const u32 @HEAD
pxui_types::Event::POINTER_PRESSED const u32 @HEAD
pxui_types::Event::POINTER_RELEASED const u32 @HEAD
pxui_types::Event::RESIZED const u32 @HEAD
pxui_types::Event::SCROLLED const u32 @HEAD
pxui_types::Event::TEXT_EDITING const u32 @HEAD
pxui_types::Event::TEXT_INPUT const u32 @HEAD
pxui_types::Event::TOUCH const u32 @HEAD
pxui_types::Event::TRACKPAD_SCROLLED const u32 @HEAD
pxui_types::Event::VISIBILITY_CHANGED const u32 @HEAD
pxui_types::Event::code field u32 @HEAD
pxui_types::Event::key fn const (kind: u32, code: u32) -> Event @U1b.5
pxui_types::Event::kind field u32 @HEAD
pxui_types::Event::new fn const (kind: u32) -> Event @U1b.5
pxui_types::Event::pointer fn const (kind: u32, code: u32, x: f64, y: f64) -> Event @U1b.5
pxui_types::Event::reserved field u32 @U5.1
pxui_types::Event::size field u32 @U5.1
pxui_types::Event::surface field u64 @U5.1
pxui_types::Event::text field Str @HEAD
pxui_types::Event::value field f64 @HEAD
pxui_types::Event::x field f64 @HEAD
pxui_types::Event::y field f64 @HEAD
pxui_types::Flags impl BitOr @HEAD
pxui_types::Flags impl BitOrAssign @HEAD
pxui_types::Flags impl Clone @HEAD
pxui_types::Flags impl Copy @HEAD
pxui_types::Flags impl Debug @HEAD
pxui_types::Flags impl Default @HEAD
pxui_types::Flags impl Eq @HEAD
pxui_types::Flags impl Hash @HEAD
pxui_types::Flags impl PartialEq @HEAD
pxui_types::Flags impl StructuralPartialEq @HEAD
pxui_types::Flags struct (i64) @HEAD
pxui_types::Flags::BLOCKING const Flags @HEAD
pxui_types::Flags::CAPTURE_UNCLAIMED const Flags @HEAD
pxui_types::Flags::CLICKABLE const Flags @HEAD
pxui_types::Flags::CLIP const Flags @HEAD
pxui_types::Flags::DRAG_FREE const Flags @HEAD
pxui_types::Flags::FIT_TEXT const Flags @HEAD
pxui_types::Flags::FIT_TEXT_PIXEL const Flags @HEAD
pxui_types::Flags::FOCUSABLE const Flags @HEAD
pxui_types::Flags::FOCUS_MARK const Flags @HEAD
pxui_types::Flags::HOVER_FREE const Flags @HEAD
pxui_types::Flags::KEEP_FOCUS const Flags @HEAD
pxui_types::Flags::KEEP_ROOM const Flags @HEAD
pxui_types::Flags::KEEP_TAB const Flags @HEAD
pxui_types::Flags::NONE const Flags @HEAD
pxui_types::Flags::OVER_THUMB const Flags @HEAD
pxui_types::Flags::ROUND_CLIP const Flags @HEAD
pxui_types::Flags::SCROLL const Flags @HEAD
pxui_types::Flags::SCROLL_X const Flags @HEAD
pxui_types::Flags::STICKY const Flags @HEAD
pxui_types::Flags::TAB_ONLY const Flags @HEAD
pxui_types::Flags::TAB_STOP const Flags @HEAD
pxui_types::Flags::TAB_STOP_MARKED const Flags @HEAD
pxui_types::Flags::any fn const (self, other: Flags) -> bool @HEAD
pxui_types::Flags::bits fn const (self) -> i64 @HEAD
pxui_types::Flags::or fn const (self, other: Flags) -> Flags @HEAD
pxui_types::Font impl Clone @HEAD
pxui_types::Font impl Copy @HEAD
pxui_types::Font impl Debug @HEAD
pxui_types::Font impl PartialEq @HEAD
pxui_types::Font impl StructuralPartialEq @HEAD
pxui_types::Font struct @HEAD
pxui_types::Font::BODY const Font @HEAD
pxui_types::Font::caps field bool @HEAD
pxui_types::Font::caps fn const (self) -> Font @HEAD
pxui_types::Font::family field u16 @HEAD
pxui_types::Font::family fn const (self, family: u16) -> Font @HEAD
pxui_types::Font::points fn (&self, body: f32) -> f32 @U1b.3
-pxui_types::Font::points fn (&self, body: i64) -> i64 @U1b.3
pxui_types::Font::size field i64 @HEAD
pxui_types::Font::size fn const (self, points: f32) -> Font @U1b.3
-pxui_types::Font::size fn const (self, points: i64) -> Font @U1b.3
pxui_types::Font::size_points field f32 @U1b.3
pxui_types::Font::tracking field f32 @HEAD
pxui_types::Font::tracking fn const (self, ems: f32) -> Font @HEAD
pxui_types::Font::weight field u16 @HEAD
pxui_types::Font::weight fn const (self, weight: u16) -> Font @HEAD
pxui_types::FrameIn impl Clone @HEAD
pxui_types::FrameIn impl Copy @HEAD
pxui_types::FrameIn impl Debug @HEAD
pxui_types::FrameIn impl Default @HEAD
pxui_types::FrameIn struct @HEAD
pxui_types::FrameIn::APPEARANCE_DARK const u32 @HEAD
pxui_types::FrameIn::APPEARANCE_LIGHT const u32 @HEAD
pxui_types::FrameIn::APPEARANCE_UNKNOWN const u32 @HEAD
pxui_types::FrameIn::MAIN const u64 @U5.1
pxui_types::FrameIn::appearance field u32 @HEAD
pxui_types::FrameIn::environment field Environment @HEAD
pxui_types::FrameIn::events field Strided<Event> @U5.4
pxui_types::FrameIn::height field f32 @HEAD
pxui_types::FrameIn::keyboard field Keyboard @HEAD
pxui_types::FrameIn::new fn const (width: f32, height: f32) -> FrameIn @U1a.3
pxui_types::FrameIn::reduced_motion field u32 @HEAD
pxui_types::FrameIn::scale field f32 @HEAD
pxui_types::FrameIn::size field u32 @HEAD
pxui_types::FrameIn::surface field u64 @HEAD
pxui_types::FrameIn::time field f64 @HEAD
pxui_types::FrameIn::width field f32 @HEAD
pxui_types::FrameOut impl Clone @HEAD
pxui_types::FrameOut impl Copy @HEAD
pxui_types::FrameOut impl Debug @HEAD
pxui_types::FrameOut impl Default @HEAD
pxui_types::FrameOut struct @HEAD
pxui_types::FrameOut::damage field [f32; 4] @HEAD
pxui_types::FrameOut::has_damage field u8 @HEAD
pxui_types::FrameOut::layer_count field u32 @HEAD
pxui_types::FrameOut::layers field *const c_void @HEAD
pxui_types::FrameOut::reserved field [u8; 3] @HEAD
pxui_types::FrameOut::size field u32 @HEAD
pxui_types::Gpu impl Debug @HEAD
pxui_types::Gpu impl Default @HEAD
pxui_types::Gpu struct @HEAD
pxui_types::Gpu::api_version field u32 @HEAD
pxui_types::Gpu::backend field u32 @HEAD
pxui_types::Gpu::device field *mut c_void @HEAD
pxui_types::Gpu::features field u64 @HEAD
pxui_types::Gpu::get_instance_proc_addr field *mut c_void @HEAD
pxui_types::Gpu::hwnd field *mut c_void @HEAD
pxui_types::Gpu::instance field *mut c_void @HEAD
pxui_types::Gpu::metal_layer field *mut c_void @HEAD
pxui_types::Gpu::mtl_device field *mut c_void @HEAD
pxui_types::Gpu::owner_token field *mut c_void @HEAD
pxui_types::Gpu::physical_device field *mut c_void @HEAD
pxui_types::Gpu::pipeline_cache field *mut c_void @HEAD
pxui_types::Gpu::queue field *mut c_void @HEAD
pxui_types::Gpu::queue_family field u32 @HEAD
pxui_types::Gpu::release_owner field Option<extern "C" fn(*mut c_void)> @HEAD
pxui_types::Gpu::size field u32 @HEAD
pxui_types::Gpu::wl_display field *mut c_void @HEAD
pxui_types::Gpu::wl_surface field *mut c_void @HEAD
pxui_types::Image impl Clone @HEAD
pxui_types::Image impl Copy @HEAD
pxui_types::Image impl Debug @HEAD
pxui_types::Image impl Default @HEAD
pxui_types::Image struct @HEAD
pxui_types::Image::FORMAT_BGRA8 const u32 @HEAD
pxui_types::Image::FORMAT_BGRA8_SRGB const u32 @HEAD
pxui_types::Image::FORMAT_DEPTH32F const u32 @HEAD
pxui_types::Image::FORMAT_R8 const u32 @HEAD
pxui_types::Image::FORMAT_RGBA16F const u32 @HEAD
pxui_types::Image::FORMAT_RGBA32F const u32 @HEAD
pxui_types::Image::FORMAT_RGBA8 const u32 @HEAD
pxui_types::Image::format field u32 @HEAD
pxui_types::Image::height field u32 @HEAD
pxui_types::Image::mtl_texture field *mut c_void @HEAD
pxui_types::Image::rgba8 fn const (width: u32, height: u32, vk_image_view: *mut c_void, mtl_texture: *mut c_void) -> Image @U1b.5
pxui_types::Image::size field u32 @HEAD
pxui_types::Image::valid fn (&self) -> bool @HEAD
pxui_types::Image::vk_image_view field *mut c_void @HEAD
pxui_types::Image::width field u32 @HEAD
pxui_types::KEY_SPECIAL const u32 @HEAD
pxui_types::KEY_UNKNOWN const u32 @HEAD
pxui_types::Keyboard impl Clone @HEAD
pxui_types::Keyboard impl Copy @HEAD
pxui_types::Keyboard impl Debug @HEAD
pxui_types::Keyboard impl Default @HEAD
pxui_types::Keyboard impl PartialEq @HEAD
pxui_types::Keyboard impl StructuralPartialEq @HEAD
pxui_types::Keyboard struct @HEAD
pxui_types::Keyboard::ANIMATING const u32 @HEAD
pxui_types::Keyboard::VALID const u32 @HEAD
pxui_types::Keyboard::flags field u32 @HEAD
pxui_types::Keyboard::frame field [f32; 4] @HEAD
pxui_types::Keyboard::new fn const (frame: Rect, flags: u32) -> Keyboard @U1b.5
pxui_types::Keyboard::size field u32 @U5.4
pxui_types::Keyboard::valid fn (&self) -> bool @HEAD
pxui_types::Label impl Clone @HEAD
pxui_types::Label impl Copy @HEAD
pxui_types::Label impl Debug @HEAD
pxui_types::Label impl From<&Label> @HEAD
pxui_types::Label struct (_) @HEAD
pxui_types::Label::BODY const Label @HEAD
pxui_types::Label::CAPTION const Label @HEAD
pxui_types::Label::DEFAULT const Label @HEAD
pxui_types::Label::MONO const Label @HEAD
pxui_types::Label::TITLE const Label @HEAD
pxui_types::Label::align fn const (self, align: TextAlign) -> Label @HEAD
pxui_types::Label::body_variant fn const (&self) -> u16 @HEAD
pxui_types::Label::bold fn const (self) -> Label @HEAD
pxui_types::Label::caps fn const (self, on: bool) -> Label @HEAD
pxui_types::Label::color fn const (self, token: ColorToken) -> Label @HEAD
pxui_types::Label::ellipsis fn const (self) -> Label @HEAD
pxui_types::Label::ellipsis_head fn const (self) -> Label @HEAD
pxui_types::Label::ellipsis_middle fn const (self) -> Label @HEAD
pxui_types::Label::ellipsis_none fn const (self) -> Label @V1
pxui_types::Label::font fn const (self, family: u16) -> Label @HEAD
pxui_types::Label::grow fn const (self) -> Label @HEAD
pxui_types::Label::h_px fn const (self, points: f32) -> Label @HEAD
pxui_types::Label::line_height fn const (self, points: f32) -> Label @HEAD
pxui_types::Label::lines fn const (self, lines: u16) -> Label @HEAD
pxui_types::Label::pad fn const (self, points: f32) -> Label @HEAD
pxui_types::Label::role fn const (self, role: Role) -> Label @HEAD
pxui_types::Label::size fn const (self, points: f32) -> Label @HEAD
pxui_types::Label::tabular fn const (self) -> Label @HEAD
pxui_types::Label::tracking fn const (self, ems: f32) -> Label @HEAD
pxui_types::Label::variant fn const (self, variant: u16) -> Label @HEAD
pxui_types::Label::w fn const (self, width: Size) -> Label @HEAD
pxui_types::Label::w_px fn const (self, points: f32) -> Label @HEAD
pxui_types::Label::weight fn const (self, weight: u16) -> Label @HEAD
pxui_types::Label::whole_pixels fn const (self) -> Label @HEAD
pxui_types::Label::wrap fn const (self) -> Label @HEAD
pxui_types::Label::wrap_glyph fn const (self) -> Label @HEAD
pxui_types::Label::wrap_spaces fn const (self) -> Label @HEAD
pxui_types::Look impl Clone @HEAD
pxui_types::Look impl Copy @HEAD
pxui_types::Look impl Debug @HEAD
pxui_types::Look impl From<Look> @HEAD
pxui_types::Look impl From<PxuiLookDesc> @HEAD
pxui_types::Look impl PartialEq @HEAD
pxui_types::Look impl StructuralPartialEq @HEAD
pxui_types::Look struct @HEAD
pxui_types::Look::NONE const Look @HEAD
pxui_types::Look::angle field f32 @HEAD
pxui_types::Look::angle fn const (self, degrees: f32) -> Look @HEAD
pxui_types::Look::border field Rgba @HEAD
pxui_types::Look::border fn const (self, color: Rgba, width: f32) -> Look @HEAD
pxui_types::Look::border_w field f32 @HEAD
pxui_types::Look::caret field Rgba @HEAD
pxui_types::Look::caret fn const (self, color: Rgba) -> Look @HEAD
pxui_types::Look::dashed field bool @HEAD
pxui_types::Look::dashed fn const (self) -> Look @HEAD
pxui_types::Look::fill field Rgba @HEAD
pxui_types::Look::fill fn const (self, color: Rgba) -> Look @HEAD
pxui_types::Look::font field Font @HEAD
pxui_types::Look::font fn const (self, font: Font) -> Look @HEAD
pxui_types::Look::gradient field OptColor @HEAD
pxui_types::Look::gradient fn const (self, to: Rgba) -> Look @HEAD
pxui_types::Look::lerp fn (&self, to: &Look, t: f32) -> Look @HEAD
pxui_types::Look::opacity field f32 @HEAD
pxui_types::Look::opacity fn const (self, opacity: f32) -> Look @HEAD
pxui_types::Look::radii fn const (self, points: [f32; 4]) -> Look @HEAD
pxui_types::Look::radius field [f32; 4] @HEAD
pxui_types::Look::radius fn const (self, points: f32) -> Look @HEAD
pxui_types::Look::ring field Ring @HEAD
pxui_types::Look::ring fn const (self, ring: Ring) -> Look @HEAD
pxui_types::Look::selection field Rgba @HEAD
pxui_types::Look::selection fn const (self, color: Rgba) -> Look @HEAD
pxui_types::Look::shadow fn const (self, shadow: Shadow) -> Look @HEAD
pxui_types::Look::shadows field [Shadow; 2] @HEAD
pxui_types::Look::shadows fn const (self, shadows: [Shadow; 2]) -> Look @HEAD
pxui_types::Look::sides field u8 @HEAD
pxui_types::Look::sides fn const (self, sides: u8) -> Look @HEAD
pxui_types::Look::text field Rgba @HEAD
pxui_types::Look::text fn const (self, color: Rgba) -> Look @HEAD
pxui_types::MAX_LAYERS const usize @HEAD
pxui_types::Mark enum @HEAD
pxui_types::Mark impl Clone @HEAD
pxui_types::Mark impl Copy @HEAD
pxui_types::Mark impl Debug @HEAD
pxui_types::Mark impl PartialEq @HEAD
pxui_types::Mark impl StructuralPartialEq @HEAD
pxui_types::Mark::Chevron variant (Dir) @HEAD
pxui_types::Mark::Line variant { width: f32, rising: bool } @HEAD
pxui_types::Mark::None variant @HEAD
pxui_types::Motion impl Clone @HEAD
pxui_types::Motion impl Copy @HEAD
pxui_types::Motion impl Debug @HEAD
pxui_types::Motion impl From<PxuiMotion> @HEAD
pxui_types::Motion impl PartialEq @HEAD
pxui_types::Motion impl StructuralPartialEq @HEAD
pxui_types::Motion struct @HEAD
pxui_types::Motion::ease field [f32; 4] @HEAD
pxui_types::Motion::fast field f32 @HEAD
pxui_types::Motion::normal field f32 @HEAD
pxui_types::Motion::slow field f32 @HEAD
pxui_types::Node impl Clone @HEAD
pxui_types::Node impl Copy @HEAD
pxui_types::Node impl Debug @HEAD
pxui_types::Node struct @HEAD
pxui_types::Node::DEFAULT const Node @HEAD
pxui_types::Node::a11y field u32 @HEAD
pxui_types::Node::a11y fn const (self, states: u32) -> Node @HEAD
pxui_types::Node::align field Align @HEAD
pxui_types::Node::align fn const (self, align: Align) -> Node @HEAD
pxui_types::Node::align_self field OptAlign @HEAD
pxui_types::Node::align_self fn const (self, align: Align) -> Node @HEAD
pxui_types::Node::anchor field OptAnchor @HEAD
pxui_types::Node::anchor fn const (self, anchor: Anchor) -> Node @HEAD
pxui_types::Node::aspect field f32 @HEAD
pxui_types::Node::aspect fn const (self, ratio: f32) -> Node @HEAD
pxui_types::Node::at field OptPoint @HEAD
pxui_types::Node::at fn const (self, x: f32, y: f32) -> Node @HEAD
pxui_types::Node::axis field Axis @HEAD
pxui_types::Node::axis fn const (self, axis: Axis) -> Node @HEAD
pxui_types::Node::bar field OptBar @HEAD
pxui_types::Node::bar fn const (self, bar: Bar) -> Node @HEAD
pxui_types::Node::cell field Cell @HEAD
pxui_types::Node::cell fn const (self, cell: Cell) -> Node @HEAD
pxui_types::Node::center fn const (self) -> Node @HEAD
pxui_types::Node::column fn const () -> Node @HEAD
pxui_types::Node::flags field Flags @HEAD
pxui_types::Node::flags fn const (self, flags: Flags) -> Node @HEAD
pxui_types::Node::gap field f32 @HEAD
pxui_types::Node::gap fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::grid field OptGrid @HEAD
pxui_types::Node::grid fn const (self, grid: Grid) -> Node @HEAD
pxui_types::Node::grow field f32 @HEAD
pxui_types::Node::grow fn const (self, share: f32) -> Node @HEAD
pxui_types::Node::h field Size @HEAD
pxui_types::Node::h fn const (self, size: Size) -> Node @HEAD
pxui_types::Node::h_px fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::inset field Edges @HEAD
pxui_types::Node::inset fn const (self, edges: Edges) -> Node @HEAD
pxui_types::Node::justify field Justify @HEAD
pxui_types::Node::justify fn const (self, justify: Justify) -> Node @HEAD
pxui_types::Node::look field Look @HEAD
pxui_types::Node::look fn const (self, look: Look) -> Node @HEAD
pxui_types::Node::margin field Edges @HEAD
pxui_types::Node::margin fn const (self, edges: Edges) -> Node @HEAD
pxui_types::Node::mark field Mark @HEAD
pxui_types::Node::mark fn const (self, mark: Mark) -> Node @HEAD
pxui_types::Node::max_h field f32 @HEAD
pxui_types::Node::max_h fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::max_w field f32 @HEAD
pxui_types::Node::max_w fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::min_h field f32 @HEAD
pxui_types::Node::min_h fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::min_w field f32 @HEAD
pxui_types::Node::min_w fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::pad fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::pad_x fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::pad_y fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::padding field Edges @HEAD
pxui_types::Node::padding fn const (self, edges: Edges) -> Node @HEAD
pxui_types::Node::role field Role @HEAD
pxui_types::Node::role fn const (self, role: Role) -> Node @HEAD
pxui_types::Node::row fn const () -> Node @HEAD
pxui_types::Node::row_gap field f32 @HEAD
pxui_types::Node::row_gap fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::scroll_step field f32 @HEAD
pxui_types::Node::scroll_step fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::shrink field f32 @HEAD
pxui_types::Node::shrink fn const (self, factor: f32) -> Node @HEAD
pxui_types::Node::stack fn const () -> Node @HEAD
pxui_types::Node::state field State @HEAD
pxui_types::Node::state fn const (self, state: State) -> Node @U1b.5
pxui_types::Node::transition field OptTransition @HEAD
pxui_types::Node::transition fn const (self, transition: Option<Transition>) -> Node @U1b.5
pxui_types::Node::w field Size @HEAD
pxui_types::Node::w fn const (self, size: Size) -> Node @HEAD
pxui_types::Node::w_px fn const (self, points: f32) -> Node @HEAD
pxui_types::Node::wrap field bool @HEAD
pxui_types::Node::wrap fn const (self) -> Node @HEAD
pxui_types::Node::z field i16 @HEAD
pxui_types::Node::z fn const (self, order: i16) -> Node @HEAD
pxui_types::PXUI_ERR_ARGUMENT const i32 @HEAD
pxui_types::PXUI_ERR_GPU const i32 @HEAD
pxui_types::PXUI_ERR_PANIC const i32 @HEAD
pxui_types::PXUI_ERR_STALE const i32 @HEAD
pxui_types::PXUI_ERR_STATE const i32 @HEAD
pxui_types::PXUI_ERR_VERSION const i32 @U5.9
pxui_types::Pass impl Clone @HEAD
pxui_types::Pass impl Copy @HEAD
pxui_types::Pass impl Debug @HEAD
pxui_types::Pass impl Default @HEAD
pxui_types::Pass struct @HEAD
pxui_types::Pass::first_layer field u32 @HEAD
pxui_types::Pass::height_px field u32 @HEAD
pxui_types::Pass::layer_count field u32 @HEAD
pxui_types::Pass::mtl_command_buffer field *mut c_void @HEAD
pxui_types::Pass::mtl_render_encoder field *mut c_void @HEAD
pxui_types::Pass::scale field f32 @HEAD
pxui_types::Pass::size field u32 @HEAD
pxui_types::Pass::split field Option<PassSplit> @HEAD
pxui_types::Pass::split_user field *mut c_void @HEAD
pxui_types::Pass::vk_command_buffer field *mut c_void @HEAD
pxui_types::Pass::width_px field u32 @HEAD
pxui_types::PassRecord type = extern "C" fn(*mut c_void, *const Image) -> i32 @HEAD
pxui_types::PassSplit type = extern "C" fn(*mut c_void, PassRecord, *mut c_void, *mut Pass) -> i32 @HEAD
pxui_types::PxuiBox impl Clone @HEAD
pxui_types::PxuiBox impl Copy @HEAD
pxui_types::PxuiBox impl Debug @HEAD
pxui_types::PxuiBox impl Default @HEAD
pxui_types::PxuiBox struct @HEAD
pxui_types::PxuiBox::box_key field i64 @HEAD
pxui_types::PxuiBox::box_slot field u64 @HEAD
pxui_types::PxuiBox::generation field u32 @HEAD
pxui_types::PxuiBox::index field u64 @HEAD
pxui_types::PxuiBox::ui field u32 @U5.2
pxui_types::PxuiPanel impl Clone @HEAD
pxui_types::PxuiPanel impl Copy @HEAD
pxui_types::PxuiPanel impl Debug @HEAD
pxui_types::PxuiPanel impl Default @HEAD
pxui_types::PxuiPanel struct @HEAD
pxui_types::PxuiPanel::has_height field u8 @HEAD
-pxui_types::PxuiPanel::has_look field u8 @U4b.6
pxui_types::PxuiPanel::has_max_height field u8 @HEAD
pxui_types::PxuiPanel::has_stroke field u8 @HEAD
pxui_types::PxuiPanel::height field f32 @HEAD
-pxui_types::PxuiPanel::look field PxuiLookDesc @U4b.6
pxui_types::PxuiPanel::look field *const PxuiLookDesc @U4b.6
pxui_types::PxuiPanel::max_height field f32 @HEAD
pxui_types::PxuiPanel::padding field i64 @HEAD
-pxui_types::PxuiPanel::reserved field [u8; 3] @U4b.6
pxui_types::PxuiPanel::reserved field [u8; 4] @U4b.6
pxui_types::PxuiPanel::size field u32 @HEAD
pxui_types::PxuiPanel::stroke field Rgba @HEAD
pxui_types::PxuiPanel::width field f32 @HEAD
pxui_types::PxuiPanel::window field u8 @HEAD
pxui_types::PxuiPanel::x field f32 @HEAD
pxui_types::PxuiPanel::y field f32 @HEAD
pxui_types::PxuiTargetDesc impl Clone @HEAD
pxui_types::PxuiTargetDesc impl Copy @HEAD
pxui_types::PxuiTargetDesc impl Debug @HEAD
pxui_types::PxuiTargetDesc impl Default @HEAD
pxui_types::PxuiTargetDesc struct @HEAD
pxui_types::PxuiTargetDesc::color_height field u32 @HEAD
pxui_types::PxuiTargetDesc::color_width field u32 @HEAD
pxui_types::PxuiTargetDesc::format field u32 @HEAD
pxui_types::PxuiTargetDesc::mask_height field u32 @HEAD
pxui_types::PxuiTargetDesc::mask_width field u32 @HEAD
pxui_types::PxuiTargetDesc::reserved field [u8; 3] @HEAD
pxui_types::PxuiTargetDesc::reserved_tail field u32 @HEAD
pxui_types::PxuiTargetDesc::samples field u8 @HEAD
pxui_types::PxuiTargetDesc::size field u32 @HEAD
pxui_types::PxuiTextInputOptions impl Clone @HEAD
pxui_types::PxuiTextInputOptions impl Copy @HEAD
pxui_types::PxuiTextInputOptions impl Debug @HEAD
pxui_types::PxuiTextInputOptions impl Default @HEAD
pxui_types::PxuiTextInputOptions impl Eq @HEAD
pxui_types::PxuiTextInputOptions impl From<TextTraits> @HEAD
pxui_types::PxuiTextInputOptions impl PartialEq @HEAD
pxui_types::PxuiTextInputOptions impl StructuralPartialEq @HEAD
pxui_types::PxuiTextInputOptions struct @HEAD
pxui_types::PxuiTextInputOptions::action field u32 @HEAD
pxui_types::PxuiTextInputOptions::autocorrect field u32 @HEAD
pxui_types::PxuiTextInputOptions::capitalization field u32 @HEAD
pxui_types::PxuiTextInputOptions::size field u32 @U5.4
pxui_types::PxuiUiDesc impl Clone @HEAD
pxui_types::PxuiUiDesc impl Copy @HEAD
pxui_types::PxuiUiDesc impl Debug @HEAD
pxui_types::PxuiUiDesc impl Default @HEAD
pxui_types::PxuiUiDesc struct @HEAD
pxui_types::PxuiUiDesc::abi_built field u32 @HEAD
pxui_types::PxuiUiDesc::font_size field i32 @HEAD
pxui_types::PxuiUiDesc::platform field u32 @HEAD
pxui_types::PxuiUiDesc::size field u32 @HEAD
pxui_types::ReturnKey enum @HEAD
pxui_types::ReturnKey impl Clone @HEAD
pxui_types::ReturnKey impl Copy @HEAD
pxui_types::ReturnKey impl Debug @HEAD
pxui_types::ReturnKey impl Default @HEAD
pxui_types::ReturnKey impl Eq @HEAD
pxui_types::ReturnKey impl PartialEq @HEAD
pxui_types::ReturnKey impl StructuralPartialEq @HEAD
pxui_types::ReturnKey::Done variant = 1 @HEAD
pxui_types::ReturnKey::Return variant = 0 @HEAD
pxui_types::ReturnKey::Search variant = 2 @HEAD
pxui_types::Ring impl Clone @HEAD
pxui_types::Ring impl Copy @HEAD
pxui_types::Ring impl Debug @HEAD
pxui_types::Ring impl PartialEq @HEAD
pxui_types::Ring impl StructuralPartialEq @HEAD
pxui_types::Ring struct @HEAD
pxui_types::Ring::NONE const Ring @HEAD
pxui_types::Ring::color field Rgba @HEAD
pxui_types::Ring::color fn const (self, color: Rgba) -> Ring @HEAD
pxui_types::Ring::sides field u8 @HEAD
pxui_types::Ring::sides fn const (self, sides: u8) -> Ring @HEAD
pxui_types::Ring::width field f32 @HEAD
pxui_types::Ring::width fn const (self, points: f32) -> Ring @HEAD
pxui_types::Shadow impl Clone @HEAD
pxui_types::Shadow impl Copy @HEAD
pxui_types::Shadow impl Debug @HEAD
pxui_types::Shadow impl PartialEq @HEAD
pxui_types::Shadow impl StructuralPartialEq @HEAD
pxui_types::Shadow struct @HEAD
pxui_types::Shadow::NONE const Shadow @HEAD
pxui_types::Shadow::blur field f32 @HEAD
pxui_types::Shadow::blur fn const (self, points: f32) -> Shadow @HEAD
pxui_types::Shadow::color field Rgba @HEAD
pxui_types::Shadow::color fn const (self, color: Rgba) -> Shadow @HEAD
pxui_types::Shadow::inner field bool @HEAD
pxui_types::Shadow::inner fn const (self) -> Shadow @HEAD
pxui_types::Shadow::offset field [f32; 2] @HEAD
pxui_types::Shadow::offset fn const (self, x: f32, y: f32) -> Shadow @HEAD
pxui_types::Shadow::spread field f32 @HEAD
pxui_types::Shadow::spread fn const (self, points: f32) -> Shadow @HEAD
pxui_types::Size enum @HEAD
pxui_types::Size impl Clone @HEAD
pxui_types::Size impl Copy @HEAD
pxui_types::Size impl Debug @HEAD
pxui_types::Size::Fit variant @HEAD
pxui_types::Size::Grow variant @HEAD
pxui_types::Size::Pct variant (f32) @HEAD
pxui_types::Size::Px variant (f32) @HEAD
pxui_types::Size::Rel variant (extern "C" fn(f32) -> f32) @HEAD
pxui_types::Size::is_grow fn (&self) -> bool @HEAD
pxui_types::Span impl Clone @HEAD
pxui_types::Span impl Copy @HEAD
pxui_types::Span impl Debug @HEAD
pxui_types::Span struct @HEAD
pxui_types::Span::EMPTY const Span @HEAD
pxui_types::Span::at field *mut Instance @HEAD
pxui_types::Span::clip field [i16; 4] @HEAD
pxui_types::Span::clip_radius_f16 field u16 @HEAD
pxui_types::Span::end field *mut Instance @HEAD
pxui_types::Span::room fn (&self) -> usize @HEAD
pxui_types::Span::size field u32 @HEAD
pxui_types::Span::view field u16 @HEAD
pxui_types::Span::write fn unsafe (&mut self, instance: Instance) -> bool @HEAD
pxui_types::State impl Clone @HEAD
pxui_types::State impl Copy @HEAD
pxui_types::State impl Debug @HEAD
pxui_types::State impl Default @HEAD
pxui_types::State impl Eq @HEAD
pxui_types::State impl From<PxuiStateDesc> @U4b
pxui_types::State impl PartialEq @HEAD
pxui_types::State impl StructuralPartialEq @HEAD
pxui_types::State struct @HEAD
pxui_types::State::checked field bool @HEAD
pxui_types::State::checked fn const (self, checked: bool) -> State @U1b.5
pxui_types::State::disabled field bool @HEAD
pxui_types::State::disabled fn const (self, disabled: bool) -> State @U1b.5
pxui_types::State::focused field bool @HEAD
pxui_types::State::focused fn const (self, focused: bool) -> State @U1b.5
pxui_types::State::hovered field bool @HEAD
pxui_types::State::hovered fn const (self, hovered: bool) -> State @U1b.5
pxui_types::State::pressed field bool @HEAD
pxui_types::State::pressed fn const (self, pressed: bool) -> State @U1b.5
pxui_types::States impl Clone @HEAD
pxui_types::States impl Copy @HEAD
pxui_types::States impl Debug @HEAD
pxui_types::States impl PartialEq @HEAD
pxui_types::States impl StructuralPartialEq @HEAD
pxui_types::States struct @HEAD
-pxui_types::States::all fn const (look: Look) -> States @U1b.4
pxui_types::States::checked field Look @HEAD
pxui_types::States::checked fn const (self, look: Look) -> States @HEAD
pxui_types::States::disabled field Look @HEAD
pxui_types::States::disabled fn const (self, look: Look) -> States @HEAD
pxui_types::States::focus field Look @HEAD
pxui_types::States::focus fn const (self, look: Look) -> States @HEAD
pxui_types::States::hover field Look @HEAD
pxui_types::States::hover fn const (self, look: Look) -> States @HEAD
pxui_types::States::idle field Look @HEAD
pxui_types::States::idle fn const (self, look: Look) -> States @HEAD
pxui_types::States::of fn const (look: Look) -> States @U1b.4
pxui_types::States::pick fn (&self, s: State) -> &Look @U2b.4.20
pxui_types::States::pressed field Look @HEAD
pxui_types::States::pressed fn const (self, look: Look) -> States @HEAD
pxui_types::Str impl Clone @HEAD
pxui_types::Str impl Copy @HEAD
pxui_types::Str impl Debug @HEAD
pxui_types::Str impl Default @HEAD
pxui_types::Str impl From<&'static str> @U1b.5
pxui_types::Str struct @HEAD
pxui_types::Str::len field usize @HEAD
pxui_types::Str::ptr field *const u8 @HEAD
pxui_types::TextGlyph impl Clone @HEAD
pxui_types::TextGlyph impl Copy @HEAD
pxui_types::TextGlyph impl Debug @HEAD
pxui_types::TextGlyph impl Default @HEAD
pxui_types::TextGlyph impl PartialEq @HEAD
pxui_types::TextGlyph impl StructuralPartialEq @HEAD
pxui_types::TextGlyph struct @HEAD
pxui_types::TextGlyph::atlas field u32 @HEAD
pxui_types::TextGlyph::rect field [f32; 4] @HEAD
pxui_types::TextGlyph::reserved field u32 @HEAD
pxui_types::TextGlyph::uv field [f32; 4] @HEAD
pxui_types::TextMetrics impl Clone @HEAD
pxui_types::TextMetrics impl Copy @HEAD
pxui_types::TextMetrics impl Debug @HEAD
pxui_types::TextMetrics impl Default @HEAD
pxui_types::TextMetrics impl PartialEq @HEAD
pxui_types::TextMetrics impl StructuralPartialEq @HEAD
pxui_types::TextMetrics struct @HEAD
pxui_types::TextMetrics::baseline field f32 @HEAD
pxui_types::TextMetrics::height field f32 @HEAD
pxui_types::TextMetrics::lines field u32 @HEAD
pxui_types::TextMetrics::size field u32 @U5.4
pxui_types::TextMetrics::width field f32 @HEAD
pxui_types::TextSpan impl Clone @HEAD
pxui_types::TextSpan impl Copy @HEAD
pxui_types::TextSpan impl Debug @HEAD
pxui_types::TextSpan impl Default @HEAD
pxui_types::TextSpan impl Eq @HEAD
pxui_types::TextSpan impl PartialEq @HEAD
pxui_types::TextSpan impl StructuralPartialEq @HEAD
pxui_types::TextSpan struct @HEAD
pxui_types::TextSpan::BACKGROUND const u16 @HEAD
pxui_types::TextSpan::COLOR const u16 @HEAD
pxui_types::TextSpan::UNDERLINE const u16 @HEAD
pxui_types::TextSpan::background field Rgba @HEAD
pxui_types::TextSpan::background fn const (self, background: Rgba) -> TextSpan @U1b.5
pxui_types::TextSpan::color field Rgba @HEAD
pxui_types::TextSpan::color fn const (self, color: Rgba) -> TextSpan @U1b.5
pxui_types::TextSpan::end field u32 @HEAD
pxui_types::TextSpan::flags field u16 @HEAD
pxui_types::TextSpan::new fn const (start: u32, end: u32) -> TextSpan @U1b.5
pxui_types::TextSpan::start field u32 @HEAD
pxui_types::TextSpan::underline field Rgba @HEAD
pxui_types::TextSpan::underline fn const (self, underline: Rgba) -> TextSpan @U1b.5
pxui_types::TextSpan::weight field u16 @HEAD
pxui_types::TextSpan::weight fn const (self, weight: u16) -> TextSpan @U1b.5
pxui_types::TextSpans impl Clone @HEAD
pxui_types::TextSpans impl Copy @HEAD
pxui_types::TextSpans impl Debug @HEAD
pxui_types::TextSpans impl Default @HEAD
pxui_types::TextSpans struct <'a> @U5.4
pxui_types::TextSpans::MAX const usize @HEAD
pxui_types::TextSpans::new fn const (spans: &'a [TextSpan]) -> Self @U5.4
pxui_types::TextSpans::reserved field u32 @U5.4
pxui_types::TextSpans::size field u32 @HEAD
pxui_types::TextSpans::spans field Strided<TextSpan> @U5.4
pxui_types::TextTraits impl Clone @HEAD
pxui_types::TextTraits impl Copy @HEAD
pxui_types::TextTraits impl Debug @HEAD
pxui_types::TextTraits impl Default @HEAD
pxui_types::TextTraits impl Eq @HEAD
pxui_types::TextTraits impl From<TextTraits> @HEAD
pxui_types::TextTraits impl PartialEq @HEAD
pxui_types::TextTraits impl StructuralPartialEq @HEAD
pxui_types::TextTraits struct @HEAD
pxui_types::TextTraits::DEFAULT const Self @HEAD
pxui_types::TextTraits::SEARCH const Self @HEAD
pxui_types::TextTraits::action fn const (self, action: ReturnKey) -> Self @HEAD
pxui_types::TextTraits::autocorrect fn const (self, autocorrect: bool) -> Self @HEAD
pxui_types::TextTraits::autocorrects fn const (&self) -> bool @HEAD
pxui_types::TextTraits::capitalization fn const (self, capitalization: Capitalize) -> Self @HEAD
pxui_types::TextTraits::capitalizes fn const (&self) -> Capitalize @HEAD
pxui_types::TextTraits::from_numbers fn const (autocorrect: u32, capitalization: u32, action: u32) -> Option<TextTraits> @HEAD
pxui_types::TextTraits::return_key fn const (&self) -> ReturnKey @HEAD
pxui_types::Tokens impl Clone @HEAD
pxui_types::Tokens impl Copy @HEAD
pxui_types::Tokens impl Debug @HEAD
pxui_types::Tokens impl Default @HEAD
pxui_types::Tokens impl From<ThemeTokens> @HEAD
pxui_types::Tokens impl From<Tokens> @HEAD
pxui_types::Tokens impl PartialEq @HEAD
pxui_types::Tokens impl StructuralPartialEq @HEAD
pxui_types::Tokens struct @HEAD
pxui_types::Tokens::DEFAULT const Tokens @HEAD
pxui_types::Tokens::accent field Rgba @HEAD
pxui_types::Tokens::border fn const (&self) -> Rgba @HEAD
pxui_types::Tokens::control field Rgba @HEAD
pxui_types::Tokens::dark field bool @U5.6
pxui_types::Tokens::density field f32 @HEAD
pxui_types::Tokens::display_size field f32 @U1b.3
-pxui_types::Tokens::display_size field i64 @U1b.3
pxui_types::Tokens::edge fn const (&self) -> Rgba @HEAD
pxui_types::Tokens::faint_border fn const (&self) -> Rgba @HEAD
pxui_types::Tokens::font_size field f32 @U1b.3
-pxui_types::Tokens::font_size field i64 @U1b.3
pxui_types::Tokens::foreground field Rgba @HEAD
pxui_types::Tokens::from_ffi fn (value: ThemeTokens) -> Self @HEAD
pxui_types::Tokens::ground fn (&self) -> Rgba @HEAD
pxui_types::Tokens::hover_fill fn (&self) -> Rgba @HEAD
pxui_types::Tokens::ink_2 fn (&self) -> Rgba @HEAD
pxui_types::Tokens::ink_3 fn (&self) -> Rgba @HEAD
pxui_types::Tokens::ink_ground field OptColor @HEAD
pxui_types::Tokens::input field Rgba @HEAD
pxui_types::Tokens::invalid field Rgba @HEAD
pxui_types::Tokens::is_dark fn const (&self) -> bool @HEAD
pxui_types::Tokens::label_size field f32 @U1b.3
-pxui_types::Tokens::label_size field i64 @U1b.3
pxui_types::Tokens::motion field Motion @HEAD
pxui_types::Tokens::moves fn (&self) -> bool @HEAD
pxui_types::Tokens::panel field Rgba @HEAD
pxui_types::Tokens::pressed_fill fn (&self) -> Rgba @HEAD
pxui_types::Tokens::radius field [f32; 3] @HEAD
pxui_types::Tokens::reduced_motion field bool @HEAD
pxui_types::Tokens::space field [f32; 4] @HEAD
pxui_types::Tokens::tint fn const (&self) -> Rgba @HEAD
pxui_types::Tokens::title_size field f32 @U1b.3
-pxui_types::Tokens::title_size field i64 @U1b.3
pxui_types::Tokens::track field Rgba @HEAD
pxui_types::Tokens::transition fn (&self, from: State, to: State) -> Curve @HEAD
pxui_types::Touch impl Clone @HEAD
pxui_types::Touch impl Copy @HEAD
pxui_types::Touch impl Debug @HEAD
pxui_types::Touch impl Default @HEAD
pxui_types::Touch impl PartialEq @HEAD
pxui_types::Touch impl StructuralPartialEq @HEAD
pxui_types::Touch struct @HEAD
pxui_types::Touch::device field u64 @HEAD
pxui_types::Touch::finger field u64 @HEAD
pxui_types::Touch::new fn const (device: u64, finger: u64, at: [f64; 2], time: f64, pressure: f32, phase: u32) -> Touch @U1b.5
pxui_types::Touch::phase field u32 @HEAD
pxui_types::Touch::pressure field f32 @HEAD
pxui_types::Touch::reserved field u32 @U5.4
pxui_types::Touch::size field u32 @U5.4
pxui_types::Touch::time field f64 @HEAD
pxui_types::Touch::x field f64 @HEAD
pxui_types::Touch::y field f64 @HEAD
pxui_types::View impl Clone @HEAD
pxui_types::View impl Copy @HEAD
pxui_types::View impl Debug @HEAD
pxui_types::View impl PartialEq @HEAD
pxui_types::View impl StructuralPartialEq @HEAD
pxui_types::View struct @HEAD
pxui_types::View::IDENTITY const Self @HEAD
pxui_types::View::clip field [f32; 4] @HEAD
pxui_types::View::offset field [f32; 2] @HEAD
pxui_types::View::opacity field f32 @HEAD
pxui_types::View::pivot field [f32; 2] @HEAD
pxui_types::View::rotation fn (radians: f32, pivot: [f32; 2]) -> Self @HEAD
pxui_types::View::scale field f32 @HEAD
pxui_types::View::transform field [f32; 4] @HEAD
pxui_types::a11y mod @HEAD
pxui_types::a11y::Role enum @HEAD
pxui_types::a11y::Role impl Clone @HEAD
pxui_types::a11y::Role impl Copy @HEAD
pxui_types::a11y::Role impl Debug @HEAD
pxui_types::a11y::Role impl Eq @HEAD
pxui_types::a11y::Role impl Hash @HEAD
pxui_types::a11y::Role impl PartialEq @HEAD
pxui_types::a11y::Role impl StructuralPartialEq @HEAD
pxui_types::a11y::Role::Button variant @HEAD
pxui_types::a11y::Role::Canvas variant @HEAD
pxui_types::a11y::Role::Cell variant @HEAD
pxui_types::a11y::Role::CheckBox variant @HEAD
pxui_types::a11y::Role::ColumnHeader variant @HEAD
pxui_types::a11y::Role::ComboBox variant @HEAD
pxui_types::a11y::Role::Dialog variant @HEAD
pxui_types::a11y::Role::Group variant @HEAD
pxui_types::a11y::Role::Heading variant @HEAD
pxui_types::a11y::Role::Image variant @HEAD
pxui_types::a11y::Role::Label variant @HEAD
pxui_types::a11y::Role::Link variant @HEAD
pxui_types::a11y::Role::List variant @HEAD
pxui_types::a11y::Role::ListItem variant @HEAD
pxui_types::a11y::Role::Menu variant @HEAD
pxui_types::a11y::Role::MenuBar variant @HEAD
pxui_types::a11y::Role::MenuItem variant @HEAD
pxui_types::a11y::Role::Panel variant @HEAD
pxui_types::a11y::Role::Paragraph variant @HEAD
pxui_types::a11y::Role::Progress variant @HEAD
pxui_types::a11y::Role::Radio variant @HEAD
pxui_types::a11y::Role::Row variant @HEAD
pxui_types::a11y::Role::ScrollArea variant @HEAD
pxui_types::a11y::Role::ScrollBar variant @HEAD
pxui_types::a11y::Role::Separator variant @HEAD
pxui_types::a11y::Role::Slider variant @HEAD
pxui_types::a11y::Role::SpinButton variant @HEAD
pxui_types::a11y::Role::Splitter variant @HEAD
pxui_types::a11y::Role::StatusBar variant @HEAD
pxui_types::a11y::Role::Tab variant @HEAD
pxui_types::a11y::Role::TabList variant @HEAD
pxui_types::a11y::Role::TabPanel variant @HEAD
pxui_types::a11y::Role::Table variant @HEAD
pxui_types::a11y::Role::TextArea variant @HEAD
pxui_types::a11y::Role::TextField variant @HEAD
pxui_types::a11y::Role::Toggle variant @HEAD
pxui_types::a11y::Role::Toolbar variant @HEAD
pxui_types::a11y::Role::Tooltip variant @HEAD
pxui_types::a11y::Role::Tree variant @HEAD
pxui_types::a11y::Role::TreeItem variant @HEAD
pxui_types::a11y::Role::Unknown variant @HEAD
pxui_types::a11y::Role::Window variant @HEAD
pxui_types::a11y::state mod @HEAD
pxui_types::a11y::state::ACTIVE const u32 @HEAD
pxui_types::a11y::state::BUSY const u32 @HEAD
pxui_types::a11y::state::CHECKABLE const u32 @HEAD
pxui_types::a11y::state::CHECKED const u32 @HEAD
pxui_types::a11y::state::DEFAULT const u32 @HEAD
pxui_types::a11y::state::EDITABLE const u32 @HEAD
pxui_types::a11y::state::ENABLED const u32 @HEAD
pxui_types::a11y::state::EXPANDABLE const u32 @HEAD
pxui_types::a11y::state::EXPANDED const u32 @HEAD
pxui_types::a11y::state::FOCUSABLE const u32 @HEAD
pxui_types::a11y::state::FOCUSED const u32 @HEAD
pxui_types::a11y::state::HORIZONTAL const u32 @HEAD
pxui_types::a11y::state::MIXED const u32 @HEAD
pxui_types::a11y::state::MODAL const u32 @HEAD
pxui_types::a11y::state::MULTILINE const u32 @HEAD
pxui_types::a11y::state::PRESSED const u32 @HEAD
pxui_types::a11y::state::READ_ONLY const u32 @HEAD
pxui_types::a11y::state::SELECTABLE const u32 @HEAD
pxui_types::a11y::state::SELECTED const u32 @HEAD
pxui_types::a11y::state::VERTICAL const u32 @HEAD
pxui_types::a11y::state::VISIBLE const u32 @HEAD
pxui_types::body_kinds macro @HEAD
pxui_types::draw mod @HEAD
pxui_types::draw::ATLAS const u32 @HEAD
pxui_types::draw::Break impl Clone @HEAD
pxui_types::draw::Break impl Copy @HEAD
pxui_types::draw::Break impl Debug @HEAD
pxui_types::draw::Break impl Eq @HEAD
pxui_types::draw::Break impl PartialEq @HEAD
pxui_types::draw::Break impl StructuralPartialEq @HEAD
pxui_types::draw::Break struct @HEAD
pxui_types::draw::Break::at_instance field u32 @HEAD
pxui_types::draw::Break::image field u32 @HEAD
pxui_types::draw::Fit enum @HEAD
pxui_types::draw::Fit impl Clone @HEAD
pxui_types::draw::Fit impl Copy @HEAD
pxui_types::draw::Fit impl Debug @HEAD
pxui_types::draw::Fit impl Eq @HEAD
pxui_types::draw::Fit impl PartialEq @HEAD
pxui_types::draw::Fit impl StructuralPartialEq @HEAD
pxui_types::draw::Fit::Contain variant @HEAD
pxui_types::draw::Fit::Cover variant @HEAD
pxui_types::draw::Fit::Fill variant @HEAD
pxui_types::draw::Fit::None variant @HEAD
pxui_types::draw::Instance impl Clone @HEAD
pxui_types::draw::Instance impl Copy @HEAD
pxui_types::draw::Instance impl Debug @HEAD
pxui_types::draw::Instance impl PartialEq @HEAD
pxui_types::draw::Instance impl StructuralPartialEq @HEAD
pxui_types::draw::Instance struct @HEAD
pxui_types::draw::Instance::DEFAULT const Instance @HEAD
pxui_types::draw::Instance::arc fn (center: [f32; 2], radius: f32, start: f32, sweep: f32, width: f32, color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::aux field [u32; 2] @HEAD
pxui_types::draw::Instance::border fn (self, width: f32, color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::clip field [i16; 4] @HEAD
pxui_types::draw::Instance::clip fn (self, rect: [i16; 4], radius: f32) -> Self @HEAD
pxui_types::draw::Instance::clip_radius field u16 @HEAD
pxui_types::draw::Instance::color field [u8; 4] @HEAD
pxui_types::draw::Instance::color2 field [u8; 4] @HEAD
pxui_types::draw::Instance::conic fn (self, center: [f32; 2], start_turns: f32, color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::curve fn (a: [f32; 2], control: [f32; 2], b: [f32; 2], width: f32, color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::dash fn (self, on: f32, off: f32, phase: f32) -> Self @HEAD
pxui_types::draw::Instance::extra field [u32; 2] @HEAD
pxui_types::draw::Instance::flags field u8 @HEAD
pxui_types::draw::Instance::grid fn (rect: [f32; 4], origin: [f32; 2], spacing: f32, dot: f32, color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::image fn (rect: [f32; 4], uv: [f32; 4], tint: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::kind field u8 @HEAD
pxui_types::draw::Instance::light fn (self, extra: [u32; 2]) -> Self @HEAD
pxui_types::draw::Instance::light_extra fn (ppem: f32, scale: f32) -> [u32; 2] @HEAD
pxui_types::draw::Instance::line fn (a: [f32; 2], b: [f32; 2], width: f32, color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::linear fn (self, from: [f32; 2], to: [f32; 2], color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::mask fn (rect: [f32; 4], uv: [f32; 4], color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::radial fn (self, center: [f32; 2], radius: [f32; 2], color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::radii field [u16; 4] @HEAD
pxui_types::draw::Instance::radii fn (self, radii: [f32; 4]) -> Self @HEAD
pxui_types::draw::Instance::ramp fn (self, row: u32) -> Self @HEAD
pxui_types::draw::Instance::rect field [f32; 4] @HEAD
pxui_types::draw::Instance::rect fn (rect: [f32; 4], color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::round_caps fn (self) -> Self @HEAD
pxui_types::draw::Instance::shadow fn (rect: [f32; 4], radii: [f32; 4], color: [u8; 4], offset: [f32; 2], sigma: f32, spread: f32, inner: bool) -> Self @HEAD
pxui_types::draw::Instance::sides fn (self, sides: u8) -> Self @HEAD
pxui_types::draw::Instance::triangle fn (a: [f32; 2], b: [f32; 2], c: [f32; 2], color: [u8; 4]) -> Self @HEAD
pxui_types::draw::Instance::view field u16 @HEAD
pxui_types::draw::Instance::view fn (self, view: u16) -> Self @HEAD
pxui_types::draw::Instance::width field u16 @HEAD
pxui_types::draw::NO_CLIP const [i16; 4] @HEAD
pxui_types::draw::add_range fn (ranges: &mut Vec<(u32, u32)>, a: u32, b: u32, cap: usize) @HEAD
pxui_types::draw::borders fn (rect: [f32; 4], radii: [f32; 4], widths: [f32; 4], colors: [[u8; 4]; 4]) -> ([Instance; 4], usize) @HEAD
pxui_types::draw::f16 fn (x: f32) -> u16 @HEAD
pxui_types::draw::fit fn (mode: Fit, image: [f32; 2], bounds: [f32; 4], uv: [f32; 4]) -> ([f32; 4], [f32; 4]) @HEAD
pxui_types::draw::flags mod @HEAD
pxui_types::draw::flags::image mod @HEAD
pxui_types::draw::flags::image::GLYPH_PAGE_2 const u8 @HEAD
pxui_types::draw::flags::image::SMOOTH const u8 @HEAD
pxui_types::draw::flags::mask mod @HEAD
pxui_types::draw::flags::mask::GLYPH_PAGE_2 const u8 @HEAD
pxui_types::draw::flags::mask::LIGHT const u8 @HEAD
pxui_types::draw::flags::rect mod @HEAD
pxui_types::draw::flags::rect::AREA const u8 @HEAD
pxui_types::draw::flags::rect::BOTTOM const u8 @HEAD
pxui_types::draw::flags::rect::CONIC const u8 @HEAD
pxui_types::draw::flags::rect::LEFT const u8 @HEAD
pxui_types::draw::flags::rect::LINEAR const u8 @HEAD
pxui_types::draw::flags::rect::RADIAL const u8 @HEAD
pxui_types::draw::flags::rect::RIGHT const u8 @HEAD
pxui_types::draw::flags::rect::TOP const u8 @HEAD
pxui_types::draw::flags::shadow mod @HEAD
pxui_types::draw::flags::shadow::INNER const u8 @HEAD
pxui_types::draw::flags::stroke mod @HEAD
pxui_types::draw::flags::stroke::ROUND const u8 @HEAD
pxui_types::draw::kind mod @HEAD
pxui_types::draw::kind::ARC const u8 @HEAD
pxui_types::draw::kind::BORDER const u8 @HEAD
pxui_types::draw::kind::CURVE const u8 @HEAD
pxui_types::draw::kind::GRID const u8 @HEAD
pxui_types::draw::kind::IMAGE const u8 @HEAD
pxui_types::draw::kind::LINE const u8 @HEAD
pxui_types::draw::kind::MASK const u8 @HEAD
pxui_types::draw::kind::RECT const u8 @HEAD
pxui_types::draw::kind::SHADOW const u8 @HEAD
pxui_types::draw::kind::TRIANGLE const u8 @HEAD
pxui_types::draw::nine_slice fn (rect: [f32; 4], uv: [f32; 4], image: [f32; 2], inset: [f32; 4], tint: [u8; 4]) -> [Instance; 9] @HEAD
pxui_types::feature mod @HEAD
pxui_types::feature::A11Y_BRIDGE const u32 @HEAD
pxui_types::feature::EXTENSION_SET const u32 @HEAD
pxui_types::feature::METAL const u32 @HEAD
pxui_types::feature::VULKAN const u32 @HEAD
pxui_types::ffi mod @HEAD
pxui_types::ffi::A11yAction impl Clone @HEAD
pxui_types::ffi::A11yAction impl Copy @HEAD
pxui_types::ffi::A11yAction impl Debug @HEAD
pxui_types::ffi::A11yAction impl Default @HEAD
pxui_types::ffi::A11yAction struct @HEAD
pxui_types::ffi::A11yAction::action_code field u32 @HEAD
pxui_types::ffi::A11yAction::end field u32 @HEAD
pxui_types::ffi::A11yAction::id field u64 @HEAD
pxui_types::ffi::A11yAction::payload field f64 @HEAD
pxui_types::ffi::A11yAction::reserved field u32 @HEAD
pxui_types::ffi::A11yAction::surface field u64 @U5.9
pxui_types::ffi::A11yAction::size field u32 @U5.4
pxui_types::ffi::A11yActionSpan type = Strided<A11yAction> @U5.4
pxui_types::ffi::A11yNode impl Clone @HEAD
pxui_types::ffi::A11yNode impl Copy @HEAD
pxui_types::ffi::A11yNode impl Debug @HEAD
pxui_types::ffi::A11yNode impl Default @HEAD
pxui_types::ffi::A11yNode struct @HEAD
pxui_types::ffi::A11yNode::actions field u32 @HEAD
pxui_types::ffi::A11yNode::child_count field u32 @HEAD
pxui_types::ffi::A11yNode::description_end field u32 @HEAD
pxui_types::ffi::A11yNode::description_start field u32 @HEAD
pxui_types::ffi::A11yNode::first_child field u32 @HEAD
pxui_types::ffi::A11yNode::id field u64 @HEAD
pxui_types::ffi::A11yNode::is_role fn (&self, role: Role) -> bool @HEAD
pxui_types::ffi::A11yNode::last_child field u32 @HEAD
pxui_types::ffi::A11yNode::name_end field u32 @HEAD
pxui_types::ffi::A11yNode::name_start field u32 @HEAD
pxui_types::ffi::A11yNode::next_sibling field u32 @HEAD
pxui_types::ffi::A11yNode::parent field u32 @HEAD
pxui_types::ffi::A11yNode::rect field Rect @HEAD
pxui_types::ffi::A11yNode::role field u32 @HEAD
pxui_types::ffi::A11yNode::selection field [u32; 2] @HEAD
pxui_types::ffi::A11yNode::size field u32 @U5.4
pxui_types::ffi::A11yNode::states field u32 @HEAD
pxui_types::ffi::A11yNode::value field A11yValue @HEAD
pxui_types::ffi::A11yTree impl Clone @HEAD
pxui_types::ffi::A11yTree impl Copy @HEAD
pxui_types::ffi::A11yTree impl Debug @HEAD
pxui_types::ffi::A11yTree struct @HEAD
pxui_types::ffi::A11yTree impl Default @U5.5
pxui_types::ffi::A11yTree::actions field Strided<A11yAction> @U5.4
pxui_types::ffi::A11yTree::child_count field u32 @HEAD
pxui_types::ffi::A11yTree::first_child field u32 @HEAD
pxui_types::ffi::A11yTree::nodes field Strided<A11yNode> @U5.4
pxui_types::ffi::A11yTree::reserved field u32 @U5.5
pxui_types::ffi::A11yTree::revision field u64 @HEAD
pxui_types::ffi::A11yTree::size field u32 @U5.5
pxui_types::ffi::A11yTree::strings field Str @HEAD
pxui_types::ffi::A11yValue impl Clone @HEAD
pxui_types::ffi::A11yValue impl Copy @HEAD
pxui_types::ffi::A11yValue impl Debug @HEAD
pxui_types::ffi::A11yValue impl Default @HEAD
pxui_types::ffi::A11yValue struct @HEAD
pxui_types::ffi::A11yValue::max field f64 @HEAD
pxui_types::ffi::A11yValue::min field f64 @HEAD
pxui_types::ffi::A11yValue::now field f64 @HEAD
pxui_types::ffi::A11yValue::reserved field u32 @HEAD
pxui_types::ffi::A11yValue::step field f64 @HEAD
pxui_types::ffi::A11yValue::tag field u32 @HEAD
pxui_types::ffi::A11yValue::text_end field u32 @HEAD
pxui_types::ffi::A11yValue::text_start field u32 @HEAD
pxui_types::ffi::AreaNotify type = Option<extern "C" fn(*mut c_void, i64, i64)> @HEAD
pxui_types::ffi::Damage impl Clone @HEAD
pxui_types::ffi::Damage impl Copy @HEAD
pxui_types::ffi::Damage impl Debug @HEAD
pxui_types::ffi::Damage impl Default @HEAD
pxui_types::ffi::Damage struct @HEAD
pxui_types::ffi::Damage::has_damage field u8 @HEAD
pxui_types::ffi::Damage::rect field Rect @HEAD
pxui_types::ffi::Damage::reserved field [u8; 3] @HEAD
pxui_types::ffi::Input impl Clone @HEAD
pxui_types::ffi::Input impl Copy @HEAD
pxui_types::ffi::Input impl Debug @HEAD
pxui_types::ffi::Input impl Default @HEAD
pxui_types::ffi::Input struct @HEAD
pxui_types::ffi::Input::buttons field Slice<u32> @HEAD
pxui_types::ffi::Input::dt field f64 @HEAD
pxui_types::ffi::Input::events field Strided<Event> @U5.4
pxui_types::ffi::Input::fps field f64 @HEAD
pxui_types::ffi::Input::frame field i64 @HEAD
pxui_types::ffi::Input::keys field Slice<u32> @HEAD
pxui_types::ffi::Input::logical_size field [i64; 2] @HEAD
pxui_types::ffi::Input::mouse_delta field [f64; 2] @HEAD
pxui_types::ffi::Input::pixel_scale field [f64; 2] @HEAD
pxui_types::ffi::Input::pointer field [f64; 2] @HEAD
pxui_types::ffi::Input::size field u32 @HEAD
pxui_types::ffi::Input::tick field i64 @HEAD
pxui_types::ffi::Input::time field f64 @HEAD
pxui_types::ffi::Output impl Clone @HEAD
pxui_types::ffi::Output impl Copy @HEAD
pxui_types::ffi::Output impl Debug @HEAD
pxui_types::ffi::Output impl Default @HEAD
pxui_types::ffi::Output struct @HEAD
pxui_types::ffi::Output::a11y field A11yTree @HEAD
pxui_types::ffi::Output::arranged_nodes field u64 @HEAD
pxui_types::ffi::Output::built_nodes field u64 @HEAD
pxui_types::ffi::Output::damage field Damage @HEAD
pxui_types::ffi::Output::draw_revision field u64 @HEAD
pxui_types::ffi::Output::layers field Strided<PxuiLayerInfo> @U5.4
pxui_types::ffi::Output::platform field PlatformOut @HEAD
pxui_types::ffi::Output::rebuilt_views field u64 @HEAD
pxui_types::ffi::Output::repaint_at field f64 @HEAD
pxui_types::ffi::Output::reserved field u32 @HEAD
pxui_types::ffi::Output::size field u32 @HEAD
pxui_types::ffi::PlatformOut impl Clone @HEAD
pxui_types::ffi::PlatformOut impl Copy @HEAD
pxui_types::ffi::PlatformOut impl Debug @HEAD
pxui_types::ffi::PlatformOut impl Default @HEAD
pxui_types::ffi::PlatformOut struct @HEAD
pxui_types::ffi::PlatformOut::background field Rgba @HEAD
pxui_types::ffi::PlatformOut::clipboard field Str @HEAD
pxui_types::ffi::PlatformOut::cursor field u32 @HEAD
pxui_types::ffi::PlatformOut::has_background field u8 @HEAD
pxui_types::ffi::PlatformOut::has_clipboard field u8 @HEAD
pxui_types::ffi::PlatformOut::has_cursor field u8 @HEAD
pxui_types::ffi::PlatformOut::has_ime field u8 @HEAD
pxui_types::ffi::PlatformOut::ime_caret field i64 @HEAD
pxui_types::ffi::PlatformOut::ime_rect field [i64; 4] @HEAD
pxui_types::ffi::PlatformOut::paste field u8 @HEAD
pxui_types::ffi::PlatformOut::reserved field [u8; 7] @HEAD
pxui_types::ffi::PlatformOut::size field u32 @HEAD
pxui_types::ffi::PxuiA11yAction type = A11yAction @HEAD
pxui_types::ffi::PxuiA11yNode type = A11yNode @HEAD
pxui_types::ffi::PxuiA11yTree type = A11yTree @HEAD
pxui_types::ffi::PxuiA11yValue type = A11yValue @HEAD
pxui_types::ffi::PxuiAnchorDesc impl Clone @HEAD
pxui_types::ffi::PxuiAnchorDesc impl Copy @HEAD
pxui_types::ffi::PxuiAnchorDesc impl Debug @HEAD
pxui_types::ffi::PxuiAnchorDesc impl Default @HEAD
pxui_types::ffi::PxuiAnchorDesc struct @HEAD
pxui_types::ffi::PxuiAnchorDesc::align field u32 @HEAD
pxui_types::ffi::PxuiAnchorDesc::flip field u8 @HEAD
pxui_types::ffi::PxuiAnchorDesc::gap field f32 @HEAD
pxui_types::ffi::PxuiAnchorDesc::has_anchor field u8 @HEAD
pxui_types::ffi::PxuiAnchorDesc::margin field f32 @HEAD
pxui_types::ffi::PxuiAnchorDesc::rect field Rect @HEAD
pxui_types::ffi::PxuiAnchorDesc::reserved field [u8; 2] @U5.4
pxui_types::ffi::PxuiAnchorDesc::side field u32 @HEAD
pxui_types::ffi::PxuiAnchorDesc::size field u32 @U5.4
pxui_types::ffi::PxuiAreaCallbacks impl Clone @HEAD
pxui_types::ffi::PxuiAreaCallbacks impl Copy @HEAD
pxui_types::ffi::PxuiAreaCallbacks impl Debug @HEAD
pxui_types::ffi::PxuiAreaCallbacks struct @HEAD
pxui_types::ffi::PxuiAreaCallbacks::brackets field Option<extern "C" fn(*mut c_void, Str, *mut Slice<PxuiRange>)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::colorize field Option<extern "C" fn(*mut c_void, Str, *mut Slice<PxuiColorSpan>)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::complete field Option<extern "C" fn(*mut c_void, Str, i64, *mut Slice<PxuiCompletion>)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::context_at field Option<extern "C" fn(*mut c_void, Str, i64) -> u32> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::describe field Option<extern "C" fn(*mut c_void, Str, i64, *mut PxuiDescription)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::indent field Option<extern "C" fn(*mut c_void, Str, i64, *mut Str)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::number_at field Option<extern "C" fn(*mut c_void, Str, i64, *mut PxuiMaybeRange)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_caret field Option<extern "C" fn(*mut c_void, i64)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_caret_move field Option<extern "C" fn(*mut c_void, i64)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_click field Option<extern "C" fn(*mut c_void, i64, u8)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_context field Option<extern "C" fn(*mut c_void, f32, f32)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_drop field Option<extern "C" fn(*mut c_void, i64, u8, Str, Str)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_scrub field Option<extern "C" fn(*mut c_void, u32)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_scrub_edit field Option<extern "C" fn(*mut c_void, i64, i64, Str)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::on_select field AreaNotify @HEAD
pxui_types::ffi::PxuiAreaCallbacks::reserved field u32 @HEAD
pxui_types::ffi::PxuiAreaCallbacks::rewrite field Option<extern "C" fn(*mut c_void, Str, i64, *mut PxuiRewrite)> @HEAD
pxui_types::ffi::PxuiAreaCallbacks::size field u32 @HEAD
pxui_types::ffi::PxuiAreaCallbacks::user field *mut c_void @HEAD
pxui_types::ffi::PxuiBadgeConfig impl Clone @HEAD
pxui_types::ffi::PxuiBadgeConfig impl Copy @HEAD
pxui_types::ffi::PxuiBadgeConfig struct @HEAD
pxui_types::ffi::PxuiBadgeConfig::label field Str @HEAD
pxui_types::ffi::PxuiBadgeConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiBadgeConfig::size field u32 @HEAD
pxui_types::ffi::PxuiBadgeConfig::text field Str @HEAD
pxui_types::ffi::PxuiBadgeConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiBodyFn type = Option<extern "C" fn(*mut c_void, *mut c_void, *const c_void)> @HEAD
-pxui_types::ffi::PxuiBoxDesc impl Clone @U2c.3
-pxui_types::ffi::PxuiBoxDesc impl Copy @U2c.3
-pxui_types::ffi::PxuiBoxDesc struct @U2c.3
-pxui_types::ffi::PxuiBoxDesc::at field [f32; 2] @U2c.3
-pxui_types::ffi::PxuiBoxDesc::axis field u32 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::flags field i64 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::h field PxuiSizeDesc @U2c.3
-pxui_types::ffi::PxuiBoxDesc::has_at field u8 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::has_hit_rect field u8 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::hit field Option<extern "C" fn(*mut c_void, Rect) -> Rect> @U2c.3
-pxui_types::ffi::PxuiBoxDesc::hit_rect field Rect @U2c.3
-pxui_types::ffi::PxuiBoxDesc::hit_release field Option<extern "C" fn(*mut c_void)> @U2c.3
-pxui_types::ffi::PxuiBoxDesc::hit_user field *mut c_void @U2c.3
-pxui_types::ffi::PxuiBoxDesc::max_h field f32 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::padding field f32 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::reserved field [u8; 2] @U2c.3
-pxui_types::ffi::PxuiBoxDesc::scroll_step field f32 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::size field u32 @U2c.3
-pxui_types::ffi::PxuiBoxDesc::w field PxuiSizeDesc @U2c.3
pxui_types::ffi::PxuiBuild type = UiBuild @HEAD
pxui_types::ffi::PxuiButtonBuild type = Option<extern "C" fn(*mut c_void, *mut c_void, *const PxuiStateDesc)> @HEAD
pxui_types::ffi::PxuiButtonConfig impl Clone @HEAD
pxui_types::ffi::PxuiButtonConfig impl Copy @HEAD
pxui_types::ffi::PxuiButtonConfig struct @HEAD
pxui_types::ffi::PxuiButtonConfig::accessory field Str @HEAD
pxui_types::ffi::PxuiButtonConfig::accessory_box field PxuiLabelBox @HEAD
pxui_types::ffi::PxuiButtonConfig::accessory_style field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiButtonConfig::at_end field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::axis field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::bare field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::built_in field PxuiSlot @U2b.4.2
pxui_types::ffi::PxuiButtonConfig::centered field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::content_align field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::danger field u8 @U2b.4.35
pxui_types::ffi::PxuiButtonConfig::detail field Str @HEAD
pxui_types::ffi::PxuiButtonConfig::detail_box field PxuiLabelBox @HEAD
pxui_types::ffi::PxuiButtonConfig::detail_style field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiButtonConfig::gap field f32 @HEAD
pxui_types::ffi::PxuiButtonConfig::h field f32 @HEAD
pxui_types::ffi::PxuiButtonConfig::h_kind field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::has_accessory_style field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::has_detail_style field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::has_label field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::icon field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::icon_size field f32 @U2b.7
pxui_types::ffi::PxuiButtonConfig::justify field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::label field Str @HEAD
pxui_types::ffi::PxuiButtonConfig::lead field Str @HEAD
pxui_types::ffi::PxuiButtonConfig::lead_draw field PxuiIconDraw @HEAD
pxui_types::ffi::PxuiButtonConfig::lead_user field *mut c_void @HEAD
pxui_types::ffi::PxuiButtonConfig::lead_word field u32 @U2b.7
pxui_types::ffi::PxuiButtonConfig::line field PxuiBox @HEAD
pxui_types::ffi::PxuiButtonConfig::padding field [f32; 4] @HEAD
pxui_types::ffi::PxuiButtonConfig::reserved field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::reserved2 field [u8; 8] @U2b.4.35
-pxui_types::ffi::PxuiButtonConfig::reserved2 field [u8; 9] @U2b.4.35
pxui_types::ffi::PxuiButtonConfig::reserved3 field u8 @HEAD
pxui_types::ffi::PxuiButtonConfig::reserved4 field [u8; 6] @HEAD
pxui_types::ffi::PxuiButtonConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiButtonConfig::size field u32 @HEAD
pxui_types::ffi::PxuiButtonConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiButtonConfig::states field *const States @HEAD
pxui_types::ffi::PxuiButtonConfig::text_box field PxuiLabelBox @HEAD
pxui_types::ffi::PxuiButtonConfig::text_style field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiButtonConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiButtonConfig::w field f32 @HEAD
pxui_types::ffi::PxuiButtonConfig::w_kind field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc impl Clone @HEAD
pxui_types::ffi::PxuiButtonDesc impl Copy @HEAD
pxui_types::ffi::PxuiButtonDesc struct @HEAD
pxui_types::ffi::PxuiButtonDesc::accessory field Str @HEAD
pxui_types::ffi::PxuiButtonDesc::accessory_box field PxuiLabelBox @HEAD
pxui_types::ffi::PxuiButtonDesc::accessory_label field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiButtonDesc::axis field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::centered field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::content_align field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::danger field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::detail field Str @HEAD
pxui_types::ffi::PxuiButtonDesc::detail_box field PxuiLabelBox @HEAD
pxui_types::ffi::PxuiButtonDesc::detail_label field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiButtonDesc::enabled field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::focus_on_click field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::gap field f32 @HEAD
pxui_types::ffi::PxuiButtonDesc::h field f32 @HEAD
pxui_types::ffi::PxuiButtonDesc::h_kind field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::has_accessory_label field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::has_detail_label field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::has_label field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::has_name field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::icon_size field f32 @U2b.7
pxui_types::ffi::PxuiButtonDesc::justify field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::key field Str @HEAD
pxui_types::ffi::PxuiButtonDesc::label field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiButtonDesc::label_box field PxuiLabelBox @HEAD
pxui_types::ffi::PxuiButtonDesc::lead field Str @HEAD
pxui_types::ffi::PxuiButtonDesc::lead_draw field PxuiIconDraw @HEAD
pxui_types::ffi::PxuiButtonDesc::lead_user field *mut c_void @HEAD
pxui_types::ffi::PxuiButtonDesc::name field Str @HEAD
pxui_types::ffi::PxuiButtonDesc::padding field [f32; 4] @HEAD
pxui_types::ffi::PxuiButtonDesc::reserved field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::reserved2 field u8 @U5.3
pxui_types::ffi::PxuiButtonDesc::reserved3 field [u8; 6] @HEAD
pxui_types::ffi::PxuiButtonDesc::reserved5 field [u8; 4] @U5.3
pxui_types::ffi::PxuiButtonDesc::selected field u8 @HEAD
pxui_types::ffi::PxuiButtonDesc::size field u32 @HEAD
pxui_types::ffi::PxuiButtonDesc::states field *const PxuiLookDesc @HEAD
pxui_types::ffi::PxuiButtonDesc::text field Str @HEAD
pxui_types::ffi::PxuiButtonDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiButtonDesc::w field f32 @HEAD
pxui_types::ffi::PxuiButtonDesc::w_kind field u8 @HEAD
pxui_types::ffi::PxuiCellBuild type = Option<extern "C" fn(*mut c_void, *mut c_void, usize, usize)> @HEAD
pxui_types::ffi::PxuiCellState impl Clone @U2b.1
pxui_types::ffi::PxuiCellState impl Copy @U2b.1
pxui_types::ffi::PxuiCellState impl Debug @U2b.1
pxui_types::ffi::PxuiCellState impl Default @U2b.1
pxui_types::ffi::PxuiCellState struct @U2b.1
pxui_types::ffi::PxuiCellState::enabled field u8 @U2b.1
pxui_types::ffi::PxuiCellState::focused field u8 @U2b.1
pxui_types::ffi::PxuiCellState::handle field PxuiBox @U2b.1
pxui_types::ffi::PxuiCellState::held field u8 @U2b.1
pxui_types::ffi::PxuiCellState::reserved field u32 @U2b.1
pxui_types::ffi::PxuiCellState::selected field u8 @U2b.1
pxui_types::ffi::PxuiCellState::size field u32 @U4a.6
pxui_types::ffi::PxuiCellState::y field f32 @U2b.1
pxui_types::ffi::PxuiCheckConfig impl Clone @HEAD
pxui_types::ffi::PxuiCheckConfig impl Copy @HEAD
pxui_types::ffi::PxuiCheckConfig struct @HEAD
pxui_types::ffi::PxuiCheckConfig::box_size field f32 @U2a.3
pxui_types::ffi::PxuiCheckConfig::label field Str @HEAD
pxui_types::ffi::PxuiCheckConfig::on field u8 @HEAD
pxui_types::ffi::PxuiCheckConfig::passive field u8 @U2a.3
pxui_types::ffi::PxuiCheckConfig::reserved2 field [u8; 3] @U2a.3
pxui_types::ffi::PxuiCheckConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiCheckConfig::size field u32 @HEAD
pxui_types::ffi::PxuiCheckConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiCheckConfig::states field *const States @U2a.3
pxui_types::ffi::PxuiCheckConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiCheckboxDesc impl Clone @U2a.3
pxui_types::ffi::PxuiCheckboxDesc impl Copy @U2a.3
pxui_types::ffi::PxuiCheckboxDesc struct @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::box_size field f32 @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::disabled field u8 @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::key field Str @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::look field *const PxuiLookDesc @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::on field u8 @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::passive field u8 @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::reserved field u8 @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::reserved_variant field [u8; 6] @U5.3
pxui_types::ffi::PxuiCheckboxDesc::size field u32 @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::text field Str @U2a.3
pxui_types::ffi::PxuiCheckboxDesc::variant field u16 @U5.3
-pxui_types::ffi::PxuiCollectionEmit type = Option<extern "C" fn(*mut c_void, Str)> @U2c.2
-pxui_types::ffi::PxuiCollectionText type = Option<extern "C" fn(*mut c_void, usize, usize, PxuiCollectionEmit, *mut c_void)> @U2c.2
pxui_types::ffi::PxuiColorSpan impl Clone @HEAD
pxui_types::ffi::PxuiColorSpan impl Copy @HEAD
pxui_types::ffi::PxuiColorSpan impl Debug @HEAD
pxui_types::ffi::PxuiColorSpan impl Default @HEAD
pxui_types::ffi::PxuiColorSpan struct @HEAD
pxui_types::ffi::PxuiColorSpan::color field Rgba @HEAD
pxui_types::ffi::PxuiColorSpan::end field i64 @HEAD
pxui_types::ffi::PxuiColorSpan::reserved field u32 @HEAD
pxui_types::ffi::PxuiColorSpan::start field i64 @HEAD
pxui_types::ffi::PxuiColumn impl Clone @U2b.1
pxui_types::ffi::PxuiColumn impl Copy @U2b.1
pxui_types::ffi::PxuiColumn impl Debug @U2b.1
pxui_types::ffi::PxuiColumn struct @U2b.1
pxui_types::ffi::PxuiColumn::grip field u8 @U2b.1
pxui_types::ffi::PxuiColumn::has_id field u8 @U2b.1
pxui_types::ffi::PxuiColumn::id field i64 @U2b.1
pxui_types::ffi::PxuiColumn::reserved field u8 @U2b.1
pxui_types::ffi::PxuiColumn::share field f32 @U2b.1
pxui_types::ffi::PxuiColumn::size field u32 @U2b.1
pxui_types::ffi::PxuiColumn::sortable field u8 @U2b.1
pxui_types::ffi::PxuiColumn::title field Str @U2b.1
pxui_types::ffi::PxuiColumn::w field f32 @U2b.1
pxui_types::ffi::PxuiColumnWidth impl Clone @U2b.1
pxui_types::ffi::PxuiColumnWidth impl Copy @U2b.1
pxui_types::ffi::PxuiColumnWidth impl Debug @U2b.1
pxui_types::ffi::PxuiColumnWidth impl Default @U2b.1
pxui_types::ffi::PxuiColumnWidth struct @U2b.1
pxui_types::ffi::PxuiColumnWidth::id field i64 @U2b.1
pxui_types::ffi::PxuiColumnWidth::reserved field u32 @U2b.1
pxui_types::ffi::PxuiColumnWidth::w field f32 @U2b.1
pxui_types::ffi::PxuiColumnWidthSpan impl Clone @U2b.1
pxui_types::ffi::PxuiColumnWidthSpan impl Copy @U2b.1
pxui_types::ffi::PxuiColumnWidthSpan impl Debug @U2b.1
pxui_types::ffi::PxuiColumnWidthSpan struct @U2b.1
pxui_types::ffi::PxuiColumnWidthSpan::len field usize @U2b.1
pxui_types::ffi::PxuiColumnWidthSpan::ptr field *mut PxuiColumnWidth @U2b.1
pxui_types::ffi::PxuiCompletion impl Clone @HEAD
pxui_types::ffi::PxuiCompletion impl Copy @HEAD
pxui_types::ffi::PxuiCompletion impl Debug @HEAD
pxui_types::ffi::PxuiCompletion struct @HEAD
pxui_types::ffi::PxuiCompletion::detail field Str @HEAD
pxui_types::ffi::PxuiCompletion::doc field Str @HEAD
pxui_types::ffi::PxuiCompletion::end field i64 @HEAD
pxui_types::ffi::PxuiCompletion::insert field Str @HEAD
pxui_types::ffi::PxuiCompletion::label field Str @HEAD
pxui_types::ffi::PxuiCompletion::start field i64 @HEAD
pxui_types::ffi::PxuiCurveDesc impl Clone @HEAD
pxui_types::ffi::PxuiCurveDesc impl Copy @HEAD
pxui_types::ffi::PxuiCurveDesc impl Debug @HEAD
pxui_types::ffi::PxuiCurveDesc impl Default @HEAD
pxui_types::ffi::PxuiCurveDesc impl From<Curve> @HEAD
pxui_types::ffi::PxuiCurveDesc struct @HEAD
pxui_types::ffi::PxuiCurveDesc::BEZIER const u32 @HEAD
pxui_types::ffi::PxuiCurveDesc::NONE const u32 @HEAD
pxui_types::ffi::PxuiCurveDesc::SPRING const u32 @HEAD
pxui_types::ffi::PxuiCurveDesc::STEPS const u32 @HEAD
pxui_types::ffi::PxuiCurveDesc::TABLE const u32 @HEAD
pxui_types::ffi::PxuiCurveDesc::count field u32 @HEAD
pxui_types::ffi::PxuiCurveDesc::duration field f32 @HEAD
pxui_types::ffi::PxuiCurveDesc::kind field u32 @HEAD
pxui_types::ffi::PxuiCurveDesc::omega field f32 @HEAD
pxui_types::ffi::PxuiCurveDesc::points field [f32; 4] @HEAD
pxui_types::ffi::PxuiCurveDesc::positions field [f32; 16] @HEAD
pxui_types::ffi::PxuiCurveDesc::size field u32 @U5.4
pxui_types::ffi::PxuiCurveDesc::values field [f32; 16] @HEAD
pxui_types::ffi::PxuiFrameReasons impl Clone @HEAD
pxui_types::ffi::PxuiFrameReasons impl Copy @HEAD
pxui_types::ffi::PxuiFrameReasons impl Debug @HEAD
pxui_types::ffi::PxuiFrameReasons impl Default @HEAD
pxui_types::ffi::PxuiFrameReasons struct @HEAD
pxui_types::ffi::PxuiFrameReasons::active_loop field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::finite_keys field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::idle_loop field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::interaction field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::requested field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::reserved field u32 @HEAD
pxui_types::ffi::PxuiFrameReasons::step_build field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::step_deadline field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::step_interaction field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::step_state field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::stepped field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::transition field u64 @HEAD
pxui_types::ffi::PxuiFrameReasons::transition_curve field PxuiCurveDesc @HEAD
pxui_types::ffi::PxuiFrameReasons::transition_key field i64 @HEAD
pxui_types::ffi::PxuiDamage type = Damage @HEAD
pxui_types::ffi::PxuiDescription impl Clone @HEAD
pxui_types::ffi::PxuiDescription impl Copy @HEAD
pxui_types::ffi::PxuiDescription impl Debug @HEAD
pxui_types::ffi::PxuiDescription struct @HEAD
pxui_types::ffi::PxuiDescription::end field i64 @HEAD
pxui_types::ffi::PxuiDescription::has field u8 @HEAD
pxui_types::ffi::PxuiDescription::reserved field [u8; 7] @HEAD
pxui_types::ffi::PxuiDescription::start field i64 @HEAD
pxui_types::ffi::PxuiDescription::text field Str @HEAD
pxui_types::ffi::PxuiDiagnostics impl Clone @U5.9
pxui_types::ffi::PxuiDiagnostics impl Copy @U5.9
pxui_types::ffi::PxuiDiagnostics impl Debug @U5.9
pxui_types::ffi::PxuiDiagnostics struct @U5.9
pxui_types::ffi::PxuiDialogConfig impl Clone @HEAD
pxui_types::ffi::PxuiDialogConfig impl Copy @HEAD
pxui_types::ffi::PxuiDialogConfig struct @HEAD
pxui_types::ffi::PxuiDialogConfig::built_in field PxuiSlot @U2b.4.38
pxui_types::ffi::PxuiDialogConfig::buttons field Slice<Str> @HEAD
pxui_types::ffi::PxuiDialogConfig::cancel_at field i64 @HEAD
pxui_types::ffi::PxuiDialogConfig::content field PxuiSlot @HEAD
pxui_types::ffi::PxuiDialogConfig::danger field u64 @HEAD
pxui_types::ffi::PxuiDialogConfig::default_at field i64 @HEAD
pxui_types::ffi::PxuiDialogConfig::details field Slice<Str> @HEAD
pxui_types::ffi::PxuiDialogConfig::disabled field u64 @HEAD
pxui_types::ffi::PxuiDialogConfig::size field u32 @HEAD
pxui_types::ffi::PxuiDialogConfig::title field Str @HEAD
pxui_types::ffi::PxuiDialogConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiDialogDesc impl Clone @HEAD
pxui_types::ffi::PxuiDialogDesc impl Copy @HEAD
pxui_types::ffi::PxuiDialogDesc impl Debug @HEAD
pxui_types::ffi::PxuiDialogDesc struct @HEAD
pxui_types::ffi::PxuiDialogDesc::buttons field Slice<Str> @HEAD
pxui_types::ffi::PxuiDialogDesc::cancel_at field i64 @HEAD
pxui_types::ffi::PxuiDialogDesc::danger field u64 @HEAD
pxui_types::ffi::PxuiDialogDesc::default_at field i64 @HEAD
pxui_types::ffi::PxuiDialogDesc::details field Slice<Str> @HEAD
pxui_types::ffi::PxuiDialogDesc::disabled field u64 @HEAD
pxui_types::ffi::PxuiDialogDesc::key field Str @HEAD
pxui_types::ffi::PxuiDialogDesc::reserved_variant field [u8; 6] @U5.3
pxui_types::ffi::PxuiDialogDesc::size field u32 @HEAD
pxui_types::ffi::PxuiDialogDesc::title field Str @HEAD
pxui_types::ffi::PxuiDialogDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiDialogDesc::width field f32 @HEAD
-pxui_types::ffi::PxuiDialogOut impl Clone @U2c.2
-pxui_types::ffi::PxuiDialogOut impl Copy @U2c.2
-pxui_types::ffi::PxuiDialogOut impl Debug @U2c.2
-pxui_types::ffi::PxuiDialogOut impl Default @U2c.2
-pxui_types::ffi::PxuiDialogOut struct @U2c.2
-pxui_types::ffi::PxuiDialogOut::index field usize @U2c.2
-pxui_types::ffi::PxuiDialogOut::tag field u32 @U2c.2
pxui_types::ffi::PxuiDialogResult impl Clone @HEAD
pxui_types::ffi::PxuiDialogResult impl Copy @HEAD
pxui_types::ffi::PxuiDialogResult impl Debug @HEAD
pxui_types::ffi::PxuiDialogResult struct @HEAD
pxui_types::ffi::PxuiDialogResult::EMPTY const PxuiDialogResult @HEAD
pxui_types::ffi::PxuiDialogResult::chosen field u64 @HEAD
pxui_types::ffi::PxuiDialogResult::dismissed field u8 @HEAD
pxui_types::ffi::PxuiDialogResult::has_chosen field u8 @HEAD
pxui_types::ffi::PxuiDialogResult::reserved field [u8; 2] @HEAD
pxui_types::ffi::PxuiDialogResult::size field u32 @HEAD
pxui_types::ffi::PxuiDirty impl Clone @HEAD
pxui_types::ffi::PxuiDirty impl Copy @HEAD
pxui_types::ffi::PxuiDirty impl Debug @HEAD
pxui_types::ffi::PxuiDirty impl Default @HEAD
pxui_types::ffi::PxuiDirty impl Eq @HEAD
pxui_types::ffi::PxuiDirty impl PartialEq @HEAD
pxui_types::ffi::PxuiDirty impl StructuralPartialEq @HEAD
pxui_types::ffi::PxuiDirty struct @HEAD
pxui_types::ffi::PxuiDirty::end field u32 @HEAD
pxui_types::ffi::PxuiDirty::start field u32 @HEAD
pxui_types::ffi::PxuiDismissIn impl Clone @HEAD
pxui_types::ffi::PxuiDismissIn impl Copy @HEAD
pxui_types::ffi::PxuiDismissIn impl Debug @HEAD
pxui_types::ffi::PxuiDismissIn impl Default @HEAD
pxui_types::ffi::PxuiDismissIn struct @HEAD
pxui_types::ffi::PxuiDismissIn::fraction field f32 @HEAD
pxui_types::ffi::PxuiDismissIn::size field u32 @HEAD
pxui_types::ffi::PxuiDismissIn::velocity field f32 @HEAD
pxui_types::ffi::PxuiDismissOut impl Clone @HEAD
pxui_types::ffi::PxuiDismissOut impl Copy @HEAD
pxui_types::ffi::PxuiDismissOut impl Debug @HEAD
pxui_types::ffi::PxuiDismissOut impl Default @HEAD
pxui_types::ffi::PxuiDismissOut struct @HEAD
pxui_types::ffi::PxuiDismissOut::flags field u32 @HEAD
pxui_types::ffi::PxuiDismissOut::offset field f32 @HEAD
pxui_types::ffi::PxuiDismissOut::size field u32 @HEAD
pxui_types::ffi::PxuiDropdownConfig impl Clone @HEAD
pxui_types::ffi::PxuiDropdownConfig impl Copy @HEAD
pxui_types::ffi::PxuiDropdownConfig struct @HEAD
pxui_types::ffi::PxuiDropdownConfig::button field PxuiBox @HEAD
pxui_types::ffi::PxuiDropdownConfig::key field i64 @HEAD
pxui_types::ffi::PxuiDropdownConfig::open field u8 @HEAD
pxui_types::ffi::PxuiDropdownConfig::size field u32 @HEAD
pxui_types::ffi::PxuiDropdownConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiDropdownConfig::text field Str @HEAD
pxui_types::ffi::PxuiDropdownConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiF32MutSpan impl Clone @HEAD
pxui_types::ffi::PxuiF32MutSpan impl Copy @HEAD
pxui_types::ffi::PxuiF32MutSpan impl Default @HEAD
pxui_types::ffi::PxuiF32MutSpan struct @HEAD
pxui_types::ffi::PxuiF32MutSpan::len field usize @HEAD
pxui_types::ffi::PxuiF32MutSpan::ptr field *mut f32 @HEAD
pxui_types::ffi::PxuiFontDesc impl Clone @HEAD
pxui_types::ffi::PxuiFontDesc impl Copy @HEAD
pxui_types::ffi::PxuiFontDesc impl Debug @HEAD
pxui_types::ffi::PxuiFontDesc impl Default @HEAD
pxui_types::ffi::PxuiFontDesc struct @HEAD
pxui_types::ffi::PxuiFontDesc::caps field u8 @HEAD
pxui_types::ffi::PxuiFontDesc::family field u16 @HEAD
pxui_types::ffi::PxuiFontDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiFontDesc::size field i64 @HEAD
pxui_types::ffi::PxuiFontDesc::tracking field f32 @HEAD
pxui_types::ffi::PxuiFontDesc::weight field u16 @HEAD
pxui_types::ffi::PxuiFooterConfig impl Clone @HEAD
pxui_types::ffi::PxuiFooterConfig impl Copy @HEAD
pxui_types::ffi::PxuiFooterConfig struct @HEAD
pxui_types::ffi::PxuiFooterConfig::has_right field u8 @HEAD
pxui_types::ffi::PxuiFooterConfig::hints field Slice<PxuiHintDesc> @HEAD
pxui_types::ffi::PxuiFooterConfig::right field Str @HEAD
pxui_types::ffi::PxuiFooterConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiFooterConfig::size field u32 @HEAD
pxui_types::ffi::PxuiFooterConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiFooterDesc struct @HEAD
pxui_types::ffi::PxuiFooterDesc::has_right field u8 @HEAD
pxui_types::ffi::PxuiFooterDesc::hints field Slice<PxuiHintDesc> @HEAD
pxui_types::ffi::PxuiFooterDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiFooterDesc::right field Str @HEAD
pxui_types::ffi::PxuiFooterDesc::size field u32 @HEAD
pxui_types::ffi::PxuiGestureIn impl Clone @HEAD
pxui_types::ffi::PxuiGestureIn impl Copy @HEAD
pxui_types::ffi::PxuiGestureIn impl Debug @HEAD
pxui_types::ffi::PxuiGestureIn impl Default @HEAD
pxui_types::ffi::PxuiGestureIn struct @HEAD
pxui_types::ffi::PxuiGestureIn::kinds field u32 @HEAD
pxui_types::ffi::PxuiGestureIn::reveal field f32 @HEAD
pxui_types::ffi::PxuiGestureIn::size field u32 @HEAD
pxui_types::ffi::PxuiGestureIn::threshold field f32 @HEAD
pxui_types::ffi::PxuiGestureOut impl Clone @HEAD
pxui_types::ffi::PxuiGestureOut impl Copy @HEAD
pxui_types::ffi::PxuiGestureOut impl Debug @HEAD
pxui_types::ffi::PxuiGestureOut impl Default @HEAD
pxui_types::ffi::PxuiGestureOut struct @HEAD
pxui_types::ffi::PxuiGestureOut::back field f32 @HEAD
pxui_types::ffi::PxuiGestureOut::flags field u32 @HEAD
pxui_types::ffi::PxuiGestureOut::pull field f32 @HEAD
pxui_types::ffi::PxuiGestureOut::size field u32 @HEAD
pxui_types::ffi::PxuiGestureOut::swipe field f32 @HEAD
pxui_types::ffi::PxuiGridBuild type = Option<extern "C" fn(*mut c_void, *mut c_void, usize)> @HEAD
pxui_types::ffi::PxuiGridListDesc struct @HEAD
pxui_types::ffi::PxuiGridListDesc::build field PxuiGridBuild @HEAD
pxui_types::ffi::PxuiGridListDesc::cell_h field f32 @HEAD
pxui_types::ffi::PxuiGridListDesc::cell_w field f32 @HEAD
pxui_types::ffi::PxuiGridListDesc::count field usize @HEAD
pxui_types::ffi::PxuiGridListDesc::label field Str @HEAD
pxui_types::ffi::PxuiGridListDesc::reserved field u32 @HEAD
pxui_types::ffi::PxuiGridListDesc::size field u32 @HEAD
pxui_types::ffi::PxuiGridListDesc::user field *mut c_void @HEAD
pxui_types::ffi::PxuiGripConfig impl Clone @HEAD
pxui_types::ffi::PxuiGripConfig impl Copy @HEAD
pxui_types::ffi::PxuiGripConfig struct @HEAD
pxui_types::ffi::PxuiGripConfig::floating field u8 @HEAD
pxui_types::ffi::PxuiGripConfig::grip field PxuiBox @HEAD
pxui_types::ffi::PxuiGripConfig::size field u32 @HEAD
pxui_types::ffi::PxuiGripConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiGripConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiGripConfig::vertical field u8 @HEAD
pxui_types::ffi::PxuiHeadActionDesc impl Clone @HEAD
pxui_types::ffi::PxuiHeadActionDesc impl Copy @HEAD
pxui_types::ffi::PxuiHeadActionDesc struct @HEAD
pxui_types::ffi::PxuiHeadActionDesc::active field u8 @HEAD
pxui_types::ffi::PxuiHeadActionDesc::caption field Str @HEAD
pxui_types::ffi::PxuiHeadActionDesc::keycap field Str @HEAD
pxui_types::ffi::PxuiHeadActionDesc::reserved field [u8; 2] @HEAD
pxui_types::ffi::PxuiHeadActionDesc::size field u32 @HEAD
pxui_types::ffi::PxuiHeadActionDesc::usable field u8 @HEAD
pxui_types::ffi::PxuiHeadOut impl Clone @HEAD
pxui_types::ffi::PxuiHeadOut impl Copy @HEAD
pxui_types::ffi::PxuiHeadOut struct @HEAD
pxui_types::ffi::PxuiHeadOut::chosen field i64 @HEAD
pxui_types::ffi::PxuiHeadOut::has_chosen field u8 @HEAD
pxui_types::ffi::PxuiHeadOut::renamed field Str @HEAD
pxui_types::ffi::PxuiHeadOut::reserved field [u8; 2] @HEAD
pxui_types::ffi::PxuiHeadOut::reset_pressed field u8 @HEAD
pxui_types::ffi::PxuiHeadOut::size field u32 @HEAD
pxui_types::ffi::PxuiHeadResult impl Clone @U2b.1
pxui_types::ffi::PxuiHeadResult impl Copy @U2b.1
pxui_types::ffi::PxuiHeadResult impl Debug @U2b.1
pxui_types::ffi::PxuiHeadResult struct @U2b.1
pxui_types::ffi::PxuiHeadResult::ascending field u8 @U2b.1
pxui_types::ffi::PxuiHeadResult::has_sort field u8 @U2b.1
pxui_types::ffi::PxuiHeadResult::resized field u8 @U2b.1
pxui_types::ffi::PxuiHeadResult::response field PxuiResponse @U2b.1
pxui_types::ffi::PxuiHeadResult::size field u32 @U2b.1
pxui_types::ffi::PxuiHeadResult::sort_id field i64 @U2b.1
pxui_types::ffi::PxuiHeadResult::sorted field u8 @U2b.1
pxui_types::ffi::PxuiHeaderDesc impl Clone @HEAD
pxui_types::ffi::PxuiHeaderDesc impl Copy @HEAD
pxui_types::ffi::PxuiHeaderDesc struct @HEAD
pxui_types::ffi::PxuiHeaderDesc::actions field Slice<PxuiHeadActionDesc> @HEAD
pxui_types::ffi::PxuiHeaderDesc::badge field Str @HEAD
pxui_types::ffi::PxuiHeaderDesc::detail field Str @HEAD
pxui_types::ffi::PxuiHeaderDesc::has_badge field u8 @HEAD
pxui_types::ffi::PxuiHeaderDesc::has_index field u8 @HEAD
pxui_types::ffi::PxuiHeaderDesc::has_kind field u8 @HEAD
pxui_types::ffi::PxuiHeaderDesc::has_reset field u8 @HEAD
pxui_types::ffi::PxuiHeaderDesc::index field Str @HEAD
pxui_types::ffi::PxuiHeaderDesc::key field Str @HEAD
pxui_types::ffi::PxuiHeaderDesc::kind field Str @HEAD
pxui_types::ffi::PxuiHeaderDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiHeaderDesc::reset field Str @HEAD
pxui_types::ffi::PxuiHeaderDesc::reset_enabled field u8 @HEAD
pxui_types::ffi::PxuiHeaderDesc::size field u32 @HEAD
pxui_types::ffi::PxuiHeaderDesc::title field Str @HEAD
pxui_types::ffi::PxuiHintDesc impl Clone @HEAD
pxui_types::ffi::PxuiHintDesc impl Copy @HEAD
pxui_types::ffi::PxuiHintDesc impl Default @HEAD
pxui_types::ffi::PxuiHintDesc struct @HEAD
pxui_types::ffi::PxuiHintDesc::key field Str @HEAD
pxui_types::ffi::PxuiHintDesc::reserved field u32 @HEAD
pxui_types::ffi::PxuiHintDesc::size field u32 @HEAD
pxui_types::ffi::PxuiHintDesc::value field Str @HEAD
pxui_types::ffi::PxuiIconButtonConfig impl Clone @HEAD
pxui_types::ffi::PxuiIconButtonConfig impl Copy @HEAD
pxui_types::ffi::PxuiIconButtonConfig struct @HEAD
pxui_types::ffi::PxuiIconButtonConfig::built_in field PxuiSlot @U2b.4.6
pxui_types::ffi::PxuiIconButtonConfig::draw field PxuiIconDraw @HEAD
pxui_types::ffi::PxuiIconButtonConfig::draw_user field *mut c_void @HEAD
pxui_types::ffi::PxuiIconButtonConfig::draw_word field u32 @U2b.7
pxui_types::ffi::PxuiIconButtonConfig::h field f32 @HEAD
pxui_types::ffi::PxuiIconButtonConfig::h_kind field u8 @HEAD
pxui_types::ffi::PxuiIconButtonConfig::icon field Str @HEAD
pxui_types::ffi::PxuiIconButtonConfig::ink field Rgba @HEAD
pxui_types::ffi::PxuiIconButtonConfig::label field Str @HEAD
pxui_types::ffi::PxuiIconButtonConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiIconButtonConfig::press field *const Curve @HEAD
pxui_types::ffi::PxuiIconButtonConfig::press_scale field f32 @HEAD
pxui_types::ffi::PxuiIconButtonConfig::reserved field u8 @HEAD
pxui_types::ffi::PxuiIconButtonConfig::size field u32 @HEAD
pxui_types::ffi::PxuiIconButtonConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiIconButtonConfig::states field *const States @HEAD
pxui_types::ffi::PxuiIconButtonConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiIconButtonConfig::w field f32 @HEAD
pxui_types::ffi::PxuiIconButtonConfig::w_kind field u8 @HEAD
pxui_types::ffi::PxuiIconButtonDesc impl Clone @HEAD
pxui_types::ffi::PxuiIconButtonDesc impl Copy @HEAD
pxui_types::ffi::PxuiIconButtonDesc struct @HEAD
pxui_types::ffi::PxuiIconButtonDesc::checkable field u8 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::checked field u8 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::draw field PxuiIconDraw @HEAD
pxui_types::ffi::PxuiIconButtonDesc::draw_user field *mut c_void @HEAD
pxui_types::ffi::PxuiIconButtonDesc::enabled field u8 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::h field f32 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::h_kind field u8 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::icon field Str @HEAD
pxui_types::ffi::PxuiIconButtonDesc::key field Str @HEAD
pxui_types::ffi::PxuiIconButtonDesc::name field Str @HEAD
pxui_types::ffi::PxuiIconButtonDesc::press field *const Curve @HEAD
pxui_types::ffi::PxuiIconButtonDesc::press_scale field f32 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::reserved field u8 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::reserved2 field [u8; 4] @U5.3
pxui_types::ffi::PxuiIconButtonDesc::size field u32 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::states field *const PxuiLookDesc @HEAD
pxui_types::ffi::PxuiIconButtonDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiIconButtonDesc::w field f32 @HEAD
pxui_types::ffi::PxuiIconButtonDesc::w_kind field u8 @HEAD
pxui_types::ffi::PxuiIconDraw type = Option<extern "C" fn(*mut c_void, *mut c_void, Rect, Rgba, *const PxuiPaintInfo)> @HEAD
pxui_types::ffi::PxuiInput type = Input @HEAD
pxui_types::ffi::PxuiInspectorRowConfig impl Clone @HEAD
pxui_types::ffi::PxuiInspectorRowConfig impl Copy @HEAD
pxui_types::ffi::PxuiInspectorRowConfig struct @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::has_pin field u8 @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::label field Str @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::label_w field f32 @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::pin field u8 @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::size field u32 @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiInspectorRowConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc struct @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::control_w field f32 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::control_x field f32 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::control_y field f32 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::has_pin field u8 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::has_width field u8 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::key field Str @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::label field Str @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::pin field u8 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::reserved field u8 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::reserved_tail field u32 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::row field PxuiBox @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::size field u32 @HEAD
pxui_types::ffi::PxuiInspectorRowDesc::width field f32 @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc struct @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::build field PxuiBuild @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::expanded field u8 @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::has_set field u8 @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::key field Str @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::label field Str @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::out_open field u8 @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::reserved field u8 @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::set_expanded field u8 @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::size field u32 @HEAD
pxui_types::ffi::PxuiInspectorSectionDesc::user field *mut c_void @HEAD
pxui_types::ffi::PxuiInspectorTextDesc struct @HEAD
pxui_types::ffi::PxuiInspectorTextDesc::key field Str @HEAD
pxui_types::ffi::PxuiInspectorTextDesc::reserved field u32 @HEAD
pxui_types::ffi::PxuiInspectorTextDesc::size field u32 @HEAD
pxui_types::ffi::PxuiInspectorTextDesc::text field Str @HEAD
pxui_types::ffi::PxuiInspectorValueDesc struct @HEAD
pxui_types::ffi::PxuiInspectorValueDesc::has_width field u8 @HEAD
pxui_types::ffi::PxuiInspectorValueDesc::key field Str @HEAD
pxui_types::ffi::PxuiInspectorValueDesc::label field Str @HEAD
pxui_types::ffi::PxuiInspectorValueDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiInspectorValueDesc::size field u32 @HEAD
pxui_types::ffi::PxuiInspectorValueDesc::value field Str @HEAD
pxui_types::ffi::PxuiInspectorValueDesc::width field f32 @HEAD
pxui_types::ffi::PxuiKeyedDesc impl Clone @HEAD
pxui_types::ffi::PxuiKeyedDesc impl Copy @HEAD
pxui_types::ffi::PxuiKeyedDesc impl Debug @HEAD
pxui_types::ffi::PxuiKeyedDesc struct @HEAD
pxui_types::ffi::PxuiKeyedDesc::key field Str @HEAD
pxui_types::ffi::PxuiKeyedDesc::name field Str @HEAD
pxui_types::ffi::PxuiKeyedDesc::new fn (key: &str, name: &str) -> PxuiKeyedDesc @HEAD
pxui_types::ffi::PxuiKeyedDesc::reserved field u16 @U5.3
pxui_types::ffi::PxuiKeyedDesc::size field u32 @HEAD
pxui_types::ffi::PxuiKeyedDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiKeyframeDesc impl Clone @HEAD
pxui_types::ffi::PxuiKeyframeDesc impl Copy @HEAD
pxui_types::ffi::PxuiKeyframeDesc impl Default @HEAD
pxui_types::ffi::PxuiKeyframeDesc struct @HEAD
pxui_types::ffi::PxuiKeyframeDesc::curve field PxuiCurveDesc @HEAD
pxui_types::ffi::PxuiKeyframeDesc::look field PxuiLookDesc @HEAD
pxui_types::ffi::PxuiKeyframeDesc::reserved field u32 @HEAD
pxui_types::ffi::PxuiKeyframeDesc::size field u32 @HEAD
pxui_types::ffi::PxuiKeyframeDesc::target field u32 @HEAD
pxui_types::ffi::PxuiKeyframeDesc::time field f32 @HEAD
pxui_types::ffi::PxuiKeyframeDesc::values field [f32; 4] @HEAD
pxui_types::ffi::PxuiLabelBox impl Clone @HEAD
pxui_types::ffi::PxuiLabelBox impl Copy @HEAD
pxui_types::ffi::PxuiLabelBox impl Debug @HEAD
pxui_types::ffi::PxuiLabelBox impl Default @HEAD
pxui_types::ffi::PxuiLabelBox impl From<&Label> @HEAD
pxui_types::ffi::PxuiLabelBox impl PartialEq @HEAD
pxui_types::ffi::PxuiLabelBox impl StructuralPartialEq @HEAD
pxui_types::ffi::PxuiLabelBox struct @HEAD
pxui_types::ffi::PxuiLabelBox::SET_ALL const u32 @HEAD
pxui_types::ffi::PxuiLabelBox::SET_HEIGHT const u32 @HEAD
pxui_types::ffi::PxuiLabelBox::SET_ROLE const u32 @HEAD
pxui_types::ffi::PxuiLabelBox::height field f32 @HEAD
pxui_types::ffi::PxuiLabelBox::role field u8 @HEAD
pxui_types::ffi::PxuiLabelBox::set field u32 @HEAD
pxui_types::ffi::PxuiLabelBox::size field u32 @HEAD
pxui_types::ffi::PxuiLabelBox::variant field u16 @U5.3
pxui_types::ffi::PxuiLabelBox::whole_pixels field u8 @HEAD
pxui_types::ffi::PxuiLabelConfig impl Clone @HEAD
pxui_types::ffi::PxuiLabelConfig impl Copy @HEAD
pxui_types::ffi::PxuiLabelConfig struct @HEAD
pxui_types::ffi::PxuiLabelConfig::first field u8 @HEAD
pxui_types::ffi::PxuiLabelConfig::label_box field PxuiLabelBox @HEAD
pxui_types::ffi::PxuiLabelConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiLabelConfig::size field u32 @HEAD
pxui_types::ffi::PxuiLabelConfig::style field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiLabelConfig::text field Str @HEAD
pxui_types::ffi::PxuiLabelConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiLabelStyle impl Clone @HEAD
pxui_types::ffi::PxuiLabelStyle impl Copy @HEAD
pxui_types::ffi::PxuiLabelStyle impl Debug @HEAD
pxui_types::ffi::PxuiLabelStyle impl Default @HEAD
pxui_types::ffi::PxuiLabelStyle impl From<&Label> @HEAD
pxui_types::ffi::PxuiLabelStyle struct @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_ALL const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_CAPS const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_COLOR const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_FONT const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_LINE_HEIGHT const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_PAD const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_SIZE const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_TRACKING const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_WEIGHT const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::SET_WIDTH const u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::align field u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::caps field u8 @HEAD
pxui_types::ffi::PxuiLabelStyle::color field u16 @HEAD
pxui_types::ffi::PxuiLabelStyle::digits field u8 @HEAD
pxui_types::ffi::PxuiLabelStyle::ellipsis field u8 @HEAD
pxui_types::ffi::PxuiLabelStyle::features field u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::font field u16 @HEAD
pxui_types::ffi::PxuiLabelStyle::line_height field f32 @HEAD
pxui_types::ffi::PxuiLabelStyle::max_lines field u16 @HEAD
pxui_types::ffi::PxuiLabelStyle::pad field f32 @HEAD
pxui_types::ffi::PxuiLabelStyle::reserved field u8 @HEAD
pxui_types::ffi::PxuiLabelStyle::set field u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::size field u32 @HEAD
pxui_types::ffi::PxuiLabelStyle::size_points field f32 @HEAD
pxui_types::ffi::PxuiLabelStyle::tracking field f32 @HEAD
pxui_types::ffi::PxuiLabelStyle::weight field u16 @HEAD
pxui_types::ffi::PxuiLabelStyle::width field PxuiSizeDesc @HEAD
pxui_types::ffi::PxuiLabelStyle::wrap field u32 @HEAD
pxui_types::ffi::PxuiLayerInfo impl Clone @HEAD
pxui_types::ffi::PxuiLayerInfo impl Copy @HEAD
pxui_types::ffi::PxuiLayerInfo impl Debug @HEAD
pxui_types::ffi::PxuiLayerInfo struct @HEAD
pxui_types::ffi::PxuiLayerInfo::anchor field Rect @U5.9
pxui_types::ffi::PxuiLayerInfo::breaks field u32 @HEAD
pxui_types::ffi::PxuiLayerInfo::dirty field Slice<PxuiDirty> @HEAD
pxui_types::ffi::PxuiLayerInfo::id field u32 @HEAD
pxui_types::ffi::PxuiLayerInfo::instances field Slice<Instance> @HEAD
pxui_types::ffi::PxuiLayerInfo::key field i64 @HEAD
pxui_types::ffi::PxuiLayerInfo::role field u32 @U5.9
pxui_types::ffi::PxuiLayerInfo::size field u32 @U5.4
pxui_types::ffi::PxuiLineFieldConfig impl Clone @HEAD
pxui_types::ffi::PxuiLineFieldConfig impl Copy @HEAD
pxui_types::ffi::PxuiLineFieldConfig struct @HEAD
pxui_types::ffi::PxuiLineFieldConfig::empty field u8 @HEAD
pxui_types::ffi::PxuiLineFieldConfig::field field PxuiBox @HEAD
pxui_types::ffi::PxuiLineFieldConfig::size field u32 @HEAD
pxui_types::ffi::PxuiLineFieldConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiLineFieldConfig::text field Str @HEAD
pxui_types::ffi::PxuiLineFieldConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiListCell type = Option<extern "C" fn(*mut c_void, *mut c_void, usize, *const PxuiCellState)> @U2b.1
pxui_types::ffi::PxuiListResult impl Clone @U2b.1
pxui_types::ffi::PxuiListResult impl Copy @U2b.1
pxui_types::ffi::PxuiListResult impl Debug @U2b.1
pxui_types::ffi::PxuiListResult struct @U2b.1
pxui_types::ffi::PxuiListResult::EMPTY const PxuiListResult @U2b.1
pxui_types::ffi::PxuiListResult::activated field usize @U2b.1
pxui_types::ffi::PxuiListResult::anchor field usize @U2b.1
pxui_types::ffi::PxuiListResult::drop_at field usize @U2b.2
-pxui_types::ffi::PxuiListResult::focused field u8 @U2b.4.18
pxui_types::ffi::PxuiListResult::handle field PxuiBox @U2b.1
pxui_types::ffi::PxuiListResult::moved_from field usize @U2b.3
pxui_types::ffi::PxuiListResult::moved_to field usize @U2b.3
pxui_types::ffi::PxuiListResult::reserved field u32 @U2b.4.18
-pxui_types::ffi::PxuiListResult::reserved field [u8; 3] @U2b.4.18
pxui_types::ffi::PxuiListResult::row field PxuiBox @U2b.1
pxui_types::ffi::PxuiListResult::secondary field usize @U2b.1
pxui_types::ffi::PxuiListResult::selected field usize @U2b.1
pxui_types::ffi::PxuiListResult::size field u32 @U2b.1
-pxui_types::ffi::PxuiListViewDesc struct @U2b.1
-pxui_types::ffi::PxuiListViewDesc::count field usize @U2b.1
-pxui_types::ffi::PxuiListViewDesc::label field Str @U2b.1
-pxui_types::ffi::PxuiListViewDesc::multi field u8 @U2b.1
-pxui_types::ffi::PxuiListViewDesc::reserved field [u8; 3] @U2b.1
-pxui_types::ffi::PxuiListViewDesc::selected field PxuiU8MutSpan @U2b.1
-pxui_types::ffi::PxuiListViewDesc::size field u32 @U2b.1
-pxui_types::ffi::PxuiListViewDesc::text field PxuiCollectionText @U2b.1
-pxui_types::ffi::PxuiListViewDesc::user field *mut c_void @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc struct @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::build field PxuiCellBuild @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::count field usize @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::label field Str @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::multi field u8 @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::reserved field [u8; 3] @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::selected field PxuiU8MutSpan @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::size field u32 @U2b.1
-pxui_types::ffi::PxuiListViewWithDesc::user field *mut c_void @U2b.1
pxui_types::ffi::PxuiLookDesc impl Clone @HEAD
pxui_types::ffi::PxuiLookDesc impl Copy @HEAD
pxui_types::ffi::PxuiLookDesc impl Debug @HEAD
pxui_types::ffi::PxuiLookDesc impl Default @HEAD
pxui_types::ffi::PxuiLookDesc impl From<Look> @HEAD
pxui_types::ffi::PxuiLookDesc impl From<PxuiLookDesc> @HEAD
pxui_types::ffi::PxuiLookDesc struct @HEAD
pxui_types::ffi::PxuiLookDesc::angle field f32 @HEAD
pxui_types::ffi::PxuiLookDesc::border field Rgba @HEAD
pxui_types::ffi::PxuiLookDesc::border_w field f32 @HEAD
pxui_types::ffi::PxuiLookDesc::caret field Rgba @HEAD
pxui_types::ffi::PxuiLookDesc::dashed field u8 @HEAD
pxui_types::ffi::PxuiLookDesc::fill field Rgba @HEAD
pxui_types::ffi::PxuiLookDesc::font field PxuiFontDesc @HEAD
pxui_types::ffi::PxuiLookDesc::gradient field Rgba @HEAD
pxui_types::ffi::PxuiLookDesc::has_gradient field u8 @HEAD
pxui_types::ffi::PxuiLookDesc::opacity field f32 @HEAD
pxui_types::ffi::PxuiLookDesc::radius field [f32; 4] @HEAD
pxui_types::ffi::PxuiLookDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiLookDesc::reserved_tail field [u8; 3] @HEAD
pxui_types::ffi::PxuiLookDesc::ring field PxuiRingDesc @HEAD
pxui_types::ffi::PxuiLookDesc::selection field Rgba @HEAD
pxui_types::ffi::PxuiLookDesc::shadows field [PxuiShadowDesc; 2] @HEAD
pxui_types::ffi::PxuiLookDesc::sides field u32 @HEAD
pxui_types::ffi::PxuiLookDesc::size field u32 @U5.4
pxui_types::ffi::PxuiLookDesc::text field Rgba @HEAD
pxui_types::ffi::PxuiMarkDesc impl Clone @HEAD
pxui_types::ffi::PxuiMarkDesc impl Copy @HEAD
pxui_types::ffi::PxuiMarkDesc impl Debug @HEAD
pxui_types::ffi::PxuiMarkDesc impl Default @HEAD
pxui_types::ffi::PxuiMarkDesc struct @HEAD
pxui_types::ffi::PxuiMarkDesc::CHEVRON const u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::DIR_DOWN const u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::DIR_LEFT const u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::DIR_RIGHT const u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::DIR_UP const u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::LINE const u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::NONE const u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::direction field u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::kind field u32 @HEAD
pxui_types::ffi::PxuiMarkDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiMarkDesc::rising field u8 @HEAD
pxui_types::ffi::PxuiMarkDesc::width field f32 @HEAD
pxui_types::ffi::PxuiMaybeRange impl Clone @HEAD
pxui_types::ffi::PxuiMaybeRange impl Copy @HEAD
pxui_types::ffi::PxuiMaybeRange impl Debug @HEAD
pxui_types::ffi::PxuiMaybeRange impl Default @HEAD
pxui_types::ffi::PxuiMaybeRange struct @HEAD
pxui_types::ffi::PxuiMaybeRange::end field i64 @HEAD
pxui_types::ffi::PxuiMaybeRange::has field u8 @HEAD
pxui_types::ffi::PxuiMaybeRange::reserved field [u8; 7] @HEAD
pxui_types::ffi::PxuiMaybeRange::start field i64 @HEAD
-pxui_types::ffi::PxuiMeasuredSectionBuild type = Option<extern "C" fn(*mut c_void, *mut c_void, usize, f32)> @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc impl Clone @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc impl Copy @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc struct @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::build field PxuiMeasuredSectionBuild @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::count field usize @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::header_height field f32 @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::key field PxuiRowKey @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::label field Str @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::leading_height field f32 @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::overlay_scrollbar field u8 @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::reserved field [u8; 3] @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::row_height field f32 @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::size field u32 @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::trailing_height field f32 @U2b.1
-pxui_types::ffi::PxuiMeasuredSectionDesc::user field *mut c_void @U2b.1
pxui_types::ffi::PxuiMenuBarDesc struct @HEAD
pxui_types::ffi::PxuiMenuBarDesc::label field Str @HEAD
pxui_types::ffi::PxuiMenuBarDesc::menus field Slice<PxuiMenuDesc> @HEAD
pxui_types::ffi::PxuiMenuBarDesc::reserved field u16 @U5.3
pxui_types::ffi::PxuiMenuBarDesc::size field u32 @HEAD
pxui_types::ffi::PxuiMenuBarDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiMenuDesc impl Clone @HEAD
pxui_types::ffi::PxuiMenuDesc impl Copy @HEAD
pxui_types::ffi::PxuiMenuDesc struct @HEAD
pxui_types::ffi::PxuiMenuDesc::items field Slice<Str> @HEAD
pxui_types::ffi::PxuiMenuDesc::reserved field u32 @HEAD
pxui_types::ffi::PxuiMenuDesc::size field u32 @HEAD
pxui_types::ffi::PxuiMenuDesc::title field Str @HEAD
pxui_types::ffi::PxuiMenuItemConfig impl Clone @HEAD
pxui_types::ffi::PxuiMenuItemConfig impl Copy @HEAD
pxui_types::ffi::PxuiMenuItemConfig struct @HEAD
pxui_types::ffi::PxuiMenuItemConfig::index field usize @HEAD
pxui_types::ffi::PxuiMenuItemConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiMenuItemConfig::size field u32 @HEAD
pxui_types::ffi::PxuiMenuItemConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiMenuItemConfig::text field Str @HEAD
pxui_types::ffi::PxuiMenuItemConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiMenuOptions impl Clone @U5.7
pxui_types::ffi::PxuiMenuOptions impl Copy @U5.7
pxui_types::ffi::PxuiMenuOptions impl Debug @U5.7
pxui_types::ffi::PxuiMenuOptions struct @U5.7
pxui_types::ffi::PxuiMenuOut impl Clone @HEAD
pxui_types::ffi::PxuiMenuOut impl Copy @HEAD
pxui_types::ffi::PxuiMenuOut impl Default @HEAD
pxui_types::ffi::PxuiMenuOut struct @HEAD
pxui_types::ffi::PxuiMenuOut::item field usize @HEAD
pxui_types::ffi::PxuiMenuOut::menu field usize @HEAD
pxui_types::ffi::PxuiMenuOut::reserved field u32 @HEAD
pxui_types::ffi::PxuiMenuOut::size field u32 @HEAD
pxui_types::ffi::PxuiMenuRow impl Clone @U5.7
pxui_types::ffi::PxuiMenuRow impl Copy @U5.7
pxui_types::ffi::PxuiMenuRow impl Debug @U5.7
pxui_types::ffi::PxuiMenuRow struct @U5.7
pxui_types::ffi::PxuiMenuRowConfig impl Clone @HEAD
pxui_types::ffi::PxuiMenuRowConfig impl Copy @HEAD
pxui_types::ffi::PxuiMenuRowConfig struct @HEAD
pxui_types::ffi::PxuiMenuRowConfig::chevron field u8 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::danger field u8 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::has_shortcut field u8 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::has_swatch field u8 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::index field i64 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::lead field f32 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiMenuRowConfig::row_h field f32 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::selected field u8 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::shortcut field Str @HEAD
pxui_types::ffi::PxuiMenuRowConfig::size field u32 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiMenuRowConfig::swatch field Rgba @HEAD
pxui_types::ffi::PxuiMenuRowConfig::swatch_pad field f32 @HEAD
pxui_types::ffi::PxuiMenuRowConfig::text field Str @HEAD
pxui_types::ffi::PxuiMenuRowConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiMotion impl Clone @HEAD
pxui_types::ffi::PxuiMotion impl Copy @HEAD
pxui_types::ffi::PxuiMotion impl Debug @HEAD
pxui_types::ffi::PxuiMotion impl Default @HEAD
pxui_types::ffi::PxuiMotion impl From<PxuiMotion> @HEAD
pxui_types::ffi::PxuiMotion struct @HEAD
pxui_types::ffi::PxuiMotion::ease field [f32; 4] @HEAD
pxui_types::ffi::PxuiMotion::fast field f32 @HEAD
pxui_types::ffi::PxuiMotion::normal field f32 @HEAD
pxui_types::ffi::PxuiMotion::slow field f32 @HEAD
pxui_types::ffi::PxuiNodeFields impl Clone @HEAD
pxui_types::ffi::PxuiNodeFields impl Copy @HEAD
pxui_types::ffi::PxuiNodeFields impl Debug @HEAD
pxui_types::ffi::PxuiNodeFields struct @HEAD
pxui_types::ffi::PxuiNodeFields::inset field Edges @HEAD
pxui_types::ffi::PxuiNodeFields::look field PxuiLookDesc @HEAD
pxui_types::ffi::PxuiNodeFields::mark field PxuiMarkDesc @HEAD
pxui_types::ffi::PxuiNodeFields::reserved field u32 @HEAD
pxui_types::ffi::PxuiNodeFields::size field u32 @HEAD
pxui_types::ffi::PxuiNote impl Clone @HEAD
pxui_types::ffi::PxuiNote impl Copy @HEAD
pxui_types::ffi::PxuiNote impl Debug @HEAD
pxui_types::ffi::PxuiNote struct @HEAD
pxui_types::ffi::PxuiNote::end field i64 @HEAD
pxui_types::ffi::PxuiNote::has_range field u8 @HEAD
pxui_types::ffi::PxuiNote::line field i64 @HEAD
pxui_types::ffi::PxuiNote::reserved field [u8; 7] @HEAD
pxui_types::ffi::PxuiNote::start field i64 @HEAD
pxui_types::ffi::PxuiNote::text field Str @HEAD
pxui_types::ffi::PxuiOutput type = Output @HEAD
pxui_types::ffi::PxuiPaintInfo impl Clone @HEAD
pxui_types::ffi::PxuiPaintInfo impl Copy @HEAD
pxui_types::ffi::PxuiPaintInfo struct @HEAD
pxui_types::ffi::PxuiPaintInfo::clip field Rect @HEAD
pxui_types::ffi::PxuiPaintInfo::clip_radius field f32 @HEAD
pxui_types::ffi::PxuiPaintInfo::density field i64 @HEAD
pxui_types::ffi::PxuiPaintInfo::font_size field i64 @HEAD
pxui_types::ffi::PxuiPaintInfo::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiPaintInfo::row_height field i64 @HEAD
pxui_types::ffi::PxuiPaintInfo::rtl field u8 @HEAD
pxui_types::ffi::PxuiPaintInfo::size field u32 @HEAD
pxui_types::ffi::PxuiPaintInfo::theme field ThemeTokens @HEAD
pxui_types::ffi::PxuiPanelConfig impl Clone @HEAD
pxui_types::ffi::PxuiPanelConfig impl Copy @HEAD
pxui_types::ffi::PxuiPanelConfig struct @HEAD
pxui_types::ffi::PxuiPanelConfig::content field PxuiSlot @HEAD
pxui_types::ffi::PxuiPanelConfig::label field Str @HEAD
pxui_types::ffi::PxuiPanelConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiPanelConfig::present field u64 @U2a.4
pxui_types::ffi::PxuiPanelConfig::size field u32 @HEAD
pxui_types::ffi::PxuiPanelConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiPickItem impl Clone @U1b.1
pxui_types::ffi::PxuiPickItem impl Copy @U1b.1
pxui_types::ffi::PxuiPickItem impl Debug @U1b.1
pxui_types::ffi::PxuiPickItem impl Default @U1b.1
pxui_types::ffi::PxuiPickItem struct @U1b.1
pxui_types::ffi::PxuiPickItem::detail field Str @U1b.1
pxui_types::ffi::PxuiPickItem::has_id field u8 @U1b.1
pxui_types::ffi::PxuiPickItem::id field i64 @U1b.1
pxui_types::ffi::PxuiPickItem::name field Str @U1b.1
pxui_types::ffi::PxuiPickItem::reserved field [u8; 7] @U1b.1
pxui_types::ffi::PxuiPickItem::text field Str @U1b.1
pxui_types::ffi::PxuiPickRow impl Clone @U2a.4
pxui_types::ffi::PxuiPickRow impl Copy @U2a.4
pxui_types::ffi::PxuiPickRow impl Debug @U2a.4
pxui_types::ffi::PxuiPickRow struct @U2a.4
pxui_types::ffi::PxuiPickRow::art field u64 @U2a.4
pxui_types::ffi::PxuiPickRow::art_kind field u8 @U2a.4
pxui_types::ffi::PxuiPickRow::detail field Str @U2a.4
pxui_types::ffi::PxuiPickRow::heading field Str @U2a.4
pxui_types::ffi::PxuiPickRow::label field Str @U2a.4
pxui_types::ffi::PxuiPickRow::new fn const (label: &str, detail: &str) -> PxuiPickRow @U2a.4
pxui_types::ffi::PxuiPickRow::reserved field [u8; 3] @U2a.4
pxui_types::ffi::PxuiPickRow::size field u32 @U2a.4
pxui_types::ffi::PxuiPickerFieldConfig impl Clone @HEAD
pxui_types::ffi::PxuiPickerFieldConfig impl Copy @HEAD
pxui_types::ffi::PxuiPickerFieldConfig struct @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::empty field u8 @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::inset field f32 @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::list field PxuiBox @U2b.4.16
pxui_types::ffi::PxuiPickerFieldConfig::placeholder field Str @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::prefix field f32 @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::search field PxuiBox @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::size field u32 @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::slash field u8 @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiPickerFieldConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiPickerRowConfig impl Clone @HEAD
pxui_types::ffi::PxuiPickerRowConfig impl Copy @HEAD
pxui_types::ffi::PxuiPickerRowConfig struct @HEAD
pxui_types::ffi::PxuiPickerRowConfig::art field u64 @U2a.4
pxui_types::ffi::PxuiPickerRowConfig::art_kind field u8 @U2a.4
pxui_types::ffi::PxuiPickerRowConfig::current field u8 @HEAD
pxui_types::ffi::PxuiPickerRowConfig::detail field Str @HEAD
pxui_types::ffi::PxuiPickerRowConfig::has_head field u8 @U2b.4.15
pxui_types::ffi::PxuiPickerRowConfig::has_image field u8 @U2b.4.15
pxui_types::ffi::PxuiPickerRowConfig::head field PxuiBox @U2b.4.15
pxui_types::ffi::PxuiPickerRowConfig::image field Image @U2b.4.15
pxui_types::ffi::PxuiPickerRowConfig::doomed field u8 @HEAD
pxui_types::ffi::PxuiPickerRowConfig::has_swatch field u8 @HEAD
pxui_types::ffi::PxuiPickerRowConfig::index field i64 @HEAD
pxui_types::ffi::PxuiPickerRowConfig::label field Str @HEAD
pxui_types::ffi::PxuiPickerRowConfig::marked field u8 @HEAD
pxui_types::ffi::PxuiPickerRowConfig::query field Str @HEAD
pxui_types::ffi::PxuiPickerRowConfig::reserved3 field [u8; 7] @U2a.4
pxui_types::ffi::PxuiPickerRowConfig::reserved4 field [u8; 6] @U2b.4.15
pxui_types::ffi::PxuiPickerRowConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiPickerRowConfig::size field u32 @HEAD
pxui_types::ffi::PxuiPickerRowConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiPickerRowConfig::swatch field Rgba @HEAD
pxui_types::ffi::PxuiPickerRowConfig::unavailable field u8 @HEAD
pxui_types::ffi::PxuiPickerRowConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiPlatformOut type = PlatformOut @HEAD
pxui_types::ffi::PxuiPopupDesc impl Clone @HEAD
pxui_types::ffi::PxuiPopupDesc impl Copy @HEAD
pxui_types::ffi::PxuiPopupDesc struct @HEAD
pxui_types::ffi::PxuiPopupDesc::attached field u8 @HEAD
pxui_types::ffi::PxuiPopupDesc::dismiss_initial field u8 @HEAD
pxui_types::ffi::PxuiPopupDesc::has_max_height field u8 @HEAD
pxui_types::ffi::PxuiPopupDesc::has_stroke field u8 @HEAD
pxui_types::ffi::PxuiPopupDesc::height field f32 @HEAD
pxui_types::ffi::PxuiPopupDesc::label field Str @HEAD
pxui_types::ffi::PxuiPopupDesc::max_height field f32 @HEAD
pxui_types::ffi::PxuiPopupDesc::size field u32 @HEAD
pxui_types::ffi::PxuiPopupDesc::stroke field Rgba @HEAD
pxui_types::ffi::PxuiPopupDesc::width field f32 @HEAD
pxui_types::ffi::PxuiPopupDesc::x field f32 @HEAD
pxui_types::ffi::PxuiPopupDesc::y field f32 @HEAD
pxui_types::ffi::PxuiProgressConfig impl Clone @HEAD
pxui_types::ffi::PxuiProgressConfig impl Copy @HEAD
pxui_types::ffi::PxuiProgressConfig struct @HEAD
pxui_types::ffi::PxuiProgressConfig::has_value field u8 @HEAD
pxui_types::ffi::PxuiProgressConfig::label field Str @HEAD
pxui_types::ffi::PxuiProgressConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiProgressConfig::size field u32 @HEAD
pxui_types::ffi::PxuiProgressConfig::value field f32 @HEAD
pxui_types::ffi::PxuiProgressConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiRange impl Clone @HEAD
pxui_types::ffi::PxuiRange impl Copy @HEAD
pxui_types::ffi::PxuiRange impl Debug @HEAD
pxui_types::ffi::PxuiRange impl Default @HEAD
pxui_types::ffi::PxuiRange struct @HEAD
pxui_types::ffi::PxuiRange::end field i64 @HEAD
pxui_types::ffi::PxuiRange::start field i64 @HEAD
pxui_types::ffi::PxuiResponse impl Clone @HEAD
pxui_types::ffi::PxuiResponse impl Copy @HEAD
pxui_types::ffi::PxuiResponse impl Debug @HEAD
pxui_types::ffi::PxuiResponse struct @HEAD
pxui_types::ffi::PxuiResponse::ACTIVATED const u32 @HEAD
pxui_types::ffi::PxuiResponse::CLICKED const u32 @HEAD
pxui_types::ffi::PxuiResponse::EMPTY const PxuiResponse @HEAD
pxui_types::ffi::PxuiResponse::FOCUSED const u32 @HEAD
pxui_types::ffi::PxuiResponse::HELD const u32 @HEAD
pxui_types::ffi::PxuiResponse::HOVERED const u32 @HEAD
pxui_types::ffi::PxuiResponse::SECONDARY const u32 @HEAD
pxui_types::ffi::PxuiResponse::flags field u32 @HEAD
pxui_types::ffi::PxuiResponse::node field PxuiBox @HEAD
pxui_types::ffi::PxuiResponse::size field u32 @HEAD
pxui_types::ffi::PxuiRewrite impl Clone @HEAD
pxui_types::ffi::PxuiRewrite impl Copy @HEAD
pxui_types::ffi::PxuiRewrite impl Debug @HEAD
pxui_types::ffi::PxuiRewrite struct @HEAD
pxui_types::ffi::PxuiRewrite::caret field i64 @HEAD
pxui_types::ffi::PxuiRewrite::text field Str @HEAD
pxui_types::ffi::PxuiRingDesc impl Clone @HEAD
pxui_types::ffi::PxuiRingDesc impl Copy @HEAD
pxui_types::ffi::PxuiRingDesc impl Debug @HEAD
pxui_types::ffi::PxuiRingDesc impl Default @HEAD
pxui_types::ffi::PxuiRingDesc struct @HEAD
pxui_types::ffi::PxuiRingDesc::color field Rgba @HEAD
pxui_types::ffi::PxuiRingDesc::sides field u32 @HEAD
pxui_types::ffi::PxuiRingDesc::width field f32 @HEAD
pxui_types::ffi::PxuiRowConfig impl Clone @HEAD
pxui_types::ffi::PxuiRowConfig impl Copy @HEAD
pxui_types::ffi::PxuiRowConfig struct @HEAD
pxui_types::ffi::PxuiRowConfig::DROP_AFTER const u8 @U2b.4.27
pxui_types::ffi::PxuiRowConfig::DROP_BEFORE const u8 @U2b.4.27
pxui_types::ffi::PxuiRowConfig::DROP_NONE const u8 @U2b.4.27
pxui_types::ffi::PxuiRowConfig::DROP_OVER const u8 @U2b.4.27
pxui_types::ffi::PxuiRowConfig::content field PxuiSlot @HEAD
pxui_types::ffi::PxuiRowConfig::drop field u8 @U2b.2
pxui_types::ffi::PxuiRowConfig::index field usize @HEAD
pxui_types::ffi::PxuiRowConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiRowConfig::size field u32 @HEAD
pxui_types::ffi::PxuiRowConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiRowConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiRowKey type = Option<extern "C" fn(*mut c_void, usize) -> i64> @HEAD
-pxui_types::ffi::PxuiSaved impl Clone @U2c.3
-pxui_types::ffi::PxuiSaved impl Copy @U2c.3
-pxui_types::ffi::PxuiSaved impl Debug @U2c.3
-pxui_types::ffi::PxuiSaved impl Default @U2c.3
-pxui_types::ffi::PxuiSaved struct @U2c.3
-pxui_types::ffi::PxuiSaved::parents field usize @U2c.3
-pxui_types::ffi::PxuiSaved::reserved field u32 @U2c.3
-pxui_types::ffi::PxuiSaved::seeds field usize @U2c.3
-pxui_types::ffi::PxuiSaved::size field u32 @U2c.3
pxui_types::ffi::PxuiScrollConfig impl Clone @HEAD
pxui_types::ffi::PxuiScrollConfig impl Copy @HEAD
pxui_types::ffi::PxuiScrollConfig struct @HEAD
pxui_types::ffi::PxuiScrollConfig::content field PxuiSlot @HEAD
pxui_types::ffi::PxuiScrollConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiScrollConfig::size field u32 @HEAD
pxui_types::ffi::PxuiScrollConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiScrubOut impl Clone @HEAD
pxui_types::ffi::PxuiScrubOut impl Copy @HEAD
pxui_types::ffi::PxuiScrubOut impl Debug @HEAD
pxui_types::ffi::PxuiScrubOut impl Default @HEAD
pxui_types::ffi::PxuiScrubOut struct @HEAD
pxui_types::ffi::PxuiScrubOut::commit field f32 @HEAD
pxui_types::ffi::PxuiScrubOut::flags field u32 @HEAD
pxui_types::ffi::PxuiScrubOut::node field PxuiBox @HEAD
pxui_types::ffi::PxuiScrubOut::preview field f32 @HEAD
pxui_types::ffi::PxuiScrubOut::size field u32 @HEAD
pxui_types::ffi::PxuiSearchFieldConfig impl Clone @HEAD
pxui_types::ffi::PxuiSearchFieldConfig impl Copy @HEAD
pxui_types::ffi::PxuiSearchFieldConfig struct @HEAD
pxui_types::ffi::PxuiSearchFieldConfig::clear field PxuiStateDesc @U2b.4.23
pxui_types::ffi::PxuiSearchFieldConfig::editor field PxuiSlot @HEAD
pxui_types::ffi::PxuiSearchFieldConfig::icon field Str @HEAD
pxui_types::ffi::PxuiSearchFieldConfig::key field i64 @HEAD
pxui_types::ffi::PxuiSearchFieldConfig::line field PxuiBox @U2b.4.8
pxui_types::ffi::PxuiSearchFieldConfig::size field u32 @HEAD
pxui_types::ffi::PxuiSearchFieldConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiSearchFieldConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiSearchFieldConfig::well field PxuiBox @HEAD
-pxui_types::ffi::PxuiSectionBuild type = Option<extern "C" fn(*mut c_void, *mut c_void, usize, f32)> @U2b.4.10
pxui_types::ffi::PxuiSectionConfig impl Clone @HEAD
pxui_types::ffi::PxuiSectionConfig impl Copy @HEAD
pxui_types::ffi::PxuiSectionConfig struct @HEAD
pxui_types::ffi::PxuiSectionConfig::content field PxuiSlot @HEAD
pxui_types::ffi::PxuiSectionConfig::gap field f32 @HEAD
pxui_types::ffi::PxuiSectionConfig::label field Str @HEAD
pxui_types::ffi::PxuiSectionConfig::open field u8 @HEAD
pxui_types::ffi::PxuiSectionConfig::size field u32 @HEAD
pxui_types::ffi::PxuiSectionConfig::title field PxuiBox @HEAD
pxui_types::ffi::PxuiSectionConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiSectionIndexConfig impl Clone @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig impl Copy @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig struct @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig::active field usize @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig::has_active field u8 @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig::letters field Slice<Str> @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig::node field PxuiBox @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig::size field u32 @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig::state field PxuiStateDesc @U2a.3
pxui_types::ffi::PxuiSectionIndexConfig::variant field u16 @U2a.3
pxui_types::ffi::PxuiSectionListDesc impl Clone @HEAD
pxui_types::ffi::PxuiSectionListDesc impl Copy @HEAD
pxui_types::ffi::PxuiSectionListDesc impl Debug @U2b.1
pxui_types::ffi::PxuiSectionListDesc struct @HEAD
-pxui_types::ffi::PxuiSectionListDesc::build field PxuiGridBuild @U2b.1
pxui_types::ffi::PxuiSectionListDesc::count field usize @HEAD
pxui_types::ffi::PxuiSectionListDesc::header_height field f32 @HEAD
-pxui_types::ffi::PxuiSectionListDesc::key field PxuiRowKey @U2b.1
-pxui_types::ffi::PxuiSectionListDesc::label field Str @U2b.1
pxui_types::ffi::PxuiSectionListDesc::leading_height field f32 @HEAD
pxui_types::ffi::PxuiSectionListDesc::measured field u8 @U2b.1
pxui_types::ffi::PxuiSectionListDesc::overlay_scrollbar field u8 @HEAD
pxui_types::ffi::PxuiSectionListDesc::reorder field u8 @U2b.2
-pxui_types::ffi::PxuiSectionListDesc::reserved field [u8; 3] @U2b.1
pxui_types::ffi::PxuiSectionListDesc::reserved field u8 @U2b.1
pxui_types::ffi::PxuiSectionListDesc::row_height field f32 @HEAD
pxui_types::ffi::PxuiSectionListDesc::size field u32 @HEAD
pxui_types::ffi::PxuiSectionListDesc::trailing_height field f32 @HEAD
-pxui_types::ffi::PxuiSectionListDesc::user field *mut c_void @U2b.1
pxui_types::ffi::PxuiSegmentedConfig impl Clone @U5.7
pxui_types::ffi::PxuiSegmentedConfig impl Copy @U5.7
pxui_types::ffi::PxuiSegmentedConfig struct @U5.7
pxui_types::ffi::PxuiSegmentedDesc impl Clone @HEAD
pxui_types::ffi::PxuiSegmentedDesc impl Copy @HEAD
pxui_types::ffi::PxuiSegmentedDesc struct @HEAD
pxui_types::ffi::PxuiSegmentedDesc::enabled field u8 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::focus_on_click field u8 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::grow field u8 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::h field f32 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::has_selected field u8 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::key field Str @HEAD
pxui_types::ffi::PxuiSegmentedDesc::min_segment field f32 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::name field Str @HEAD
pxui_types::ffi::PxuiSegmentedDesc::names field Slice<Str> @HEAD
pxui_types::ffi::PxuiSegmentedDesc::padding field f32 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::reserved field [u8; 2] @HEAD
pxui_types::ffi::PxuiSegmentedDesc::reserved2 field u16 @U5.3
pxui_types::ffi::PxuiSegmentedDesc::reserved3 field [u8; 7] @HEAD
pxui_types::ffi::PxuiSegmentedDesc::reserved4 field [u8; 7] @HEAD
pxui_types::ffi::PxuiSegmentedDesc::selected field usize @HEAD
pxui_types::ffi::PxuiSegmentedDesc::size field u32 @HEAD
pxui_types::ffi::PxuiSegmentedDesc::states field *const PxuiLookDesc @HEAD
pxui_types::ffi::PxuiSegmentedDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiSegmentedDesc::w field f32 @HEAD
pxui_types::ffi::PxuiSegmentedResult impl Clone @HEAD
pxui_types::ffi::PxuiSegmentedResult impl Copy @HEAD
pxui_types::ffi::PxuiSegmentedResult impl Debug @HEAD
pxui_types::ffi::PxuiSegmentedResult struct @HEAD
pxui_types::ffi::PxuiSegmentedResult::EMPTY const PxuiSegmentedResult @HEAD
pxui_types::ffi::PxuiSegmentedResult::chosen field u64 @HEAD
pxui_types::ffi::PxuiSegmentedResult::has_chosen field u8 @HEAD
pxui_types::ffi::PxuiSegmentedResult::has_secondary field u8 @HEAD
pxui_types::ffi::PxuiSegmentedResult::reserved field [u8; 2] @HEAD
pxui_types::ffi::PxuiSegmentedResult::secondary field u64 @HEAD
pxui_types::ffi::PxuiSegmentedResult::size field u32 @HEAD
pxui_types::ffi::PxuiSelectionListDesc impl Clone @U2b.1
pxui_types::ffi::PxuiSelectionListDesc impl Copy @U2b.1
pxui_types::ffi::PxuiSelectionListDesc struct @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::anchor field usize @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::bare field u8 @U2b.4.3
pxui_types::ffi::PxuiSelectionListDesc::count field usize @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::drop_look field PxuiLookDesc @U2b.2
pxui_types::ffi::PxuiSelectionListDesc::fit field u8 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::has_drop_look field u8 @U2b.2
pxui_types::ffi::PxuiSelectionListDesc::has_revision field u8 @U2b.4.13
pxui_types::ffi::PxuiSelectionListDesc::header_height field f32 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::leading_height field f32 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::measured field u8 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::mode field u8 @U2b.4.33
-pxui_types::ffi::PxuiSelectionListDesc::multi field u8 @U2b.4.33
pxui_types::ffi::PxuiSelectionListDesc::open_on_click field u8 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::overlay_scrollbar field u8 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::reorder field u8 @U2b.2
-pxui_types::ffi::PxuiSelectionListDesc::reserved field u8 @U2b.4.3
pxui_types::ffi::PxuiSelectionListDesc::reserved field [u8; 3] @U2b.4.13
pxui_types::ffi::PxuiSelectionListDesc::revision field u64 @U2b.4.13
pxui_types::ffi::PxuiSelectionListDesc::row_height field f32 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::rows field PxuiU8MutSpan @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::size field u32 @U2b.1
pxui_types::ffi::PxuiSelectionListDesc::trailing_height field f32 @U2b.1
pxui_types::ffi::PxuiShadowDesc impl Clone @HEAD
pxui_types::ffi::PxuiShadowDesc impl Copy @HEAD
pxui_types::ffi::PxuiShadowDesc impl Debug @HEAD
pxui_types::ffi::PxuiShadowDesc impl Default @HEAD
pxui_types::ffi::PxuiShadowDesc struct @HEAD
pxui_types::ffi::PxuiShadowDesc::blur field f32 @HEAD
pxui_types::ffi::PxuiShadowDesc::color field Rgba @HEAD
pxui_types::ffi::PxuiShadowDesc::inner field u8 @HEAD
pxui_types::ffi::PxuiShadowDesc::offset field [f32; 2] @HEAD
pxui_types::ffi::PxuiShadowDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiShadowDesc::spread field f32 @HEAD
pxui_types::ffi::PxuiSheetDesc impl Clone @U2a.4
pxui_types::ffi::PxuiSheetDesc impl Copy @U2a.4
pxui_types::ffi::PxuiSheetDesc impl Debug @U2a.4
pxui_types::ffi::PxuiSheetDesc struct @U2a.4
pxui_types::ffi::PxuiSheetDesc::dismiss_fraction field f32 @U2a.4
pxui_types::ffi::PxuiSheetDesc::dismiss_velocity field f32 @U2a.4
pxui_types::ffi::PxuiSheetDesc::has_width field u8 @U2a.4
pxui_types::ffi::PxuiSheetDesc::max_h field f32 @U2b.5
pxui_types::ffi::PxuiSheetDesc::present field u8 @U2a.4
pxui_types::ffi::PxuiSheetDesc::reserved field [u8; 2] @U2a.4
pxui_types::ffi::PxuiSheetDesc::size field u32 @U2a.4
pxui_types::ffi::PxuiSheetDesc::top field f32 @U2b.5
pxui_types::ffi::PxuiSheetDesc::width field f32 @U2a.4
pxui_types::ffi::PxuiSignal impl Clone @HEAD
pxui_types::ffi::PxuiSignal impl Copy @HEAD
pxui_types::ffi::PxuiSignal impl Debug @HEAD
pxui_types::ffi::PxuiSignal impl Default @HEAD
pxui_types::ffi::PxuiSignal struct @HEAD
pxui_types::ffi::PxuiSignal::CLICKED const u32 @HEAD
pxui_types::ffi::PxuiSignal::DOUBLE_CLICKED const u32 @HEAD
pxui_types::ffi::PxuiSignal::DRAGGING const u32 @HEAD
pxui_types::ffi::PxuiSignal::HELD const u32 @HEAD
pxui_types::ffi::PxuiSignal::HOVERED const u32 @HEAD
pxui_types::ffi::PxuiSignal::PRESSED const u32 @HEAD
pxui_types::ffi::PxuiSignal::RELEASED const u32 @HEAD
pxui_types::ffi::PxuiSignal::a11y_select field [u32; 2] @HEAD
pxui_types::ffi::PxuiSignal::a11y_value field f64 @HEAD
pxui_types::ffi::PxuiSignal::button field u8 @HEAD
pxui_types::ffi::PxuiSignal::clicks field i64 @HEAD
pxui_types::ffi::PxuiSignal::drag field [f32; 2] @HEAD
pxui_types::ffi::PxuiSignal::flags field u32 @HEAD
pxui_types::ffi::PxuiSignal::has_a11y_select field u8 @HEAD
pxui_types::ffi::PxuiSignal::has_a11y_value field u8 @HEAD
pxui_types::ffi::PxuiSignal::has_subtree_press field u8 @HEAD
pxui_types::ffi::PxuiSignal::keys field Strided<Event> @U5.4
pxui_types::ffi::PxuiSignal::pinch field f64 @HEAD
pxui_types::ffi::PxuiSignal::pointer field [f32; 2] @HEAD
pxui_types::ffi::PxuiSignal::press_point field [f32; 2] @HEAD
pxui_types::ffi::PxuiSignal::release_point field [f32; 2] @HEAD
pxui_types::ffi::PxuiSignal::reserved field [u8; 4] @HEAD
pxui_types::ffi::PxuiSignal::scroll field [f32; 2] @HEAD
pxui_types::ffi::PxuiSignal::size field u32 @HEAD
pxui_types::ffi::PxuiSignal::subtree_press field i64 @HEAD
pxui_types::ffi::PxuiSizeDesc impl Clone @HEAD
pxui_types::ffi::PxuiSizeDesc impl Copy @HEAD
pxui_types::ffi::PxuiSizeDesc impl Debug @HEAD
pxui_types::ffi::PxuiSizeDesc impl Default @HEAD
pxui_types::ffi::PxuiSizeDesc struct @HEAD
pxui_types::ffi::PxuiSizeDesc::FIT const u32 @HEAD
pxui_types::ffi::PxuiSizeDesc::GROW const u32 @HEAD
pxui_types::ffi::PxuiSizeDesc::PCT const u32 @HEAD
pxui_types::ffi::PxuiSizeDesc::PX const u32 @HEAD
pxui_types::ffi::PxuiSizeDesc::REL const u32 @HEAD
pxui_types::ffi::PxuiSizeDesc::kind field u32 @HEAD
pxui_types::ffi::PxuiSizeDesc::relative field Option<extern "C" fn(f32) -> f32> @HEAD
pxui_types::ffi::PxuiSizeDesc::value field f32 @HEAD
pxui_types::ffi::PxuiSliderConfig impl Clone @HEAD
pxui_types::ffi::PxuiSliderConfig impl Copy @HEAD
pxui_types::ffi::PxuiSliderConfig struct @HEAD
pxui_types::ffi::PxuiSliderConfig::fraction field f32 @HEAD
pxui_types::ffi::PxuiSliderConfig::label field Str @HEAD
pxui_types::ffi::PxuiSliderConfig::size field u32 @HEAD
pxui_types::ffi::PxuiSliderConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiSliderConfig::track field PxuiBox @HEAD
pxui_types::ffi::PxuiSliderConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiSlot impl Clone @HEAD
pxui_types::ffi::PxuiSlot impl Copy @HEAD
pxui_types::ffi::PxuiSlot impl Debug @HEAD
pxui_types::ffi::PxuiSlot impl Default @HEAD
pxui_types::ffi::PxuiSlot impl Eq @HEAD
pxui_types::ffi::PxuiSlot impl PartialEq @HEAD
pxui_types::ffi::PxuiSlot impl StructuralPartialEq @HEAD
pxui_types::ffi::PxuiSlot struct @HEAD
pxui_types::ffi::PxuiSlot::serial field u64 @HEAD
pxui_types::ffi::PxuiSpinnerConfig impl Clone @HEAD
pxui_types::ffi::PxuiSpinnerConfig impl Copy @HEAD
pxui_types::ffi::PxuiSpinnerConfig struct @HEAD
pxui_types::ffi::PxuiSpinnerConfig::label field Str @HEAD
pxui_types::ffi::PxuiSpinnerConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiSpinnerConfig::size field u32 @HEAD
pxui_types::ffi::PxuiSpinnerConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiSplice impl Clone @HEAD
pxui_types::ffi::PxuiSplice impl Copy @HEAD
pxui_types::ffi::PxuiSplice impl Debug @HEAD
pxui_types::ffi::PxuiSplice struct @HEAD
pxui_types::ffi::PxuiSplice::end field i64 @HEAD
pxui_types::ffi::PxuiSplice::start field i64 @HEAD
pxui_types::ffi::PxuiSplice::text field Str @HEAD
pxui_types::ffi::PxuiSplitConfig impl Clone @HEAD
pxui_types::ffi::PxuiSplitConfig impl Copy @HEAD
pxui_types::ffi::PxuiSplitConfig struct @HEAD
pxui_types::ffi::PxuiSplitConfig::axis field u32 @HEAD
pxui_types::ffi::PxuiSplitConfig::first field PxuiSlot @HEAD
pxui_types::ffi::PxuiSplitConfig::min_a field f32 @HEAD
pxui_types::ffi::PxuiSplitConfig::min_b field f32 @HEAD
pxui_types::ffi::PxuiSplitConfig::ratio field f32 @HEAD
pxui_types::ffi::PxuiSplitConfig::second field PxuiSlot @HEAD
pxui_types::ffi::PxuiSplitConfig::size field u32 @HEAD
pxui_types::ffi::PxuiSplitConfig::split field PxuiBox @HEAD
pxui_types::ffi::PxuiSplitConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiSplitDesc struct @HEAD
pxui_types::ffi::PxuiSplitDesc::axis field u32 @HEAD
pxui_types::ffi::PxuiSplitDesc::build_a field PxuiBuild @HEAD
pxui_types::ffi::PxuiSplitDesc::build_b field PxuiBuild @HEAD
pxui_types::ffi::PxuiSplitDesc::label field Str @HEAD
pxui_types::ffi::PxuiSplitDesc::min_a field f32 @HEAD
pxui_types::ffi::PxuiSplitDesc::min_b field f32 @HEAD
pxui_types::ffi::PxuiSplitDesc::ratio field f32 @HEAD
pxui_types::ffi::PxuiSplitDesc::reserved field u32 @HEAD
pxui_types::ffi::PxuiSplitDesc::size field u32 @HEAD
pxui_types::ffi::PxuiSplitDesc::user_a field *mut c_void @HEAD
pxui_types::ffi::PxuiSplitDesc::user_b field *mut c_void @HEAD
pxui_types::ffi::PxuiStateDesc impl Clone @HEAD
pxui_types::ffi::PxuiStateDesc impl Copy @HEAD
pxui_types::ffi::PxuiStateDesc impl Debug @HEAD
pxui_types::ffi::PxuiStateDesc impl Default @HEAD
pxui_types::ffi::PxuiStateDesc impl From<PxuiStateDesc> @U4b
pxui_types::ffi::PxuiStateDesc struct @HEAD
pxui_types::ffi::PxuiStateDesc::checked field u8 @HEAD
pxui_types::ffi::PxuiStateDesc::disabled field u8 @HEAD
pxui_types::ffi::PxuiStateDesc::focused field u8 @HEAD
pxui_types::ffi::PxuiStateDesc::hovered field u8 @HEAD
pxui_types::ffi::PxuiStateDesc::pressed field u8 @HEAD
pxui_types::ffi::PxuiStateDesc::reserved field [u8; 3] @HEAD
pxui_types::ffi::PxuiStateDesc::size field u32 @U5.4
pxui_types::ffi::PxuiSwipeIn impl Clone @HEAD
pxui_types::ffi::PxuiSwipeIn impl Copy @HEAD
pxui_types::ffi::PxuiSwipeIn impl Debug @HEAD
pxui_types::ffi::PxuiSwipeIn impl Default @HEAD
pxui_types::ffi::PxuiSwipeIn struct @HEAD
pxui_types::ffi::PxuiSwipeIn::full field f32 @HEAD
pxui_types::ffi::PxuiSwipeIn::leading field f32 @HEAD
pxui_types::ffi::PxuiSwipeIn::size field u32 @HEAD
pxui_types::ffi::PxuiSwipeIn::trailing field f32 @HEAD
-pxui_types::ffi::PxuiTableCell type = Option<extern "C" fn(*mut c_void, i64, i64) -> Str> @U2b.1
pxui_types::ffi::PxuiTableCellBuild type = Option<extern "C" fn(*mut c_void, *mut c_void, usize, usize, *const PxuiCellState)> @U2b.1
-pxui_types::ffi::PxuiTableCellConfig impl Clone @U2b.1
-pxui_types::ffi::PxuiTableCellConfig impl Copy @U2b.1
-pxui_types::ffi::PxuiTableCellConfig struct @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::column field i64 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::header field u8 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::height field f32 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::key field i64 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::left field f32 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::right field f32 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::row field i64 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::size field u32 @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::text field Str @U2b.1
-pxui_types::ffi::PxuiTableCellConfig::variant field u16 @U2b.1
pxui_types::ffi::PxuiTableDesc impl Clone @U2b.1
pxui_types::ffi::PxuiTableDesc impl Copy @U2b.1
pxui_types::ffi::PxuiTableDesc struct @HEAD
pxui_types::ffi::PxuiTableDesc::anchor field usize @U2b.1
pxui_types::ffi::PxuiTableDesc::ascending field u8 @U2b.1
pxui_types::ffi::PxuiTableDesc::bare field u8 @U2b.4.37
-pxui_types::ffi::PxuiTableDesc::at field [f32; 2] @U2b.1
-pxui_types::ffi::PxuiTableDesc::cell field PxuiTableCell @U2b.1
pxui_types::ffi::PxuiTableDesc::columns field Slice<PxuiColumn> @U2b.1
pxui_types::ffi::PxuiTableDesc::flags field PxuiU8MutSpan @U2b.1
pxui_types::ffi::PxuiTableDesc::has_revision field u8 @U2b.4.13
pxui_types::ffi::PxuiTableDesc::has_sort field u8 @U2b.1
pxui_types::ffi::PxuiTableDesc::head_h field f32 @U2b.1
pxui_types::ffi::PxuiTableDesc::head_variant field u16 @U2b.1
-pxui_types::ffi::PxuiTableDesc::headers field Slice<Str> @U2b.1
-pxui_types::ffi::PxuiTableDesc::height field f32 @U2b.1
-pxui_types::ffi::PxuiTableDesc::label field Str @U2b.1
pxui_types::ffi::PxuiTableDesc::mode field u8 @U2b.4.37
-pxui_types::ffi::PxuiTableDesc::multi field u8 @U2b.4.37
pxui_types::ffi::PxuiTableDesc::open_on_click field u8 @U2b.4.37
-pxui_types::ffi::PxuiTableDesc::reserved field u32 @U2b.1
-pxui_types::ffi::PxuiTableDesc::reserved field u8 @U2b.4.13
pxui_types::ffi::PxuiTableDesc::reserved2 field [u8; 4] @U2b.4.37
-pxui_types::ffi::PxuiTableDesc::reserved2 field [u8; 6] @U2b.4.37
pxui_types::ffi::PxuiTableDesc::revision field u64 @U2b.4.13
pxui_types::ffi::PxuiTableDesc::row_h field f32 @U2b.1
-pxui_types::ffi::PxuiTableDesc::rows field i64 @U2b.1
pxui_types::ffi::PxuiTableDesc::rows field usize @U2b.1
pxui_types::ffi::PxuiTableDesc::size field u32 @HEAD
pxui_types::ffi::PxuiTableDesc::sort_id field i64 @U2b.1
-pxui_types::ffi::PxuiTableDesc::user field *mut c_void @U2b.1
-pxui_types::ffi::PxuiTableDesc::width field f32 @U2b.1
pxui_types::ffi::PxuiTableDesc::widths field PxuiColumnWidthSpan @U2b.1
pxui_types::ffi::PxuiTableHeadConfig impl Clone @HEAD
pxui_types::ffi::PxuiTableHeadConfig impl Copy @HEAD
pxui_types::ffi::PxuiTableHeadConfig struct @HEAD
pxui_types::ffi::PxuiTableHeadConfig::ascending field u8 @HEAD
pxui_types::ffi::PxuiTableHeadConfig::built_in field PxuiSlot @U2b.4.36
pxui_types::ffi::PxuiTableHeadConfig::has_ascending field u8 @HEAD
pxui_types::ffi::PxuiTableHeadConfig::index field usize @HEAD
pxui_types::ffi::PxuiTableHeadConfig::node field PxuiBox @HEAD
pxui_types::ffi::PxuiTableHeadConfig::size field u32 @HEAD
pxui_types::ffi::PxuiTableHeadConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiTableHeadConfig::title field Str @HEAD
pxui_types::ffi::PxuiTableHeadConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiTableHeadDesc impl Clone @U2b.1
pxui_types::ffi::PxuiTableHeadDesc impl Copy @U2b.1
pxui_types::ffi::PxuiTableHeadDesc impl Debug @U2b.1
pxui_types::ffi::PxuiTableHeadDesc struct @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::ascending field u8 @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::columns field Slice<PxuiColumn> @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::h field f32 @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::has_sort field u8 @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::reserved field [u8; 2] @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::reserved2 field u32 @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::size field u32 @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::sort_id field i64 @U2b.1
pxui_types::ffi::PxuiTableHeadDesc::widths field PxuiColumnWidthSpan @U2b.1
pxui_types::ffi::PxuiTableResult impl Clone @U2b.1
pxui_types::ffi::PxuiTableResult impl Copy @U2b.1
pxui_types::ffi::PxuiTableResult impl Debug @U2b.1
pxui_types::ffi::PxuiTableResult struct @U2b.1
pxui_types::ffi::PxuiTableResult::head field PxuiHeadResult @U2b.1
pxui_types::ffi::PxuiTableResult::reserved field u32 @U2b.1
pxui_types::ffi::PxuiTableResult::rows field PxuiListResult @U2b.1
pxui_types::ffi::PxuiTableResult::size field u32 @U2b.1
-pxui_types::ffi::PxuiTableViewDesc struct @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::ascending field u8 @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::has_sort field u8 @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::label field Str @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::reserved field [u8; 2] @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::rows field usize @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::selected field *mut usize @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::size field u32 @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::sort_column field usize @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::text field PxuiCollectionText @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::titles field Slice<Str> @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::user field *mut c_void @U2b.1
-pxui_types::ffi::PxuiTableViewDesc::widths field PxuiF32MutSpan @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc struct @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::ascending field u8 @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::build field PxuiCellBuild @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::has_sort field u8 @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::label field Str @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::reserved field [u8; 2] @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::rows field usize @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::selected field *mut usize @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::size field u32 @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::sort_column field usize @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::titles field Slice<Str> @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::user field *mut c_void @U2b.1
-pxui_types::ffi::PxuiTableViewWithDesc::widths field PxuiF32MutSpan @U2b.1
pxui_types::ffi::PxuiTabsConfig impl Clone @U5.7
pxui_types::ffi::PxuiTabsConfig impl Copy @U5.7
pxui_types::ffi::PxuiTabsConfig struct @U5.7
pxui_types::ffi::PxuiTextAreaDesc impl Clone @HEAD
pxui_types::ffi::PxuiTextAreaDesc impl Copy @HEAD
pxui_types::ffi::PxuiTextAreaDesc impl Debug @HEAD
pxui_types::ffi::PxuiTextAreaDesc struct @HEAD
pxui_types::ffi::PxuiTextAreaDesc::HAS_LANGUAGE const u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::HAS_REVEAL const u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::HAS_REWRITE const u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::HAS_SELECT const u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::KNOWN const u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::WRAP const u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::chips field Slice<PxuiColorSpan> @HEAD
pxui_types::ffi::PxuiTextAreaDesc::comment field Str @HEAD
pxui_types::ffi::PxuiTextAreaDesc::errors field Slice<i64> @HEAD
pxui_types::ffi::PxuiTextAreaDesc::flags field u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::height field f32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::label field Str @HEAD
pxui_types::ffi::PxuiTextAreaDesc::messages field Slice<PxuiNote> @HEAD
pxui_types::ffi::PxuiTextAreaDesc::pairs field Slice<u32> @HEAD
pxui_types::ffi::PxuiTextAreaDesc::reveal field i64 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::reveal_nonce field i64 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::select field PxuiRange @HEAD
pxui_types::ffi::PxuiTextAreaDesc::size field u32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::spans field Slice<PxuiRange> @HEAD
pxui_types::ffi::PxuiTextAreaDesc::splice field Slice<PxuiSplice> @HEAD
pxui_types::ffi::PxuiTextAreaDesc::value field Str @HEAD
pxui_types::ffi::PxuiTextAreaDesc::width field f32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::x field f32 @HEAD
pxui_types::ffi::PxuiTextAreaDesc::y field f32 @HEAD
pxui_types::ffi::PxuiTextFieldConfig impl Clone @HEAD
pxui_types::ffi::PxuiTextFieldConfig impl Copy @HEAD
pxui_types::ffi::PxuiTextFieldConfig struct @HEAD
pxui_types::ffi::PxuiTextFieldConfig::label field Str @HEAD
pxui_types::ffi::PxuiTextFieldConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiTextFieldConfig::size field u32 @HEAD
pxui_types::ffi::PxuiTextFieldConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiTextFieldConfig::value field Str @HEAD
pxui_types::ffi::PxuiTextFieldConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiTextFieldDesc impl Clone @HEAD
pxui_types::ffi::PxuiTextFieldDesc impl Copy @HEAD
pxui_types::ffi::PxuiTextFieldDesc struct @HEAD
pxui_types::ffi::PxuiTextFieldDesc::action field u8 @HEAD
pxui_types::ffi::PxuiTextFieldDesc::autocorrect field u8 @HEAD
pxui_types::ffi::PxuiTextFieldDesc::capitalization field u8 @HEAD
pxui_types::ffi::PxuiTextFieldDesc::key field Str @HEAD
pxui_types::ffi::PxuiTextFieldDesc::name field Str @HEAD
pxui_types::ffi::PxuiTextFieldDesc::reserved field u8 @HEAD
pxui_types::ffi::PxuiTextFieldDesc::reserved_variant field [u8; 6] @U5.3
pxui_types::ffi::PxuiTextFieldDesc::size field u32 @HEAD
pxui_types::ffi::PxuiTextFieldDesc::value field Str @HEAD
pxui_types::ffi::PxuiTextFieldDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiTextStyle impl Clone @HEAD
pxui_types::ffi::PxuiTextStyle impl Copy @HEAD
pxui_types::ffi::PxuiTextStyle impl Debug @HEAD
pxui_types::ffi::PxuiTextStyle impl Default @HEAD
pxui_types::ffi::PxuiTextStyle impl PartialEq @HEAD
pxui_types::ffi::PxuiTextStyle impl StructuralPartialEq @HEAD
pxui_types::ffi::PxuiTextStyle struct @HEAD
pxui_types::ffi::PxuiTextStyle::FEATURE_TABULAR_NUMBERS const u32 @HEAD
pxui_types::ffi::PxuiTextStyle::align field u32 @HEAD
pxui_types::ffi::PxuiTextStyle::ellipsis field u8 @HEAD
pxui_types::ffi::PxuiTextStyle::features field u32 @HEAD
pxui_types::ffi::PxuiTextStyle::font field u16 @HEAD
pxui_types::ffi::PxuiTextStyle::italic field u8 @HEAD
pxui_types::ffi::PxuiTextStyle::line_height field f32 @HEAD
pxui_types::ffi::PxuiTextStyle::max_lines field u16 @HEAD
pxui_types::ffi::PxuiTextStyle::size field u32 @HEAD
pxui_types::ffi::PxuiTextStyle::size_points field f32 @HEAD
pxui_types::ffi::PxuiTextStyle::tracking field f32 @HEAD
pxui_types::ffi::PxuiTextStyle::weight field u16 @HEAD
pxui_types::ffi::PxuiTextStyle::wrap field u32 @HEAD
-pxui_types::ffi::PxuiThemeSource type = ThemeSource @U2c.5
pxui_types::ffi::PxuiThemeTokens type = ThemeTokens @HEAD
pxui_types::ffi::PxuiToastsConfig impl Clone @U5.7
pxui_types::ffi::PxuiToastsConfig impl Copy @U5.7
pxui_types::ffi::PxuiToastsConfig struct @U5.7
pxui_types::ffi::PxuiToggleConfig impl Clone @HEAD
pxui_types::ffi::PxuiToggleConfig impl Copy @HEAD
pxui_types::ffi::PxuiToggleConfig struct @HEAD
pxui_types::ffi::PxuiToggleConfig::clicked field u8 @HEAD
pxui_types::ffi::PxuiToggleConfig::detail field Str @HEAD
pxui_types::ffi::PxuiToggleConfig::has_style field u8 @HEAD
pxui_types::ffi::PxuiToggleConfig::inset field f32 @HEAD
pxui_types::ffi::PxuiToggleConfig::knob field *const States @HEAD
pxui_types::ffi::PxuiToggleConfig::label field Str @HEAD
pxui_types::ffi::PxuiToggleConfig::on field u8 @HEAD
pxui_types::ffi::PxuiToggleConfig::press_row field u8 @HEAD
pxui_types::ffi::PxuiToggleConfig::reserved2 field [u8; 5] @HEAD
pxui_types::ffi::PxuiToggleConfig::reserved3 field [u8; 6] @HEAD
pxui_types::ffi::PxuiToggleConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiToggleConfig::row_h field f32 @HEAD
pxui_types::ffi::PxuiToggleConfig::size field u32 @HEAD
pxui_types::ffi::PxuiToggleConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiToggleConfig::stored field u8 @HEAD
pxui_types::ffi::PxuiToggleConfig::style field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiToggleConfig::switch_h field f32 @HEAD
pxui_types::ffi::PxuiToggleConfig::switch_w field f32 @HEAD
pxui_types::ffi::PxuiToggleConfig::track field *const States @HEAD
pxui_types::ffi::PxuiToggleConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiToggleDesc impl Clone @HEAD
pxui_types::ffi::PxuiToggleDesc impl Copy @HEAD
pxui_types::ffi::PxuiToggleDesc struct @HEAD
pxui_types::ffi::PxuiToggleDesc::detail field Str @HEAD
pxui_types::ffi::PxuiToggleDesc::enabled field u8 @HEAD
pxui_types::ffi::PxuiToggleDesc::has_style field u8 @HEAD
pxui_types::ffi::PxuiToggleDesc::inset field f32 @HEAD
pxui_types::ffi::PxuiToggleDesc::key field Str @HEAD
pxui_types::ffi::PxuiToggleDesc::knob field *const PxuiLookDesc @HEAD
pxui_types::ffi::PxuiToggleDesc::on field u8 @HEAD
pxui_types::ffi::PxuiToggleDesc::press_row field u8 @HEAD
pxui_types::ffi::PxuiToggleDesc::reserved field u8 @HEAD
pxui_types::ffi::PxuiToggleDesc::reserved2 field u8 @U5.3
pxui_types::ffi::PxuiToggleDesc::reserved3 field [u8; 4] @U5.3
pxui_types::ffi::PxuiToggleDesc::row_h field f32 @HEAD
pxui_types::ffi::PxuiToggleDesc::size field u32 @HEAD
pxui_types::ffi::PxuiToggleDesc::style field PxuiLabelStyle @HEAD
pxui_types::ffi::PxuiToggleDesc::switch_h field f32 @HEAD
pxui_types::ffi::PxuiToggleDesc::switch_w field f32 @HEAD
pxui_types::ffi::PxuiToggleDesc::text field Str @HEAD
pxui_types::ffi::PxuiToggleDesc::track field *const PxuiLookDesc @HEAD
pxui_types::ffi::PxuiToggleDesc::variant field u16 @U5.3
pxui_types::ffi::PxuiTooltipConfig impl Clone @U5.7
pxui_types::ffi::PxuiTooltipConfig impl Copy @U5.7
pxui_types::ffi::PxuiTooltipConfig struct @U5.7
pxui_types::ffi::PxuiTreeNode impl Clone @HEAD
pxui_types::ffi::PxuiTreeNode impl Copy @HEAD
pxui_types::ffi::PxuiTreeNode impl Default @HEAD
pxui_types::ffi::PxuiTreeNode struct @HEAD
pxui_types::ffi::PxuiTreeNode::children field u8 @HEAD
pxui_types::ffi::PxuiTreeNode::depth field u16 @HEAD
pxui_types::ffi::PxuiTreeRowConfig impl Clone @HEAD
pxui_types::ffi::PxuiTreeRowConfig impl Copy @HEAD
pxui_types::ffi::PxuiTreeRowConfig struct @HEAD
pxui_types::ffi::PxuiTreeRowConfig::content field PxuiSlot @HEAD
pxui_types::ffi::PxuiTreeRowConfig::depth field usize @HEAD
pxui_types::ffi::PxuiTreeRowConfig::has field u8 @HEAD
pxui_types::ffi::PxuiTreeRowConfig::index field usize @HEAD
pxui_types::ffi::PxuiTreeRowConfig::node field usize @HEAD
pxui_types::ffi::PxuiTreeRowConfig::open field u8 @HEAD
pxui_types::ffi::PxuiTreeRowConfig::row field PxuiBox @HEAD
pxui_types::ffi::PxuiTreeRowConfig::size field u32 @HEAD
pxui_types::ffi::PxuiTreeRowConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiTreeRowConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiTreeViewDesc struct @HEAD
pxui_types::ffi::PxuiTreeViewDesc::build field PxuiCellBuild @U2c.2
pxui_types::ffi::PxuiTreeViewDesc::has_revision field u8 @U2b.4.17
pxui_types::ffi::PxuiTreeViewDesc::label field Str @HEAD
pxui_types::ffi::PxuiTreeViewDesc::nodes field Slice<PxuiTreeNode> @HEAD
pxui_types::ffi::PxuiTreeViewDesc::open field PxuiU8MutSpan @HEAD
-pxui_types::ffi::PxuiTreeViewDesc::reserved field u32 @HEAD
pxui_types::ffi::PxuiTreeViewDesc::reserved field [u8; 3] @U2b.4.17
pxui_types::ffi::PxuiTreeViewDesc::revision field u64 @U2b.4.17
pxui_types::ffi::PxuiTreeViewDesc::selected field *mut usize @HEAD
pxui_types::ffi::PxuiTreeViewDesc::size field u32 @HEAD
-pxui_types::ffi::PxuiTreeViewDesc::text field PxuiCollectionText @U2c.2
pxui_types::ffi::PxuiTreeViewDesc::user field *mut c_void @HEAD
pxui_types::ffi::PxuiTreeViewDesc::visible field PxuiU32MutSpan @HEAD
pxui_types::ffi::PxuiTreeViewDesc::visible_len field *mut usize @HEAD
-pxui_types::ffi::PxuiTreeViewWithDesc struct @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::build field PxuiCellBuild @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::label field Str @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::nodes field Slice<PxuiTreeNode> @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::open field PxuiU8MutSpan @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::reserved field u32 @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::selected field *mut usize @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::size field u32 @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::user field *mut c_void @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::visible field PxuiU32MutSpan @U2c.2
-pxui_types::ffi::PxuiTreeViewWithDesc::visible_len field *mut usize @U2c.2
pxui_types::ffi::PxuiU32MutSpan impl Clone @HEAD
pxui_types::ffi::PxuiU32MutSpan impl Copy @HEAD
pxui_types::ffi::PxuiU32MutSpan impl Default @HEAD
pxui_types::ffi::PxuiU32MutSpan struct @HEAD
pxui_types::ffi::PxuiU32MutSpan::len field usize @HEAD
pxui_types::ffi::PxuiU32MutSpan::ptr field *mut u32 @HEAD
pxui_types::ffi::PxuiU8MutSpan impl Clone @HEAD
pxui_types::ffi::PxuiU8MutSpan impl Copy @HEAD
pxui_types::ffi::PxuiU8MutSpan impl Default @HEAD
pxui_types::ffi::PxuiU8MutSpan struct @HEAD
pxui_types::ffi::PxuiU8MutSpan::len field usize @HEAD
pxui_types::ffi::PxuiU8MutSpan::ptr field *mut u8 @HEAD
pxui_types::ffi::PxuiValueFieldConfig impl Clone @HEAD
pxui_types::ffi::PxuiValueFieldConfig impl Copy @HEAD
pxui_types::ffi::PxuiValueFieldConfig struct @HEAD
pxui_types::ffi::PxuiValueFieldConfig::field field PxuiBox @HEAD
pxui_types::ffi::PxuiValueFieldConfig::h field f32 @HEAD
pxui_types::ffi::PxuiValueFieldConfig::invalid field u8 @HEAD
pxui_types::ffi::PxuiValueFieldConfig::opts field PxuiValueFieldDesc @HEAD
pxui_types::ffi::PxuiValueFieldConfig::size field u32 @HEAD
pxui_types::ffi::PxuiValueFieldConfig::state field PxuiStateDesc @HEAD
pxui_types::ffi::PxuiValueFieldConfig::text field Str @HEAD
pxui_types::ffi::PxuiValueFieldConfig::variant field u16 @HEAD
pxui_types::ffi::PxuiValueFieldConfig::w field f32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc impl Clone @HEAD
pxui_types::ffi::PxuiValueFieldDesc impl Copy @HEAD
pxui_types::ffi::PxuiValueFieldDesc struct @HEAD
pxui_types::ffi::PxuiValueFieldDesc::BARE const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::EDIT const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_DISPLAY const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_FRACTION const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_LEAD const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_LINE const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_PLACEHOLDER const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_SIZE const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_TRACKING const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::HAS_TRAIL const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::KEPT_DRAG const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::KNOWN const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::LEFT const u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::display field Str @HEAD
pxui_types::ffi::PxuiValueFieldDesc::flags field u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::font_size field i64 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::fraction field f32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::height field f32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::label field Str @HEAD
pxui_types::ffi::PxuiValueFieldDesc::lead field Str @HEAD
pxui_types::ffi::PxuiValueFieldDesc::lead_color field Rgba @HEAD
pxui_types::ffi::PxuiValueFieldDesc::line field Rgba @HEAD
pxui_types::ffi::PxuiValueFieldDesc::placeholder field Str @HEAD
pxui_types::ffi::PxuiValueFieldDesc::reserved field [u8; 4] @HEAD
pxui_types::ffi::PxuiValueFieldDesc::size field u32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::tracking field f32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::trail field Str @HEAD
pxui_types::ffi::PxuiValueFieldDesc::trail_color field Rgba @HEAD
pxui_types::ffi::PxuiValueFieldDesc::value field Str @HEAD
pxui_types::ffi::PxuiValueFieldDesc::width field f32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::x field f32 @HEAD
pxui_types::ffi::PxuiValueFieldDesc::y field f32 @HEAD
pxui_types::ffi::Slice impl Clone @HEAD
pxui_types::ffi::Slice impl Copy @HEAD
pxui_types::ffi::Slice impl Debug @HEAD
pxui_types::ffi::Slice impl Default @HEAD
pxui_types::ffi::Slice impl From<&'static [u8]> @U1b.5
pxui_types::ffi::Slice struct <T> @HEAD
pxui_types::ffi::Slice::len field usize @HEAD
pxui_types::ffi::Slice::ptr field *const T @HEAD
pxui_types::ffi::Strided impl Clone @U5.4
pxui_types::ffi::Strided impl Copy @U5.4
pxui_types::ffi::Strided impl Debug @U5.4
pxui_types::ffi::Strided impl Default @U5.4
pxui_types::ffi::Strided struct <T> @U5.4
pxui_types::ffi::Strided::as_slice fn unsafe <'a>(&self) -> Option<&'a [T]> where T: 'a @U5.4
pxui_types::ffi::Strided::at fn unsafe <'a>(&self, index: usize) -> &'a T where T: 'a @U5.4
pxui_types::ffi::Strided::is_valid fn (&self) -> bool @U5.4
pxui_types::ffi::Strided::len field usize @U5.4
pxui_types::ffi::Strided::of fn const (records: &[T]) -> Self @U5.4
pxui_types::ffi::Strided::ptr field *const T @U5.4
pxui_types::ffi::Strided::stride field usize @U5.4
-pxui_types::ffi::ThemeSource impl Clone @U2c.5
-pxui_types::ffi::ThemeSource impl Copy @U2c.5
-pxui_types::ffi::ThemeSource impl Debug @U2c.5
-pxui_types::ffi::ThemeSource impl Default @U2c.5
-pxui_types::ffi::ThemeSource struct @U2c.5
-pxui_types::ffi::ThemeSource::COMPACT const u32 @U2c.5
-pxui_types::ffi::ThemeSource::REDUCED_MOTION const u32 @U2c.5
-pxui_types::ffi::ThemeSource::flags field u32 @U2c.5
-pxui_types::ffi::ThemeSource::size field u32 @U2c.5
-pxui_types::ffi::ThemeSource::source field Str @U2c.5
pxui_types::ffi::ThemeTokens impl Clone @HEAD
pxui_types::ffi::ThemeTokens impl Copy @HEAD
pxui_types::ffi::ThemeTokens impl Debug @HEAD
pxui_types::ffi::ThemeTokens impl From<ThemeTokens> @HEAD
pxui_types::ffi::ThemeTokens impl From<Tokens> @HEAD
pxui_types::ffi::ThemeTokens struct @HEAD
pxui_types::ffi::ThemeTokens::accent field Rgba @HEAD
pxui_types::ffi::ThemeTokens::control field Rgba @HEAD
pxui_types::ffi::ThemeTokens::dark field u8 @U5.6
pxui_types::ffi::ThemeTokens::density field f32 @HEAD
pxui_types::ffi::ThemeTokens::display_size field i64 @HEAD
pxui_types::ffi::ThemeTokens::font_size field i64 @HEAD
pxui_types::ffi::ThemeTokens::foreground field Rgba @HEAD
pxui_types::ffi::ThemeTokens::has_ink_ground field u8 @HEAD
pxui_types::ffi::ThemeTokens::ink_ground field Rgba @HEAD
pxui_types::ffi::ThemeTokens::input field Rgba @HEAD
pxui_types::ffi::ThemeTokens::label_size field i64 @HEAD
pxui_types::ffi::ThemeTokens::motion field PxuiMotion @HEAD
pxui_types::ffi::ThemeTokens::panel field Rgba @HEAD
pxui_types::ffi::ThemeTokens::radius field [f32; 3] @HEAD
pxui_types::ffi::ThemeTokens::reduced_motion field u8 @HEAD
pxui_types::ffi::ThemeTokens::reserved field [u8; 3] @HEAD
pxui_types::ffi::ThemeTokens::reserved_tail field [u8; 2] @U5.6
pxui_types::ffi::ThemeTokens::size field u32 @HEAD
pxui_types::ffi::ThemeTokens::space field [f32; 4] @HEAD
pxui_types::ffi::ThemeTokens::title_size field i64 @HEAD
pxui_types::ffi::ThemeTokens::track field Rgba @HEAD
pxui_types::ffi::UiBuild type = Option<extern "C" fn(*mut c_void, *mut c_void)> @HEAD
pxui_types::geom mod @HEAD
pxui_types::geom::ColorParseError enum @HEAD
pxui_types::geom::ColorParseError impl Clone @HEAD
pxui_types::geom::ColorParseError impl Copy @HEAD
pxui_types::geom::ColorParseError impl Debug @HEAD
pxui_types::geom::ColorParseError impl Display @HEAD
pxui_types::geom::ColorParseError impl Eq @HEAD
pxui_types::geom::ColorParseError impl Error @HEAD
pxui_types::geom::ColorParseError impl PartialEq @HEAD
pxui_types::geom::ColorParseError impl StructuralPartialEq @HEAD
pxui_types::geom::ColorParseError::Digit variant @HEAD
pxui_types::geom::ColorParseError::Length variant @HEAD
pxui_types::geom::Edges impl Clone @HEAD
pxui_types::geom::Edges impl Copy @HEAD
pxui_types::geom::Edges impl Debug @HEAD
pxui_types::geom::Edges impl Default @HEAD
pxui_types::geom::Edges impl PartialEq @HEAD
pxui_types::geom::Edges impl StructuralPartialEq @HEAD
pxui_types::geom::Edges struct @HEAD
pxui_types::geom::Edges::ZERO const Edges @HEAD
pxui_types::geom::Edges::all fn const (v: f32) -> Edges @HEAD
pxui_types::geom::Edges::b field f32 @HEAD
pxui_types::geom::Edges::b fn const (self, points: f32) -> Edges @U1b.5
pxui_types::geom::Edges::l field f32 @HEAD
pxui_types::geom::Edges::l fn const (self, points: f32) -> Edges @U1b.5
pxui_types::geom::Edges::new fn const (l: f32, t: f32, r: f32, b: f32) -> Edges @U1b.5
pxui_types::geom::Edges::r field f32 @HEAD
pxui_types::geom::Edges::r fn const (self, points: f32) -> Edges @U1b.5
pxui_types::geom::Edges::t field f32 @HEAD
pxui_types::geom::Edges::t fn const (self, points: f32) -> Edges @U1b.5
pxui_types::geom::Edges::xy fn const (x: f32, y: f32) -> Edges @HEAD
pxui_types::geom::Point impl Clone @HEAD
pxui_types::geom::Point impl Copy @HEAD
pxui_types::geom::Point impl Debug @HEAD
pxui_types::geom::Point impl Default @HEAD
pxui_types::geom::Point impl PartialEq @HEAD
pxui_types::geom::Point impl StructuralPartialEq @HEAD
pxui_types::geom::Point struct @HEAD
pxui_types::geom::Point::new fn const (x: f32, y: f32) -> Point @U1b.3
pxui_types::geom::Point::x field f32 @HEAD
pxui_types::geom::Point::y field f32 @HEAD
pxui_types::geom::Rect impl Clone @HEAD
pxui_types::geom::Rect impl Copy @HEAD
pxui_types::geom::Rect impl Debug @HEAD
pxui_types::geom::Rect impl Default @HEAD
pxui_types::geom::Rect impl PartialEq @HEAD
pxui_types::geom::Rect impl StructuralPartialEq @HEAD
pxui_types::geom::Rect struct @HEAD
pxui_types::geom::Rect::contains fn (self, p: (f32, f32)) -> bool @HEAD
pxui_types::geom::Rect::f64 fn (self) -> (f64, f64, f64, f64) @HEAD
pxui_types::geom::Rect::h field f32 @HEAD
pxui_types::geom::Rect::intersect fn (self, o: Rect) -> Rect @HEAD
pxui_types::geom::Rect::new fn const (x: f32, y: f32, w: f32, h: f32) -> Rect @HEAD
pxui_types::geom::Rect::w field f32 @HEAD
pxui_types::geom::Rect::x field f32 @HEAD
pxui_types::geom::Rect::y field f32 @HEAD
pxui_types::geom::Rgba impl Clone @HEAD
pxui_types::geom::Rgba impl Copy @HEAD
pxui_types::geom::Rgba impl Debug @HEAD
pxui_types::geom::Rgba impl Default @HEAD
pxui_types::geom::Rgba impl Eq @HEAD
pxui_types::geom::Rgba impl PartialEq @HEAD
pxui_types::geom::Rgba impl StructuralPartialEq @HEAD
pxui_types::geom::Rgba struct @HEAD
pxui_types::geom::Rgba::BLACK const Rgba @HEAD
pxui_types::geom::Rgba::CLEAR const Rgba @HEAD
pxui_types::geom::Rgba::WHITE const Rgba @HEAD
pxui_types::geom::Rgba::a field u8 @HEAD
pxui_types::geom::Rgba::alpha fn const (self, alpha: u8) -> Rgba @U2b.7
pxui_types::geom::Rgba::as_token fn const (self) -> Option<ColorToken> @HEAD
pxui_types::geom::Rgba::b field u8 @HEAD
pxui_types::geom::Rgba::blend fn (self, to: Rgba, t: f64) -> Rgba @HEAD
pxui_types::geom::Rgba::g field u8 @HEAD
pxui_types::geom::Rgba::gray fn const (v: u8) -> Rgba @HEAD
pxui_types::geom::Rgba::hex fn const (rgb: u32) -> Rgba @HEAD
pxui_types::geom::Rgba::parse fn (text: &str) -> Result<Rgba, ColorParseError> @HEAD
pxui_types::geom::Rgba::r field u8 @HEAD
pxui_types::geom::Rgba::rgb fn const (r: u8, g: u8, b: u8) -> Rgba @HEAD
pxui_types::geom::Rgba::rgba fn const (r: u8, g: u8, b: u8, a: u8) -> Rgba @HEAD
pxui_types::geom::Rgba::to_hex_string fn (self) -> String @HEAD
pxui_types::geom::Rgba::token fn const (token: ColorToken) -> Rgba @HEAD
pxui_types::geom::Rgba::with_alpha fn const (self, alpha: u8) -> Rgba @HEAD
pxui_types::geom::hash mod @HEAD
pxui_types::geom::hash::FastHasher impl Clone @HEAD
pxui_types::geom::hash::FastHasher impl Copy @HEAD
pxui_types::geom::hash::FastHasher impl Default @HEAD
pxui_types::geom::hash::FastHasher impl Hasher @HEAD
pxui_types::geom::hash::FastHasher struct (_) @HEAD
pxui_types::geom::hash::bytes fn (seed: u64, bytes: &[u8]) -> u64 @HEAD
pxui_types::geom::hash::mix fn (hash: i64, value: i64) -> i64 @HEAD
pxui_types::layout mod @HEAD
pxui_types::layout::Align enum @HEAD
pxui_types::layout::Align impl Clone @HEAD
pxui_types::layout::Align impl Copy @HEAD
pxui_types::layout::Align impl Debug @HEAD
pxui_types::layout::Align impl Default @HEAD
pxui_types::layout::Align impl Eq @HEAD
pxui_types::layout::Align impl PartialEq @HEAD
pxui_types::layout::Align impl StructuralPartialEq @HEAD
pxui_types::layout::Align::Baseline variant @HEAD
pxui_types::layout::Align::Center variant @HEAD
pxui_types::layout::Align::End variant @HEAD
pxui_types::layout::Align::Start variant @HEAD
pxui_types::layout::Align::Stretch variant @HEAD
pxui_types::layout::Along enum @HEAD
pxui_types::layout::Along impl Clone @HEAD
pxui_types::layout::Along impl Copy @HEAD
pxui_types::layout::Along impl Debug @HEAD
pxui_types::layout::Along impl Default @HEAD
pxui_types::layout::Along impl Eq @HEAD
pxui_types::layout::Along impl PartialEq @HEAD
pxui_types::layout::Along impl StructuralPartialEq @HEAD
pxui_types::layout::Along::Center variant @HEAD
pxui_types::layout::Along::End variant @HEAD
pxui_types::layout::Along::Start variant @HEAD
pxui_types::layout::Anchor impl Clone @HEAD
pxui_types::layout::Anchor impl Copy @HEAD
pxui_types::layout::Anchor impl Debug @HEAD
pxui_types::layout::Anchor impl PartialEq @HEAD
pxui_types::layout::Anchor impl StructuralPartialEq @HEAD
pxui_types::layout::Anchor struct @HEAD
pxui_types::layout::Anchor::new fn const (rect: Rect, placement: Placement) -> Anchor @HEAD
pxui_types::layout::Anchor::placement field Placement @HEAD
pxui_types::layout::Anchor::rect field Rect @HEAD
pxui_types::layout::Cell impl Clone @HEAD
pxui_types::layout::Cell impl Copy @HEAD
pxui_types::layout::Cell impl Debug @HEAD
pxui_types::layout::Cell impl Eq @HEAD
pxui_types::layout::Cell impl PartialEq @HEAD
pxui_types::layout::Cell impl StructuralPartialEq @HEAD
pxui_types::layout::Cell struct @HEAD
pxui_types::layout::Cell::AUTO const u16 @HEAD
pxui_types::layout::Cell::DEFAULT const Cell @HEAD
pxui_types::layout::Cell::at fn const (col: u16, row: u16) -> Cell @HEAD
pxui_types::layout::Cell::col field u16 @HEAD
pxui_types::layout::Cell::col_span field u16 @HEAD
pxui_types::layout::Cell::row field u16 @HEAD
pxui_types::layout::Cell::row_span field u16 @HEAD
pxui_types::layout::Cell::span fn const (self, col_span: u16, row_span: u16) -> Cell @HEAD
pxui_types::layout::Direction enum @HEAD
pxui_types::layout::Direction impl Clone @HEAD
pxui_types::layout::Direction impl Copy @HEAD
pxui_types::layout::Direction impl Debug @HEAD
pxui_types::layout::Direction impl Default @HEAD
pxui_types::layout::Direction impl Eq @HEAD
pxui_types::layout::Direction impl PartialEq @HEAD
pxui_types::layout::Direction impl StructuralPartialEq @HEAD
pxui_types::layout::Direction::Ltr variant @HEAD
pxui_types::layout::Direction::Rtl variant @HEAD
pxui_types::layout::Grid impl Clone @HEAD
pxui_types::layout::Grid impl Copy @HEAD
pxui_types::layout::Grid impl Debug @HEAD
pxui_types::layout::Grid impl PartialEq @HEAD
pxui_types::layout::Grid struct @HEAD
pxui_types::layout::Grid::cols fn (&self) -> &'static [Track] @HEAD
pxui_types::layout::Grid::new fn const (cols: &'static [Track], rows: &'static [Track]) -> Grid @HEAD
pxui_types::layout::Grid::raw fn (&self) -> (*const Track, usize, *const Track, usize) @HEAD
pxui_types::layout::Grid::rows fn (&self) -> &'static [Track] @HEAD
pxui_types::layout::Justify enum @HEAD
pxui_types::layout::Justify impl Clone @HEAD
pxui_types::layout::Justify impl Copy @HEAD
pxui_types::layout::Justify impl Debug @HEAD
pxui_types::layout::Justify impl Default @HEAD
pxui_types::layout::Justify impl Eq @HEAD
pxui_types::layout::Justify impl PartialEq @HEAD
pxui_types::layout::Justify impl StructuralPartialEq @HEAD
pxui_types::layout::Justify::Center variant @HEAD
pxui_types::layout::Justify::End variant @HEAD
pxui_types::layout::Justify::SpaceAround variant @HEAD
pxui_types::layout::Justify::SpaceBetween variant @HEAD
pxui_types::layout::Justify::SpaceEvenly variant @HEAD
pxui_types::layout::Justify::Start variant @HEAD
pxui_types::layout::MAX_TRACKS const usize @HEAD
pxui_types::layout::Placement impl Clone @HEAD
pxui_types::layout::Placement impl Copy @HEAD
pxui_types::layout::Placement impl Debug @HEAD
pxui_types::layout::Placement impl PartialEq @HEAD
pxui_types::layout::Placement impl StructuralPartialEq @HEAD
pxui_types::layout::Placement struct @HEAD
pxui_types::layout::Placement::BELOW const Placement @HEAD
pxui_types::layout::Placement::align field Along @HEAD
pxui_types::layout::Placement::flip field bool @HEAD
pxui_types::layout::Placement::gap field f32 @HEAD
pxui_types::layout::Placement::margin field f32 @HEAD
pxui_types::layout::Placement::side field Side @HEAD
pxui_types::layout::Side enum @HEAD
pxui_types::layout::Side impl Clone @HEAD
pxui_types::layout::Side impl Copy @HEAD
pxui_types::layout::Side impl Debug @HEAD
pxui_types::layout::Side impl Eq @HEAD
pxui_types::layout::Side impl PartialEq @HEAD
pxui_types::layout::Side impl StructuralPartialEq @HEAD
pxui_types::layout::Side::Above variant @HEAD
pxui_types::layout::Side::Below variant @HEAD
pxui_types::layout::Side::Left variant @HEAD
pxui_types::layout::Side::Right variant @HEAD
pxui_types::layout::Track enum @HEAD
pxui_types::layout::Track impl Clone @HEAD
pxui_types::layout::Track impl Copy @HEAD
pxui_types::layout::Track impl Debug @HEAD
pxui_types::layout::Track impl PartialEq @HEAD
pxui_types::layout::Track impl StructuralPartialEq @HEAD
pxui_types::layout::Track::Fit variant @HEAD
pxui_types::layout::Track::Fr variant (f32) @HEAD
pxui_types::layout::Track::Px variant (f32) @HEAD
pxui_types::layout::place_anchored fn (anchor: Rect, size: (f32, f32), p: Placement, bounds: Rect) -> Rect @HEAD
pxui_types::node mod @HEAD
pxui_types::node::Axis enum @HEAD
pxui_types::node::Axis impl Clone @HEAD
pxui_types::node::Axis impl Copy @HEAD
pxui_types::node::Axis impl Debug @HEAD
pxui_types::node::Axis impl Eq @HEAD
pxui_types::node::Axis impl PartialEq @HEAD
pxui_types::node::Axis impl StructuralPartialEq @HEAD
pxui_types::node::Axis::Column variant @HEAD
pxui_types::node::Axis::Row variant @HEAD
pxui_types::node::Axis::Stack variant @HEAD
pxui_types::node::Bar impl Clone @HEAD
pxui_types::node::Bar impl Copy @HEAD
pxui_types::node::Bar impl Debug @HEAD
pxui_types::node::Bar impl PartialEq @HEAD
pxui_types::node::Bar impl StructuralPartialEq @HEAD
pxui_types::node::Bar struct @HEAD
pxui_types::node::Bar::color field Rgba @HEAD
pxui_types::node::Bar::gap field f32 @HEAD
pxui_types::node::Bar::gap fn const (self, points: f32) -> Bar @HEAD
pxui_types::node::Bar::margin field f32 @HEAD
pxui_types::node::Bar::margin fn const (self, points: f32) -> Bar @HEAD
pxui_types::node::Bar::new fn const (color: Rgba, width: f32) -> Bar @HEAD
pxui_types::node::Bar::radius field f32 @HEAD
pxui_types::node::Bar::radius fn const (self, points: f32) -> Bar @HEAD
pxui_types::node::Bar::whole field bool @HEAD
pxui_types::node::Bar::whole fn const (self) -> Bar @HEAD
pxui_types::node::Bar::width field f32 @HEAD
pxui_types::node::Dir enum @HEAD
pxui_types::node::Dir impl Clone @HEAD
pxui_types::node::Dir impl Copy @HEAD
pxui_types::node::Dir impl Debug @HEAD
pxui_types::node::Dir impl Eq @HEAD
pxui_types::node::Dir impl PartialEq @HEAD
pxui_types::node::Dir impl StructuralPartialEq @HEAD
pxui_types::node::Dir::Down variant @HEAD
pxui_types::node::Dir::Left variant @HEAD
pxui_types::node::Dir::Right variant @HEAD
pxui_types::node::Dir::Up variant @HEAD
pxui_types::node::Mark enum @HEAD
pxui_types::node::Mark impl Clone @HEAD
pxui_types::node::Mark impl Copy @HEAD
pxui_types::node::Mark impl Debug @HEAD
pxui_types::node::Mark impl PartialEq @HEAD
pxui_types::node::Mark impl StructuralPartialEq @HEAD
pxui_types::node::Mark::Chevron variant (Dir) @HEAD
pxui_types::node::Mark::Line variant { width: f32, rising: bool } @HEAD
pxui_types::node::Mark::None variant @HEAD
pxui_types::node::Node impl Clone @HEAD
pxui_types::node::Node impl Copy @HEAD
pxui_types::node::Node impl Debug @HEAD
pxui_types::node::Node struct @HEAD
pxui_types::node::Node::DEFAULT const Node @HEAD
pxui_types::node::Node::a11y field u32 @HEAD
pxui_types::node::Node::a11y fn const (self, states: u32) -> Node @HEAD
pxui_types::node::Node::align field Align @HEAD
pxui_types::node::Node::align fn const (self, align: Align) -> Node @HEAD
pxui_types::node::Node::align_self field OptAlign @HEAD
pxui_types::node::Node::align_self fn const (self, align: Align) -> Node @HEAD
pxui_types::node::Node::anchor field OptAnchor @HEAD
pxui_types::node::Node::anchor fn const (self, anchor: Anchor) -> Node @HEAD
pxui_types::node::Node::aspect field f32 @HEAD
pxui_types::node::Node::aspect fn const (self, ratio: f32) -> Node @HEAD
pxui_types::node::Node::at field OptPoint @HEAD
pxui_types::node::Node::at fn const (self, x: f32, y: f32) -> Node @HEAD
pxui_types::node::Node::axis field Axis @HEAD
pxui_types::node::Node::axis fn const (self, axis: Axis) -> Node @HEAD
pxui_types::node::Node::bar field OptBar @HEAD
pxui_types::node::Node::bar fn const (self, bar: Bar) -> Node @HEAD
pxui_types::node::Node::cell field Cell @HEAD
pxui_types::node::Node::cell fn const (self, cell: Cell) -> Node @HEAD
pxui_types::node::Node::center fn const (self) -> Node @HEAD
pxui_types::node::Node::column fn const () -> Node @HEAD
pxui_types::node::Node::flags field Flags @HEAD
pxui_types::node::Node::flags fn const (self, flags: Flags) -> Node @HEAD
pxui_types::node::Node::gap field f32 @HEAD
pxui_types::node::Node::gap fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::grid field OptGrid @HEAD
pxui_types::node::Node::grid fn const (self, grid: Grid) -> Node @HEAD
pxui_types::node::Node::grow field f32 @HEAD
pxui_types::node::Node::grow fn const (self, share: f32) -> Node @HEAD
pxui_types::node::Node::h field Size @HEAD
pxui_types::node::Node::h fn const (self, size: Size) -> Node @HEAD
pxui_types::node::Node::h_px fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::inset field Edges @HEAD
pxui_types::node::Node::inset fn const (self, edges: Edges) -> Node @HEAD
pxui_types::node::Node::justify field Justify @HEAD
pxui_types::node::Node::justify fn const (self, justify: Justify) -> Node @HEAD
pxui_types::node::Node::look field Look @HEAD
pxui_types::node::Node::look fn const (self, look: Look) -> Node @HEAD
pxui_types::node::Node::margin field Edges @HEAD
pxui_types::node::Node::margin fn const (self, edges: Edges) -> Node @HEAD
pxui_types::node::Node::mark field Mark @HEAD
pxui_types::node::Node::mark fn const (self, mark: Mark) -> Node @HEAD
pxui_types::node::Node::max_h field f32 @HEAD
pxui_types::node::Node::max_h fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::max_w field f32 @HEAD
pxui_types::node::Node::max_w fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::min_h field f32 @HEAD
pxui_types::node::Node::min_h fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::min_w field f32 @HEAD
pxui_types::node::Node::min_w fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::pad fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::pad_x fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::pad_y fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::padding field Edges @HEAD
pxui_types::node::Node::padding fn const (self, edges: Edges) -> Node @HEAD
pxui_types::node::Node::role field Role @HEAD
pxui_types::node::Node::role fn const (self, role: Role) -> Node @HEAD
pxui_types::node::Node::row fn const () -> Node @HEAD
pxui_types::node::Node::row_gap field f32 @HEAD
pxui_types::node::Node::row_gap fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::scroll_step field f32 @HEAD
pxui_types::node::Node::scroll_step fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::shrink field f32 @HEAD
pxui_types::node::Node::shrink fn const (self, factor: f32) -> Node @HEAD
pxui_types::node::Node::stack fn const () -> Node @HEAD
pxui_types::node::Node::state field State @HEAD
pxui_types::node::Node::state fn const (self, state: State) -> Node @U1b.5
pxui_types::node::Node::transition field OptTransition @HEAD
pxui_types::node::Node::transition fn const (self, transition: Option<Transition>) -> Node @U1b.5
pxui_types::node::Node::w field Size @HEAD
pxui_types::node::Node::w fn const (self, size: Size) -> Node @HEAD
pxui_types::node::Node::w_px fn const (self, points: f32) -> Node @HEAD
pxui_types::node::Node::wrap field bool @HEAD
pxui_types::node::Node::wrap fn const (self) -> Node @HEAD
pxui_types::node::Node::z field i16 @HEAD
pxui_types::node::Node::z fn const (self, order: i16) -> Node @HEAD
pxui_types::node::OptAlign enum @HEAD
pxui_types::node::OptAlign impl Clone @HEAD
pxui_types::node::OptAlign impl Copy @HEAD
pxui_types::node::OptAlign impl Debug @HEAD
pxui_types::node::OptAlign impl Default @HEAD
pxui_types::node::OptAlign impl From<Option<Align>> @HEAD
pxui_types::node::OptAlign impl PartialEq @HEAD
pxui_types::node::OptAlign impl StructuralPartialEq @HEAD
pxui_types::node::OptAlign::None variant @HEAD
pxui_types::node::OptAlign::Some variant (Align) @HEAD
pxui_types::node::OptAlign::get fn const (self) -> Option<Align> @HEAD
pxui_types::node::OptAlign::is_none fn const (&self) -> bool @HEAD
pxui_types::node::OptAlign::is_some fn const (&self) -> bool @HEAD
pxui_types::node::OptAnchor enum @HEAD
pxui_types::node::OptAnchor impl Clone @HEAD
pxui_types::node::OptAnchor impl Copy @HEAD
pxui_types::node::OptAnchor impl Debug @HEAD
pxui_types::node::OptAnchor impl Default @HEAD
pxui_types::node::OptAnchor impl From<Option<Anchor>> @HEAD
pxui_types::node::OptAnchor impl PartialEq @HEAD
pxui_types::node::OptAnchor impl StructuralPartialEq @HEAD
pxui_types::node::OptAnchor::None variant @HEAD
pxui_types::node::OptAnchor::Some variant (Anchor) @HEAD
pxui_types::node::OptAnchor::get fn const (self) -> Option<Anchor> @HEAD
pxui_types::node::OptAnchor::is_none fn const (&self) -> bool @HEAD
pxui_types::node::OptAnchor::is_some fn const (&self) -> bool @HEAD
pxui_types::node::OptBar enum @HEAD
pxui_types::node::OptBar impl Clone @HEAD
pxui_types::node::OptBar impl Copy @HEAD
pxui_types::node::OptBar impl Debug @HEAD
pxui_types::node::OptBar impl Default @HEAD
pxui_types::node::OptBar impl From<Option<Bar>> @HEAD
pxui_types::node::OptBar impl PartialEq @HEAD
pxui_types::node::OptBar impl StructuralPartialEq @HEAD
pxui_types::node::OptBar::None variant @HEAD
pxui_types::node::OptBar::Some variant (Bar) @HEAD
pxui_types::node::OptBar::get fn const (self) -> Option<Bar> @HEAD
pxui_types::node::OptBar::is_none fn const (&self) -> bool @HEAD
pxui_types::node::OptBar::is_some fn const (&self) -> bool @HEAD
pxui_types::node::OptColor enum @HEAD
pxui_types::node::OptColor impl Clone @HEAD
pxui_types::node::OptColor impl Copy @HEAD
pxui_types::node::OptColor impl Debug @HEAD
pxui_types::node::OptColor impl Default @HEAD
pxui_types::node::OptColor impl From<Option<Rgba>> @HEAD
pxui_types::node::OptColor impl PartialEq @HEAD
pxui_types::node::OptColor impl StructuralPartialEq @HEAD
pxui_types::node::OptColor::None variant @HEAD
pxui_types::node::OptColor::Some variant (Rgba) @HEAD
pxui_types::node::OptColor::get fn const (self) -> Option<Rgba> @HEAD
pxui_types::node::OptColor::is_none fn const (&self) -> bool @HEAD
pxui_types::node::OptColor::is_some fn const (&self) -> bool @HEAD
pxui_types::node::OptGrid enum @HEAD
pxui_types::node::OptGrid impl Clone @HEAD
pxui_types::node::OptGrid impl Copy @HEAD
pxui_types::node::OptGrid impl Debug @HEAD
pxui_types::node::OptGrid impl Default @HEAD
pxui_types::node::OptGrid impl From<Option<Grid>> @HEAD
pxui_types::node::OptGrid impl PartialEq @HEAD
pxui_types::node::OptGrid impl StructuralPartialEq @HEAD
pxui_types::node::OptGrid::None variant @HEAD
pxui_types::node::OptGrid::Some variant (Grid) @HEAD
pxui_types::node::OptGrid::get fn const (self) -> Option<Grid> @HEAD
pxui_types::node::OptGrid::is_none fn const (&self) -> bool @HEAD
pxui_types::node::OptGrid::is_some fn const (&self) -> bool @HEAD
pxui_types::node::OptPoint enum @HEAD
pxui_types::node::OptPoint impl Clone @HEAD
pxui_types::node::OptPoint impl Copy @HEAD
pxui_types::node::OptPoint impl Debug @HEAD
pxui_types::node::OptPoint impl Default @HEAD
pxui_types::node::OptPoint impl From<Option<(f32, f32)>> @HEAD
pxui_types::node::OptPoint impl PartialEq @HEAD
pxui_types::node::OptPoint impl StructuralPartialEq @HEAD
pxui_types::node::OptPoint::None variant @HEAD
pxui_types::node::OptPoint::Some variant (f32, f32) @HEAD
pxui_types::node::OptPoint::get fn const (self) -> Option<(f32, f32)> @HEAD
pxui_types::node::OptPoint::is_none fn const (&self) -> bool @HEAD
pxui_types::node::OptPoint::is_some fn const (&self) -> bool @HEAD
pxui_types::node::OptTransition enum @HEAD
pxui_types::node::OptTransition impl Clone @HEAD
pxui_types::node::OptTransition impl Copy @HEAD
pxui_types::node::OptTransition impl Debug @HEAD
pxui_types::node::OptTransition impl Default @HEAD
pxui_types::node::OptTransition impl From<Option<Transition>> @HEAD
pxui_types::node::OptTransition impl PartialEq @HEAD
pxui_types::node::OptTransition impl StructuralPartialEq @HEAD
pxui_types::node::OptTransition::None variant @HEAD
pxui_types::node::OptTransition::Some variant (Transition) @HEAD
pxui_types::node::OptTransition::get fn const (self) -> Option<Transition> @HEAD
pxui_types::node::OptTransition::is_none fn const (&self) -> bool @HEAD
pxui_types::node::OptTransition::is_some fn const (&self) -> bool @HEAD
pxui_types::node::Transition enum @HEAD
pxui_types::node::Transition impl Clone @HEAD
pxui_types::node::Transition impl Copy @HEAD
pxui_types::node::Transition impl Debug @HEAD
pxui_types::node::Transition impl PartialEq @HEAD
pxui_types::node::Transition impl StructuralPartialEq @HEAD
pxui_types::node::Transition::Bezier variant { duration: f32, points: [f32; 4] } @HEAD
pxui_types::node::Transition::Spring variant { omega: f32 } @HEAD
pxui_types::node::Transition::curve fn (self) -> Curve @HEAD
pxui_types::node::Transition::of fn (curve: Curve) -> Option<Self> @HEAD
pxui_types::node::Transition::valid fn (&self) -> bool @HEAD
pxui_types::shape mod @HEAD
pxui_types::shape::MAX_DASHES const i64 @HEAD
pxui_types::shape::Shapes trait  @HEAD
pxui_types::shape::Shapes::bands fn (&mut self, _: Rect, width: f32, sides: u8, color: Rgba) @HEAD
pxui_types::shape::Shapes::brackets fn (&mut self, x: f32, y: f32, w: f32, h: f32, offset: f32, length: f32, width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::chevron fn (&mut self, at: (f32, f32), direction: Dir, width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::circle fn (&mut self, at: (f32, f32), radius: f32, fill: Option<Rgba>, stroke: Option<(f32, Rgba)>) @HEAD
pxui_types::shape::Shapes::clip_rect fn (&self) -> Rect @HEAD
pxui_types::shape::Shapes::cubic fn (&mut self, points: [(f32, f32); 4], width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::curve fn (&mut self, from: (f32, f32), control: (f32, f32), to: (f32, f32), width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::dashed fn (&mut self, from: (f32, f32), to: (f32, f32), color: Rgba) @HEAD
pxui_types::shape::Shapes::dashed_rect fn (&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::dashed_w fn (&mut self, from: (f32, f32), to: (f32, f32), width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::density fn (&self) -> i64 @HEAD
pxui_types::shape::Shapes::fill fn (&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::fill_fractional fn (&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::flag fn (&mut self, at: (f32, f32), round: bool, on: bool) @HEAD
pxui_types::shape::Shapes::frame fn (&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::frame_w fn (&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::grid fn (&mut self, x: f32, y: f32, w: f32, h: f32, origin: (f32, f32), spacing: f32, dot: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::hard fn (&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 4]) @HEAD
pxui_types::shape::Shapes::line fn (&mut self, from: (f32, f32), to: (f32, f32), color: Rgba) @HEAD
pxui_types::shape::Shapes::line_w fn (&mut self, from: (f32, f32), to: (f32, f32), width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::push fn (&mut self, instance: Instance) @HEAD
pxui_types::shape::Shapes::rect fn (&mut self, x: f32, y: f32, w: f32, h: f32, fill: Option<Rgba>, stroke: Option<Rgba>) @HEAD
pxui_types::shape::Shapes::set_clip_rect fn (&mut self, clip: Rect) @HEAD
pxui_types::shape::Shapes::snap fn (&self, v: f32) -> f32 @HEAD
pxui_types::shape::Shapes::stroke fn (&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::stroke_w fn (&mut self, x: f32, y: f32, w: f32, h: f32, width: f32, color: Rgba) @HEAD
pxui_types::shape::Shapes::theme fn (&self) -> Tokens @HEAD
pxui_types::shape::Shapes::triangle fn (&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), color: Rgba) @HEAD
pxui_types::shape::Span impl Clone @HEAD
pxui_types::shape::Span impl Copy @HEAD
pxui_types::shape::Span impl Debug @HEAD
pxui_types::shape::Span struct @HEAD
pxui_types::shape::Span::EMPTY const Span @HEAD
pxui_types::shape::Span::at field *mut Instance @HEAD
pxui_types::shape::Span::clip field [i16; 4] @HEAD
pxui_types::shape::Span::clip_radius_f16 field u16 @HEAD
pxui_types::shape::Span::end field *mut Instance @HEAD
pxui_types::shape::Span::room fn (&self) -> usize @HEAD
pxui_types::shape::Span::size field u32 @HEAD
pxui_types::shape::Span::view field u16 @HEAD
pxui_types::shape::Span::write fn unsafe (&mut self, instance: Instance) -> bool @HEAD
pxui_types::shape::clip_of fn (_: Rect) -> [i16; 4] @HEAD
pxui_types::text mod @HEAD
pxui_types::text::Ellipsis enum @HEAD
pxui_types::text::Ellipsis impl Clone @HEAD
pxui_types::text::Ellipsis impl Copy @HEAD
pxui_types::text::Ellipsis impl Debug @HEAD
pxui_types::text::Ellipsis impl Eq @HEAD
pxui_types::text::Ellipsis impl PartialEq @HEAD
pxui_types::text::Ellipsis impl StructuralPartialEq @HEAD
pxui_types::text::Ellipsis::End variant = 1 @HEAD
pxui_types::text::Ellipsis::Head variant = 3 @HEAD
pxui_types::text::Ellipsis::Middle variant = 2 @HEAD
pxui_types::text::Ellipsis::None variant = 0 @HEAD
pxui_types::text::Label impl Clone @HEAD
pxui_types::text::Label impl Copy @HEAD
pxui_types::text::Label impl Debug @HEAD
pxui_types::text::Label impl From<&Label> @HEAD
pxui_types::text::Label struct (_) @HEAD
pxui_types::text::Label::BODY const Label @HEAD
pxui_types::text::Label::CAPTION const Label @HEAD
pxui_types::text::Label::DEFAULT const Label @HEAD
pxui_types::text::Label::MONO const Label @HEAD
pxui_types::text::Label::TITLE const Label @HEAD
pxui_types::text::Label::align fn const (self, align: TextAlign) -> Label @HEAD
pxui_types::text::Label::body_variant fn const (&self) -> u16 @HEAD
pxui_types::text::Label::bold fn const (self) -> Label @HEAD
pxui_types::text::Label::caps fn const (self, on: bool) -> Label @HEAD
pxui_types::text::Label::color fn const (self, token: ColorToken) -> Label @HEAD
pxui_types::text::Label::ellipsis fn const (self) -> Label @HEAD
pxui_types::text::Label::ellipsis_head fn const (self) -> Label @HEAD
pxui_types::text::Label::ellipsis_middle fn const (self) -> Label @HEAD
pxui_types::text::Label::ellipsis_none fn const (self) -> Label @V1
pxui_types::text::Label::font fn const (self, family: u16) -> Label @HEAD
pxui_types::text::Label::grow fn const (self) -> Label @HEAD
pxui_types::text::Label::h_px fn const (self, points: f32) -> Label @HEAD
pxui_types::text::Label::line_height fn const (self, points: f32) -> Label @HEAD
pxui_types::text::Label::lines fn const (self, lines: u16) -> Label @HEAD
pxui_types::text::Label::pad fn const (self, points: f32) -> Label @HEAD
pxui_types::text::Label::role fn const (self, role: Role) -> Label @HEAD
pxui_types::text::Label::size fn const (self, points: f32) -> Label @HEAD
pxui_types::text::Label::tabular fn const (self) -> Label @HEAD
pxui_types::text::Label::tracking fn const (self, ems: f32) -> Label @HEAD
pxui_types::text::Label::variant fn const (self, variant: u16) -> Label @HEAD
pxui_types::text::Label::w fn const (self, width: Size) -> Label @HEAD
pxui_types::text::Label::w_px fn const (self, points: f32) -> Label @HEAD
pxui_types::text::Label::weight fn const (self, weight: u16) -> Label @HEAD
pxui_types::text::Label::whole_pixels fn const (self) -> Label @HEAD
pxui_types::text::Label::wrap fn const (self) -> Label @HEAD
pxui_types::text::Label::wrap_glyph fn const (self) -> Label @HEAD
pxui_types::text::Label::wrap_spaces fn const (self) -> Label @HEAD
pxui_types::text::TextAlign enum @HEAD
pxui_types::text::TextAlign impl Clone @HEAD
pxui_types::text::TextAlign impl Copy @HEAD
pxui_types::text::TextAlign impl Debug @HEAD
pxui_types::text::TextAlign impl Eq @HEAD
pxui_types::text::TextAlign impl PartialEq @HEAD
pxui_types::text::TextAlign impl StructuralPartialEq @HEAD
pxui_types::text::TextAlign::Center variant = 1 @HEAD
pxui_types::text::TextAlign::End variant = 2 @HEAD
pxui_types::text::TextAlign::Start variant = 0 @HEAD
pxui_types::text::TextGlyph impl Clone @HEAD
pxui_types::text::TextGlyph impl Copy @HEAD
pxui_types::text::TextGlyph impl Debug @HEAD
pxui_types::text::TextGlyph impl Default @HEAD
pxui_types::text::TextGlyph impl PartialEq @HEAD
pxui_types::text::TextGlyph impl StructuralPartialEq @HEAD
pxui_types::text::TextGlyph struct @HEAD
pxui_types::text::TextGlyph::atlas field u32 @HEAD
pxui_types::text::TextGlyph::rect field [f32; 4] @HEAD
pxui_types::text::TextGlyph::reserved field u32 @HEAD
pxui_types::text::TextGlyph::uv field [f32; 4] @HEAD
pxui_types::text::TextMetrics impl Clone @HEAD
pxui_types::text::TextMetrics impl Copy @HEAD
pxui_types::text::TextMetrics impl Debug @HEAD
pxui_types::text::TextMetrics impl Default @HEAD
pxui_types::text::TextMetrics impl PartialEq @HEAD
pxui_types::text::TextMetrics impl StructuralPartialEq @HEAD
pxui_types::text::TextMetrics struct @HEAD
pxui_types::text::TextMetrics::baseline field f32 @HEAD
pxui_types::text::TextMetrics::height field f32 @HEAD
pxui_types::text::TextMetrics::lines field u32 @HEAD
pxui_types::text::TextMetrics::size field u32 @U5.4
pxui_types::text::TextMetrics::width field f32 @HEAD
pxui_types::text::TextSpan impl Clone @HEAD
pxui_types::text::TextSpan impl Copy @HEAD
pxui_types::text::TextSpan impl Debug @HEAD
pxui_types::text::TextSpan impl Default @HEAD
pxui_types::text::TextSpan impl Eq @HEAD
pxui_types::text::TextSpan impl PartialEq @HEAD
pxui_types::text::TextSpan impl StructuralPartialEq @HEAD
pxui_types::text::TextSpan struct @HEAD
pxui_types::text::TextSpan::BACKGROUND const u16 @HEAD
pxui_types::text::TextSpan::COLOR const u16 @HEAD
pxui_types::text::TextSpan::UNDERLINE const u16 @HEAD
pxui_types::text::TextSpan::background field Rgba @HEAD
pxui_types::text::TextSpan::background fn const (self, background: Rgba) -> TextSpan @U1b.5
pxui_types::text::TextSpan::color field Rgba @HEAD
pxui_types::text::TextSpan::color fn const (self, color: Rgba) -> TextSpan @U1b.5
pxui_types::text::TextSpan::end field u32 @HEAD
pxui_types::text::TextSpan::flags field u16 @HEAD
pxui_types::text::TextSpan::new fn const (start: u32, end: u32) -> TextSpan @U1b.5
pxui_types::text::TextSpan::start field u32 @HEAD
pxui_types::text::TextSpan::underline field Rgba @HEAD
pxui_types::text::TextSpan::underline fn const (self, underline: Rgba) -> TextSpan @U1b.5
pxui_types::text::TextSpan::weight field u16 @HEAD
pxui_types::text::TextSpan::weight fn const (self, weight: u16) -> TextSpan @U1b.5
pxui_types::text::TextSpans impl Clone @HEAD
pxui_types::text::TextSpans impl Copy @HEAD
pxui_types::text::TextSpans impl Debug @HEAD
pxui_types::text::TextSpans impl Default @HEAD
pxui_types::text::TextSpans struct <'a> @U5.4
pxui_types::text::TextSpans::MAX const usize @HEAD
pxui_types::text::TextSpans::new fn const (spans: &'a [TextSpan]) -> Self @U5.4
pxui_types::text::TextSpans::reserved field u32 @U5.4
pxui_types::text::TextSpans::size field u32 @HEAD
pxui_types::text::TextSpans::spans field Strided<TextSpan> @U5.4
pxui_types::text::Wrap enum @HEAD
pxui_types::text::Wrap impl Clone @HEAD
pxui_types::text::Wrap impl Copy @HEAD
pxui_types::text::Wrap impl Debug @HEAD
pxui_types::text::Wrap impl Eq @HEAD
pxui_types::text::Wrap impl PartialEq @HEAD
pxui_types::text::Wrap impl StructuralPartialEq @HEAD
pxui_types::text::Wrap::Glyph variant = 2 @HEAD
pxui_types::text::Wrap::None variant = 0 @HEAD
pxui_types::text::Wrap::Word variant = 1 @HEAD
pxui_types::text::Wrap::WordSpaces variant = 3 @HEAD
pxui_types::text_edit mod @HEAD
pxui_types::text_edit::fuzzy_match fn (query: &str, text: &str) -> bool @HEAD
pxui_types::text_edit::fuzzy_score fn (query: &str, text: &str) -> Option<i32> @HEAD
pxui_types::text_edit::line_end fn (text: &str, i: usize) -> usize @HEAD
pxui_types::text_edit::line_start fn (text: &str, i: usize) -> usize @HEAD
pxui_types::text_edit::scrubbed fn (literal: &str, dx: f32, coarse: bool) -> String @HEAD
pxui_types::theme mod @HEAD
pxui_types::theme::Bodies impl Clone @HEAD
pxui_types::theme::Bodies impl Copy @HEAD
pxui_types::theme::Bodies impl Debug @HEAD
pxui_types::theme::Bodies impl Default @HEAD
pxui_types::theme::Bodies impl Eq @HEAD
pxui_types::theme::Bodies impl PartialEq @HEAD
pxui_types::theme::Bodies struct @HEAD
pxui_types::theme::Bodies::KINDS const &'static [&'static str] @HEAD
pxui_types::theme::Bodies::SLOTS const usize @U5.4
pxui_types::theme::Bodies::badge field BodyId @HEAD
pxui_types::theme::Bodies::button field BodyId @HEAD
pxui_types::theme::Bodies::checkbox field BodyId @HEAD
pxui_types::theme::Bodies::dialog field BodyId @HEAD
pxui_types::theme::Bodies::dropdown field BodyId @HEAD
pxui_types::theme::Bodies::footer field BodyId @HEAD
pxui_types::theme::Bodies::grip field BodyId @HEAD
pxui_types::theme::Bodies::icon_button field BodyId @HEAD
pxui_types::theme::Bodies::inspector_row field BodyId @HEAD
pxui_types::theme::Bodies::inspector_section field BodyId @HEAD
pxui_types::theme::Bodies::label field BodyId @HEAD
pxui_types::theme::Bodies::line_field field BodyId @HEAD
pxui_types::theme::Bodies::menu_bar field BodyId @HEAD
pxui_types::theme::Bodies::menu_item field BodyId @HEAD
pxui_types::theme::Bodies::menu_row field BodyId @HEAD
pxui_types::theme::Bodies::modal field BodyId @HEAD
pxui_types::theme::Bodies::panel field BodyId @HEAD
pxui_types::theme::Bodies::picker field BodyId @HEAD
pxui_types::theme::Bodies::picker_field field BodyId @HEAD
pxui_types::theme::Bodies::popup field BodyId @HEAD
pxui_types::theme::Bodies::progress field BodyId @HEAD
pxui_types::theme::Bodies::radio field BodyId @HEAD
pxui_types::theme::Bodies::reserved field [BodyId; 24] @U5.4
pxui_types::theme::Bodies::scroll_view field BodyId @HEAD
pxui_types::theme::Bodies::search_field field BodyId @HEAD
pxui_types::theme::Bodies::section_index field BodyId @U2a.3
pxui_types::theme::Bodies::segmented field BodyId @HEAD
pxui_types::theme::Bodies::selection_list field BodyId @HEAD
pxui_types::theme::Bodies::set fn (&mut self, kind: &str, id: BodyId) -> bool @HEAD
pxui_types::theme::Bodies::size field u32 @U5.4
pxui_types::theme::Bodies::slider field BodyId @HEAD
pxui_types::theme::Bodies::spinner field BodyId @HEAD
pxui_types::theme::Bodies::split field BodyId @HEAD
-pxui_types::theme::Bodies::table_cell field BodyId @U2b.1
pxui_types::theme::Bodies::table_head field BodyId @HEAD
pxui_types::theme::Bodies::table_row field BodyId @HEAD
pxui_types::theme::Bodies::tabs field BodyId @HEAD
pxui_types::theme::Bodies::text_field field BodyId @HEAD
pxui_types::theme::Bodies::toasts field BodyId @HEAD
pxui_types::theme::Bodies::toggle field BodyId @HEAD
pxui_types::theme::Bodies::tooltip field BodyId @HEAD
pxui_types::theme::Bodies::tree field BodyId @HEAD
pxui_types::theme::Bodies::value_field field BodyId @HEAD
pxui_types::theme::Bodies::window field BodyId @HEAD
pxui_types::theme::BodyId impl Clone @HEAD
pxui_types::theme::BodyId impl Copy @HEAD
pxui_types::theme::BodyId impl Debug @HEAD
pxui_types::theme::BodyId impl Default @HEAD
pxui_types::theme::BodyId impl Eq @HEAD
pxui_types::theme::BodyId impl PartialEq @HEAD
pxui_types::theme::BodyId impl StructuralPartialEq @HEAD
pxui_types::theme::BodyId struct (u16) @HEAD
pxui_types::theme::BodyId::DEFAULT const BodyId @HEAD
pxui_types::theme::BodyKind enum @HEAD
pxui_types::theme::BodyKind impl Clone @HEAD
pxui_types::theme::BodyKind impl Copy @HEAD
pxui_types::theme::BodyKind impl Debug @HEAD
pxui_types::theme::BodyKind impl Eq @HEAD
pxui_types::theme::BodyKind impl PartialEq @HEAD
pxui_types::theme::BodyKind impl StructuralPartialEq @HEAD
pxui_types::theme::BodyKind::badge variant @HEAD
pxui_types::theme::BodyKind::button variant @HEAD
pxui_types::theme::BodyKind::checkbox variant @HEAD
pxui_types::theme::BodyKind::dialog variant @HEAD
pxui_types::theme::BodyKind::dropdown variant @HEAD
pxui_types::theme::BodyKind::footer variant @HEAD
pxui_types::theme::BodyKind::grip variant @HEAD
pxui_types::theme::BodyKind::icon_button variant @HEAD
pxui_types::theme::BodyKind::inspector_row variant @HEAD
pxui_types::theme::BodyKind::inspector_section variant @HEAD
pxui_types::theme::BodyKind::label variant @HEAD
pxui_types::theme::BodyKind::line_field variant @HEAD
pxui_types::theme::BodyKind::menu_bar variant @HEAD
pxui_types::theme::BodyKind::menu_item variant @HEAD
pxui_types::theme::BodyKind::menu_row variant @HEAD
pxui_types::theme::BodyKind::modal variant @HEAD
pxui_types::theme::BodyKind::panel variant @HEAD
pxui_types::theme::BodyKind::picker variant @HEAD
pxui_types::theme::BodyKind::picker_field variant @HEAD
pxui_types::theme::BodyKind::popup variant @HEAD
pxui_types::theme::BodyKind::progress variant @HEAD
pxui_types::theme::BodyKind::radio variant @HEAD
pxui_types::theme::BodyKind::scroll_view variant @HEAD
pxui_types::theme::BodyKind::search_field variant @HEAD
pxui_types::theme::BodyKind::section_index variant @U2a.3
pxui_types::theme::BodyKind::segmented variant @HEAD
pxui_types::theme::BodyKind::selection_list variant @HEAD
pxui_types::theme::BodyKind::slider variant @HEAD
pxui_types::theme::BodyKind::spinner variant @HEAD
pxui_types::theme::BodyKind::split variant @HEAD
-pxui_types::theme::BodyKind::table_cell variant @U2b.1
pxui_types::theme::BodyKind::table_head variant @HEAD
pxui_types::theme::BodyKind::table_row variant @HEAD
pxui_types::theme::BodyKind::tabs variant @HEAD
pxui_types::theme::BodyKind::text_field variant @HEAD
pxui_types::theme::BodyKind::toasts variant @HEAD
pxui_types::theme::BodyKind::toggle variant @HEAD
pxui_types::theme::BodyKind::tooltip variant @HEAD
pxui_types::theme::BodyKind::tree variant @HEAD
pxui_types::theme::BodyKind::value_field variant @HEAD
pxui_types::theme::BodyKind::window variant @HEAD
pxui_types::theme::ColorToken impl Clone @HEAD
pxui_types::theme::ColorToken impl Copy @HEAD
pxui_types::theme::ColorToken impl Debug @HEAD
pxui_types::theme::ColorToken impl Default @HEAD
pxui_types::theme::ColorToken impl Eq @HEAD
pxui_types::theme::ColorToken impl Hash @HEAD
pxui_types::theme::ColorToken impl PartialEq @HEAD
pxui_types::theme::ColorToken impl StructuralPartialEq @HEAD
pxui_types::theme::ColorToken struct (u16) @HEAD
pxui_types::theme::ColorToken::ACCENT const ColorToken @U2b.7
pxui_types::theme::ColorToken::LIMIT const usize @HEAD
pxui_types::theme::Curve enum @HEAD
pxui_types::theme::Curve impl Clone @HEAD
pxui_types::theme::Curve impl Copy @HEAD
pxui_types::theme::Curve impl Debug @HEAD
pxui_types::theme::Curve impl From<Curve> @HEAD
pxui_types::theme::Curve impl PartialEq @HEAD
pxui_types::theme::Curve impl StructuralPartialEq @HEAD
pxui_types::theme::Curve::Bezier variant { duration: f32, points: [f32; 4] } @HEAD
pxui_types::theme::Curve::Spring variant { omega: f32 } @HEAD
pxui_types::theme::Curve::Steps variant { duration: f32, count: u32 } @HEAD
pxui_types::theme::Curve::Table variant { duration: f32, count: u32, positions: [f32; 16], values: [f32; 16] } @HEAD
pxui_types::theme::Curve::at fn (&self, t: f32) -> f32 @HEAD
pxui_types::theme::Curve::bezier_valid fn (duration: f32, points: &[f32; 4]) -> bool @HEAD
pxui_types::theme::Curve::finished fn (&self, t: f32) -> bool @HEAD
pxui_types::theme::Curve::instant fn (&self) -> bool @HEAD
pxui_types::theme::Curve::spring_valid fn (omega: f32) -> bool @HEAD
pxui_types::theme::Curve::valid fn (&self) -> bool @HEAD
pxui_types::theme::Curve::with_duration fn (self, seconds: f32) -> Self @HEAD
pxui_types::theme::DISPLAY_SIZE const i64 @HEAD
pxui_types::theme::FONT_SIZE const i64 @HEAD
pxui_types::theme::Font impl Clone @HEAD
pxui_types::theme::Font impl Copy @HEAD
pxui_types::theme::Font impl Debug @HEAD
pxui_types::theme::Font impl PartialEq @HEAD
pxui_types::theme::Font impl StructuralPartialEq @HEAD
pxui_types::theme::Font struct @HEAD
pxui_types::theme::Font::BODY const Font @HEAD
pxui_types::theme::Font::caps field bool @HEAD
pxui_types::theme::Font::caps fn const (self) -> Font @HEAD
pxui_types::theme::Font::family field u16 @HEAD
pxui_types::theme::Font::family fn const (self, family: u16) -> Font @HEAD
pxui_types::theme::Font::points fn (&self, body: f32) -> f32 @U1b.3
-pxui_types::theme::Font::points fn (&self, body: i64) -> i64 @U1b.3
pxui_types::theme::Font::size field i64 @HEAD
pxui_types::theme::Font::size fn const (self, points: f32) -> Font @U1b.3
-pxui_types::theme::Font::size fn const (self, points: i64) -> Font @U1b.3
pxui_types::theme::Font::size_points field f32 @U1b.3
pxui_types::theme::Font::tracking field f32 @HEAD
pxui_types::theme::Font::tracking fn const (self, ems: f32) -> Font @HEAD
pxui_types::theme::Font::weight field u16 @HEAD
pxui_types::theme::Font::weight fn const (self, weight: u16) -> Font @HEAD
pxui_types::theme::LABEL_SIZE const i64 @HEAD
pxui_types::theme::Look impl Clone @HEAD
pxui_types::theme::Look impl Copy @HEAD
pxui_types::theme::Look impl Debug @HEAD
pxui_types::theme::Look impl From<Look> @HEAD
pxui_types::theme::Look impl From<PxuiLookDesc> @HEAD
pxui_types::theme::Look impl PartialEq @HEAD
pxui_types::theme::Look impl StructuralPartialEq @HEAD
pxui_types::theme::Look struct @HEAD
pxui_types::theme::Look::NONE const Look @HEAD
pxui_types::theme::Look::angle field f32 @HEAD
pxui_types::theme::Look::angle fn const (self, degrees: f32) -> Look @HEAD
pxui_types::theme::Look::border field Rgba @HEAD
pxui_types::theme::Look::border fn const (self, color: Rgba, width: f32) -> Look @HEAD
pxui_types::theme::Look::border_w field f32 @HEAD
pxui_types::theme::Look::caret field Rgba @HEAD
pxui_types::theme::Look::caret fn const (self, color: Rgba) -> Look @HEAD
pxui_types::theme::Look::dashed field bool @HEAD
pxui_types::theme::Look::dashed fn const (self) -> Look @HEAD
pxui_types::theme::Look::fill field Rgba @HEAD
pxui_types::theme::Look::fill fn const (self, color: Rgba) -> Look @HEAD
pxui_types::theme::Look::font field Font @HEAD
pxui_types::theme::Look::font fn const (self, font: Font) -> Look @HEAD
pxui_types::theme::Look::gradient field OptColor @HEAD
pxui_types::theme::Look::gradient fn const (self, to: Rgba) -> Look @HEAD
pxui_types::theme::Look::lerp fn (&self, to: &Look, t: f32) -> Look @HEAD
pxui_types::theme::Look::opacity field f32 @HEAD
pxui_types::theme::Look::opacity fn const (self, opacity: f32) -> Look @HEAD
pxui_types::theme::Look::radii fn const (self, points: [f32; 4]) -> Look @HEAD
pxui_types::theme::Look::radius field [f32; 4] @HEAD
pxui_types::theme::Look::radius fn const (self, points: f32) -> Look @HEAD
pxui_types::theme::Look::ring field Ring @HEAD
pxui_types::theme::Look::ring fn const (self, ring: Ring) -> Look @HEAD
pxui_types::theme::Look::selection field Rgba @HEAD
pxui_types::theme::Look::selection fn const (self, color: Rgba) -> Look @HEAD
pxui_types::theme::Look::shadow fn const (self, shadow: Shadow) -> Look @HEAD
pxui_types::theme::Look::shadows field [Shadow; 2] @HEAD
pxui_types::theme::Look::shadows fn const (self, shadows: [Shadow; 2]) -> Look @HEAD
pxui_types::theme::Look::sides field u8 @HEAD
pxui_types::theme::Look::sides fn const (self, sides: u8) -> Look @HEAD
pxui_types::theme::Look::text field Rgba @HEAD
pxui_types::theme::Look::text fn const (self, color: Rgba) -> Look @HEAD
pxui_types::theme::Motion impl Clone @HEAD
pxui_types::theme::Motion impl Copy @HEAD
pxui_types::theme::Motion impl Debug @HEAD
pxui_types::theme::Motion impl From<PxuiMotion> @HEAD
pxui_types::theme::Motion impl PartialEq @HEAD
pxui_types::theme::Motion impl StructuralPartialEq @HEAD
pxui_types::theme::Motion struct @HEAD
pxui_types::theme::Motion::ease field [f32; 4] @HEAD
pxui_types::theme::Motion::fast field f32 @HEAD
pxui_types::theme::Motion::normal field f32 @HEAD
pxui_types::theme::Motion::slow field f32 @HEAD
pxui_types::theme::Part impl Clone @HEAD
pxui_types::theme::Part impl Copy @HEAD
pxui_types::theme::Part impl Debug @HEAD
pxui_types::theme::Part impl Eq @HEAD
pxui_types::theme::Part impl PartialEq @HEAD
pxui_types::theme::Part impl StructuralPartialEq @HEAD
pxui_types::theme::Part struct (u32) @HEAD
pxui_types::theme::Part::BAR const Part @HEAD
pxui_types::theme::Part::CLEAR const Part @HEAD
pxui_types::theme::Part::DISCLOSURE const Part @HEAD
pxui_types::theme::Part::EDITOR const Part @HEAD
pxui_types::theme::Part::HANDLE const Part @U2b.3
pxui_types::theme::Part::HIT const Part @HEAD
pxui_types::theme::Part::RAIL const Part @HEAD
pxui_types::theme::Part::THUMB const Part @HEAD
pxui_types::theme::Part::TITLE const Part @HEAD
pxui_types::theme::Part::TRACK const Part @HEAD
pxui_types::theme::Part::button fn const (i: u8) -> Part @HEAD
pxui_types::theme::Part::dot fn const (i: u8) -> Part @HEAD
pxui_types::theme::Part::valid fn const (n: u32) -> bool @HEAD
pxui_types::theme::Ring impl Clone @HEAD
pxui_types::theme::Ring impl Copy @HEAD
pxui_types::theme::Ring impl Debug @HEAD
pxui_types::theme::Ring impl PartialEq @HEAD
pxui_types::theme::Ring impl StructuralPartialEq @HEAD
pxui_types::theme::Ring struct @HEAD
pxui_types::theme::Ring::NONE const Ring @HEAD
pxui_types::theme::Ring::color field Rgba @HEAD
pxui_types::theme::Ring::color fn const (self, color: Rgba) -> Ring @HEAD
pxui_types::theme::Ring::sides field u8 @HEAD
pxui_types::theme::Ring::sides fn const (self, sides: u8) -> Ring @HEAD
pxui_types::theme::Ring::width field f32 @HEAD
pxui_types::theme::Ring::width fn const (self, points: f32) -> Ring @HEAD
pxui_types::theme::Shadow impl Clone @HEAD
pxui_types::theme::Shadow impl Copy @HEAD
pxui_types::theme::Shadow impl Debug @HEAD
pxui_types::theme::Shadow impl PartialEq @HEAD
pxui_types::theme::Shadow impl StructuralPartialEq @HEAD
pxui_types::theme::Shadow struct @HEAD
pxui_types::theme::Shadow::NONE const Shadow @HEAD
pxui_types::theme::Shadow::blur field f32 @HEAD
pxui_types::theme::Shadow::blur fn const (self, points: f32) -> Shadow @HEAD
pxui_types::theme::Shadow::color field Rgba @HEAD
pxui_types::theme::Shadow::color fn const (self, color: Rgba) -> Shadow @HEAD
pxui_types::theme::Shadow::inner field bool @HEAD
pxui_types::theme::Shadow::inner fn const (self) -> Shadow @HEAD
pxui_types::theme::Shadow::offset field [f32; 2] @HEAD
pxui_types::theme::Shadow::offset fn const (self, x: f32, y: f32) -> Shadow @HEAD
pxui_types::theme::Shadow::spread field f32 @HEAD
pxui_types::theme::Shadow::spread fn const (self, points: f32) -> Shadow @HEAD
pxui_types::theme::State impl Clone @HEAD
pxui_types::theme::State impl Copy @HEAD
pxui_types::theme::State impl Debug @HEAD
pxui_types::theme::State impl Default @HEAD
pxui_types::theme::State impl Eq @HEAD
pxui_types::theme::State impl From<PxuiStateDesc> @U4b
pxui_types::theme::State impl PartialEq @HEAD
pxui_types::theme::State impl StructuralPartialEq @HEAD
pxui_types::theme::State struct @HEAD
pxui_types::theme::State::checked field bool @HEAD
pxui_types::theme::State::checked fn const (self, checked: bool) -> State @U1b.5
pxui_types::theme::State::disabled field bool @HEAD
pxui_types::theme::State::disabled fn const (self, disabled: bool) -> State @U1b.5
pxui_types::theme::State::focused field bool @HEAD
pxui_types::theme::State::focused fn const (self, focused: bool) -> State @U1b.5
pxui_types::theme::State::hovered field bool @HEAD
pxui_types::theme::State::hovered fn const (self, hovered: bool) -> State @U1b.5
pxui_types::theme::State::pressed field bool @HEAD
pxui_types::theme::State::pressed fn const (self, pressed: bool) -> State @U1b.5
pxui_types::theme::States impl Clone @HEAD
pxui_types::theme::States impl Copy @HEAD
pxui_types::theme::States impl Debug @HEAD
pxui_types::theme::States impl PartialEq @HEAD
pxui_types::theme::States impl StructuralPartialEq @HEAD
pxui_types::theme::States struct @HEAD
-pxui_types::theme::States::all fn const (look: Look) -> States @U1b.4
pxui_types::theme::States::checked field Look @HEAD
pxui_types::theme::States::checked fn const (self, look: Look) -> States @HEAD
pxui_types::theme::States::disabled field Look @HEAD
pxui_types::theme::States::disabled fn const (self, look: Look) -> States @HEAD
pxui_types::theme::States::focus field Look @HEAD
pxui_types::theme::States::focus fn const (self, look: Look) -> States @HEAD
pxui_types::theme::States::hover field Look @HEAD
pxui_types::theme::States::hover fn const (self, look: Look) -> States @HEAD
pxui_types::theme::States::idle field Look @HEAD
pxui_types::theme::States::idle fn const (self, look: Look) -> States @HEAD
pxui_types::theme::States::of fn const (look: Look) -> States @U1b.4
pxui_types::theme::States::pick fn (&self, s: State) -> &Look @U2b.4.20
pxui_types::theme::States::pressed field Look @HEAD
pxui_types::theme::States::pressed fn const (self, look: Look) -> States @HEAD
pxui_types::theme::TITLE_SIZE const i64 @HEAD
pxui_types::theme::Theme type = ThemeSnapshot @HEAD
pxui_types::theme::ThemeSnapshot impl Clone @HEAD
pxui_types::theme::ThemeSnapshot impl Copy @HEAD
pxui_types::theme::ThemeSnapshot impl Debug @HEAD
pxui_types::theme::ThemeSnapshot impl PartialEq @HEAD
pxui_types::theme::ThemeSnapshot impl StructuralPartialEq @HEAD
pxui_types::theme::ThemeSnapshot struct @HEAD
pxui_types::theme::ThemeSnapshot::badge field States @HEAD
pxui_types::theme::ThemeSnapshot::body field Bodies @HEAD
pxui_types::theme::ThemeSnapshot::button field States @HEAD
pxui_types::theme::ThemeSnapshot::button_bare field States @HEAD
pxui_types::theme::ThemeSnapshot::checkbox field States @HEAD
pxui_types::theme::ThemeSnapshot::chevron field States @HEAD
pxui_types::theme::ThemeSnapshot::column field States @HEAD
pxui_types::theme::ThemeSnapshot::control_h field f32 @HEAD
pxui_types::theme::ThemeSnapshot::dialog field States @HEAD
pxui_types::theme::ThemeSnapshot::dropdown field States @HEAD
pxui_types::theme::ThemeSnapshot::field field States @HEAD
pxui_types::theme::ThemeSnapshot::field_invalid field States @HEAD
pxui_types::theme::ThemeSnapshot::footer field States @HEAD
pxui_types::theme::ThemeSnapshot::fraction field States @HEAD
pxui_types::theme::ThemeSnapshot::ghost field States @HEAD
pxui_types::theme::ThemeSnapshot::header field States @HEAD
pxui_types::theme::ThemeSnapshot::hint field States @HEAD
pxui_types::theme::ThemeSnapshot::icon_button field States @HEAD
pxui_types::theme::ThemeSnapshot::image field States @HEAD
pxui_types::theme::ThemeSnapshot::index field States @HEAD
pxui_types::theme::ThemeSnapshot::key field States @HEAD
pxui_types::theme::ThemeSnapshot::knob field States @HEAD
pxui_types::theme::ThemeSnapshot::label field States @HEAD
pxui_types::theme::ThemeSnapshot::list field States @HEAD
pxui_types::theme::ThemeSnapshot::listbox field States @HEAD
pxui_types::theme::ThemeSnapshot::mark field States @HEAD
pxui_types::theme::ThemeSnapshot::matched field States @HEAD
pxui_types::theme::ThemeSnapshot::menu field States @HEAD
pxui_types::theme::ThemeSnapshot::menu_danger field States @HEAD
pxui_types::theme::ThemeSnapshot::menu_key field States @HEAD
pxui_types::theme::ThemeSnapshot::menubar field States @HEAD
pxui_types::theme::ThemeSnapshot::modal field States @HEAD
pxui_types::theme::ThemeSnapshot::notice field States @HEAD
pxui_types::theme::ThemeSnapshot::panel field States @HEAD
pxui_types::theme::ThemeSnapshot::part_count field u32 @U5.5
pxui_types::theme::ThemeSnapshot::pin field States @HEAD
pxui_types::theme::ThemeSnapshot::placeholder field States @HEAD
pxui_types::theme::ThemeSnapshot::popup field States @HEAD
pxui_types::theme::ThemeSnapshot::progress field States @HEAD
pxui_types::theme::ThemeSnapshot::radio field States @HEAD
pxui_types::theme::ThemeSnapshot::ring field Ring @HEAD
pxui_types::theme::ThemeSnapshot::row field States @HEAD
pxui_types::theme::ThemeSnapshot::row_doomed field States @HEAD
pxui_types::theme::ThemeSnapshot::row_h field f32 @HEAD
pxui_types::theme::ThemeSnapshot::scroll_view field States @HEAD
pxui_types::theme::ThemeSnapshot::scrollbar field States @HEAD
pxui_types::theme::ThemeSnapshot::scrollbar_gap field f32 @HEAD
pxui_types::theme::ThemeSnapshot::scrollbar_w field f32 @HEAD
pxui_types::theme::ThemeSnapshot::search field States @HEAD
pxui_types::theme::ThemeSnapshot::section field States @HEAD
pxui_types::theme::ThemeSnapshot::segment field States @HEAD
pxui_types::theme::ThemeSnapshot::separator field States @HEAD
pxui_types::theme::ThemeSnapshot::size field u32 @U5.5
pxui_types::theme::ThemeSnapshot::slider field States @HEAD
pxui_types::theme::ThemeSnapshot::spinner field States @HEAD
pxui_types::theme::ThemeSnapshot::swatch field States @HEAD
pxui_types::theme::ThemeSnapshot::switch field States @HEAD
pxui_types::theme::ThemeSnapshot::tab field States @HEAD
pxui_types::theme::ThemeSnapshot::table_head field States @HEAD
pxui_types::theme::ThemeSnapshot::table_row field States @HEAD
pxui_types::theme::ThemeSnapshot::text_area field States @HEAD
pxui_types::theme::ThemeSnapshot::title field States @HEAD
pxui_types::theme::ThemeSnapshot::toast field States @HEAD
pxui_types::theme::ThemeSnapshot::toggle field States @HEAD
pxui_types::theme::ThemeSnapshot::tokens field Tokens @HEAD
pxui_types::theme::ThemeSnapshot::tooltip field States @HEAD
pxui_types::theme::ThemeSnapshot::tree field States @HEAD
pxui_types::theme::ThemeSnapshot::value field States @HEAD
pxui_types::theme::Tokens impl Clone @HEAD
pxui_types::theme::Tokens impl Copy @HEAD
pxui_types::theme::Tokens impl Debug @HEAD
pxui_types::theme::Tokens impl Default @HEAD
pxui_types::theme::Tokens impl From<ThemeTokens> @HEAD
pxui_types::theme::Tokens impl From<Tokens> @HEAD
pxui_types::theme::Tokens impl PartialEq @HEAD
pxui_types::theme::Tokens impl StructuralPartialEq @HEAD
pxui_types::theme::Tokens struct @HEAD
pxui_types::theme::Tokens::DEFAULT const Tokens @HEAD
pxui_types::theme::Tokens::accent field Rgba @HEAD
pxui_types::theme::Tokens::border fn const (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::control field Rgba @HEAD
pxui_types::theme::Tokens::dark field bool @U5.6
pxui_types::theme::Tokens::density field f32 @HEAD
pxui_types::theme::Tokens::display_size field f32 @U1b.3
-pxui_types::theme::Tokens::display_size field i64 @U1b.3
pxui_types::theme::Tokens::edge fn const (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::faint_border fn const (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::font_size field f32 @U1b.3
-pxui_types::theme::Tokens::font_size field i64 @U1b.3
pxui_types::theme::Tokens::foreground field Rgba @HEAD
pxui_types::theme::Tokens::from_ffi fn (value: ThemeTokens) -> Self @HEAD
pxui_types::theme::Tokens::ground fn (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::hover_fill fn (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::ink_2 fn (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::ink_3 fn (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::ink_ground field OptColor @HEAD
pxui_types::theme::Tokens::input field Rgba @HEAD
pxui_types::theme::Tokens::invalid field Rgba @HEAD
pxui_types::theme::Tokens::is_dark fn const (&self) -> bool @HEAD
pxui_types::theme::Tokens::label_size field f32 @U1b.3
-pxui_types::theme::Tokens::label_size field i64 @U1b.3
pxui_types::theme::Tokens::motion field Motion @HEAD
pxui_types::theme::Tokens::moves fn (&self) -> bool @HEAD
pxui_types::theme::Tokens::panel field Rgba @HEAD
pxui_types::theme::Tokens::pressed_fill fn (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::radius field [f32; 3] @HEAD
pxui_types::theme::Tokens::reduced_motion field bool @HEAD
pxui_types::theme::Tokens::space field [f32; 4] @HEAD
pxui_types::theme::Tokens::tint fn const (&self) -> Rgba @HEAD
pxui_types::theme::Tokens::title_size field f32 @U1b.3
-pxui_types::theme::Tokens::title_size field i64 @U1b.3
pxui_types::theme::Tokens::track field Rgba @HEAD
pxui_types::theme::Tokens::transition fn (&self, from: State, to: State) -> Curve @HEAD
pxui_types::theme::WIDGET_COUNT const usize @HEAD
pxui_types::theme_widgets macro @HEAD
pxui_types::ui mod @HEAD
pxui_types::ui::Checkbox impl Clone @HEAD
pxui_types::ui::Checkbox impl Copy @HEAD
pxui_types::ui::Checkbox impl Debug @HEAD
-pxui_types::ui::Checkbox impl Eq @U2a.3
pxui_types::ui::Checkbox impl PartialEq @HEAD
pxui_types::ui::Checkbox impl StructuralPartialEq @HEAD
-pxui_types::ui::Checkbox struct @U2a.3
pxui_types::ui::Checkbox struct <'a> @U2a.3
-pxui_types::ui::Checkbox::DEFAULT const Checkbox @U2a.3
pxui_types::ui::Checkbox::DEFAULT const Checkbox<'a> @U2a.3
pxui_types::ui::Checkbox::disabled field bool @U2a.3
pxui_types::ui::Checkbox::disabled fn const (self, disabled: bool) -> Checkbox<'a> @U2a.3
pxui_types::ui::Checkbox::look field Option<&'a States> @U2a.3
pxui_types::ui::Checkbox::look fn const (self, looks: &'a States) -> Checkbox<'a> @U2a.3
pxui_types::ui::Checkbox::passive field bool @U2a.3
pxui_types::ui::Checkbox::passive fn const (self, passive: bool) -> Checkbox<'a> @U2a.3
pxui_types::ui::Checkbox::size field Option<f32> @U2a.3
pxui_types::ui::Checkbox::size fn const (self, points: f32) -> Checkbox<'a> @U2a.3
pxui_types::ui::Checkbox::variant field u16 @HEAD
-pxui_types::ui::Checkbox::variant fn const (self, variant: u16) -> Checkbox @U2a.3
pxui_types::ui::Checkbox::variant fn const (self, variant: u16) -> Checkbox<'a> @U2a.3
-pxui_types::ui::Choice impl Clone @U2a.2
-pxui_types::ui::Choice impl Copy @U2a.2
-pxui_types::ui::Choice impl Debug @U2a.2
-pxui_types::ui::Choice impl Eq @U2a.2
-pxui_types::ui::Choice impl PartialEq @U2a.2
-pxui_types::ui::Choice impl StructuralPartialEq @U2a.2
-pxui_types::ui::Choice struct <'a> @U2a.2
-pxui_types::ui::Choice::DEFAULT const Choice<'a> @U2a.2
-pxui_types::ui::Choice::name field &'a str @U2a.2
-pxui_types::ui::Choice::name fn const (self, name: &'a str) -> Choice<'a> @U2a.2
-pxui_types::ui::Choice::variant field u16 @U2a.2
-pxui_types::ui::Choice::variant fn const (self, variant: u16) -> Choice<'a> @U2a.2
pxui_types::ui::Chosen impl Clone @HEAD
pxui_types::ui::Chosen impl Copy @HEAD
pxui_types::ui::Chosen impl Debug @HEAD
pxui_types::ui::Chosen impl Default @HEAD
pxui_types::ui::Chosen impl Eq @HEAD
pxui_types::ui::Chosen impl PartialEq @HEAD
pxui_types::ui::Chosen impl StructuralPartialEq @HEAD
pxui_types::ui::Chosen struct @HEAD
pxui_types::ui::Chosen::chosen field Option<usize> @HEAD
pxui_types::ui::Chosen::chosen_id field Option<i64> @HEAD
pxui_types::ui::Chosen::dismissed field bool @HEAD
pxui_types::ui::CodeContext enum @HEAD
pxui_types::ui::CodeContext impl Clone @HEAD
pxui_types::ui::CodeContext impl Copy @HEAD
pxui_types::ui::CodeContext impl Debug @HEAD
pxui_types::ui::CodeContext impl Eq @HEAD
pxui_types::ui::CodeContext impl PartialEq @HEAD
pxui_types::ui::CodeContext impl StructuralPartialEq @HEAD
pxui_types::ui::CodeContext::Code variant @HEAD
pxui_types::ui::CodeContext::Comment variant @HEAD
pxui_types::ui::CodeContext::String variant @HEAD
pxui_types::ui::Completion impl Clone @HEAD
pxui_types::ui::Completion impl Debug @HEAD
pxui_types::ui::Completion impl Eq @HEAD
pxui_types::ui::Completion impl PartialEq @HEAD
pxui_types::ui::Completion impl StructuralPartialEq @HEAD
pxui_types::ui::Completion struct @HEAD
pxui_types::ui::Completion::detail field String @HEAD
pxui_types::ui::Completion::doc field String @HEAD
pxui_types::ui::Completion::insert field String @HEAD
pxui_types::ui::Completion::label field String @HEAD
pxui_types::ui::Completion::replace field (usize, usize) @HEAD
pxui_types::ui::Cursor enum @HEAD
pxui_types::ui::Cursor impl Clone @HEAD
pxui_types::ui::Cursor impl Copy @HEAD
pxui_types::ui::Cursor impl Debug @HEAD
pxui_types::ui::Cursor impl Eq @HEAD
pxui_types::ui::Cursor impl PartialEq @HEAD
pxui_types::ui::Cursor impl StructuralPartialEq @HEAD
pxui_types::ui::Cursor::HorizontalResize variant @HEAD
pxui_types::ui::Cursor::Text variant @HEAD
pxui_types::ui::Cursor::VerticalResize variant @HEAD
pxui_types::ui::DRAG_THRESHOLD const f32 @HEAD
pxui_types::ui::Dialog impl Clone @HEAD
pxui_types::ui::Dialog impl Copy @HEAD
pxui_types::ui::Dialog impl Debug @HEAD
pxui_types::ui::Dialog impl PartialEq @HEAD
pxui_types::ui::Dialog impl StructuralPartialEq @HEAD
pxui_types::ui::Dialog struct @HEAD
pxui_types::ui::Dialog::DEFAULT const Dialog @HEAD
pxui_types::ui::Dialog::variant field u16 @HEAD
pxui_types::ui::Dialog::variant fn const (self, variant: u16) -> Dialog @HEAD
pxui_types::ui::Dialog::w field f32 @HEAD
pxui_types::ui::Dialog::w_px fn const (self, w: f32) -> Dialog @HEAD
pxui_types::ui::DialogButton impl Clone @HEAD
pxui_types::ui::DialogButton impl Copy @HEAD
pxui_types::ui::DialogButton impl Debug @HEAD
pxui_types::ui::DialogButton impl PartialEq @HEAD
pxui_types::ui::DialogButton impl StructuralPartialEq @HEAD
pxui_types::ui::DialogButton struct <'a> @HEAD
pxui_types::ui::DialogButton::cancel field bool @HEAD
pxui_types::ui::DialogButton::cancel fn const (self) -> DialogButton<'a> @HEAD
pxui_types::ui::DialogButton::danger field bool @HEAD
pxui_types::ui::DialogButton::danger fn const (self) -> DialogButton<'a> @HEAD
pxui_types::ui::DialogButton::detail field &'a str @HEAD
pxui_types::ui::DialogButton::detail fn const (self, detail: &'a str) -> DialogButton<'a> @HEAD
pxui_types::ui::DialogButton::disabled fn const (self, disabled: bool) -> DialogButton<'a> @U1b.6
pxui_types::ui::DialogButton::enabled field bool @HEAD
-pxui_types::ui::DialogButton::enabled fn const (self, enabled: bool) -> DialogButton<'a> @U1b.6
pxui_types::ui::DialogButton::new fn const (text: &'a str) -> DialogButton<'a> @HEAD
pxui_types::ui::DialogButton::primary field bool @HEAD
pxui_types::ui::DialogButton::primary fn const (self) -> DialogButton<'a> @HEAD
pxui_types::ui::DialogButton::text field &'a str @HEAD
pxui_types::ui::DismissOpts impl Clone @U2a.4
pxui_types::ui::DismissOpts impl Copy @U2a.4
pxui_types::ui::DismissOpts impl Debug @U2a.4
pxui_types::ui::DismissOpts impl Default @U2a.4
pxui_types::ui::DismissOpts impl PartialEq @U2a.4
pxui_types::ui::DismissOpts impl StructuralPartialEq @U2a.4
pxui_types::ui::DismissOpts struct @U2a.4
pxui_types::ui::DismissOpts::DEFAULT const DismissOpts @U2a.4
pxui_types::ui::DismissOpts::fraction field Option<f32> @U2a.4
pxui_types::ui::DismissOpts::fraction fn const (self, fraction: f32) -> DismissOpts @U2a.4
pxui_types::ui::DismissOpts::velocity field Option<f32> @U2a.4
pxui_types::ui::DismissOpts::velocity fn const (self, velocity: f32) -> DismissOpts @U2a.4
pxui_types::ui::Dropdown impl Clone @HEAD
pxui_types::ui::Dropdown impl Copy @HEAD
pxui_types::ui::Dropdown impl Debug @HEAD
pxui_types::ui::Dropdown impl Eq @HEAD
pxui_types::ui::Dropdown impl PartialEq @HEAD
pxui_types::ui::Dropdown impl StructuralPartialEq @HEAD
pxui_types::ui::Dropdown struct @HEAD
pxui_types::ui::Dropdown::DEFAULT const Dropdown @HEAD
pxui_types::ui::Dropdown::variant field u16 @HEAD
pxui_types::ui::Dropdown::variant fn const (self, variant: u16) -> Dropdown @HEAD
pxui_types::ui::Head impl Clone @HEAD
pxui_types::ui::Head impl Debug @HEAD
pxui_types::ui::Head impl Eq @HEAD
pxui_types::ui::Head impl PartialEq @HEAD
pxui_types::ui::Head impl StructuralPartialEq @HEAD
pxui_types::ui::Head struct <'ui> @U5.9
pxui_types::ui::Head::chosen field Option<i64> @HEAD
pxui_types::ui::Head::renamed field &'ui str @U5.9
pxui_types::ui::Head::reset_pressed field bool @HEAD
pxui_types::ui::HeadAction impl Clone @HEAD
pxui_types::ui::HeadAction impl Debug @HEAD
pxui_types::ui::HeadAction impl Eq @HEAD
pxui_types::ui::HeadAction impl PartialEq @HEAD
pxui_types::ui::HeadAction impl StructuralPartialEq @HEAD
pxui_types::ui::HeadAction struct @HEAD
pxui_types::ui::HeadAction::active field bool @HEAD
pxui_types::ui::HeadAction::caption field String @HEAD
pxui_types::ui::HeadAction::keycap field String @HEAD
pxui_types::ui::HeadAction::usable field bool @HEAD
pxui_types::ui::Key enum @HEAD
pxui_types::ui::Key impl Clone @HEAD
pxui_types::ui::Key impl Copy @HEAD
pxui_types::ui::Key impl Debug @HEAD
pxui_types::ui::Key impl Eq @HEAD
pxui_types::ui::Key impl From<&'static str> @HEAD
pxui_types::ui::Key impl From<i64> @HEAD
pxui_types::ui::Key impl From<u64> @HEAD
pxui_types::ui::Key impl PartialEq @HEAD
pxui_types::ui::Key impl StructuralPartialEq @HEAD
pxui_types::ui::Key::Id variant (i64) @HEAD
pxui_types::ui::Key::Label variant (&'static str) @HEAD
pxui_types::ui::Key::Resolved variant (i64) @U1b.2
pxui_types::ui::Key::label fn <'b>(&self, buf: &'b mut [u8; 17]) -> &'b str @HEAD
pxui_types::ui::Keyframe impl Clone @HEAD
pxui_types::ui::Keyframe impl Copy @HEAD
pxui_types::ui::Keyframe impl Debug @HEAD
pxui_types::ui::Keyframe impl PartialEq @HEAD
pxui_types::ui::Keyframe impl StructuralPartialEq @HEAD
pxui_types::ui::Keyframe struct @HEAD
pxui_types::ui::Keyframe::anchor fn const (self, x: f32, y: f32) -> Keyframe @HEAD
pxui_types::ui::Keyframe::curve fn const (self, curve: Curve) -> Keyframe @HEAD
pxui_types::ui::Keyframe::look fn const (time: f32, look: &Look) -> Keyframe @HEAD
pxui_types::ui::Keyframe::offset fn const (time: f32, x: f32, y: f32) -> Keyframe @HEAD
pxui_types::ui::Keyframe::opacity fn const (time: f32, value: f32) -> Keyframe @HEAD
pxui_types::ui::Keyframe::scale fn const (time: f32, x: f32, y: f32) -> Keyframe @HEAD
pxui_types::ui::Keyframe::segment fn const (&self) -> Curve @HEAD
pxui_types::ui::Keyframe::target fn const (&self) -> Target @HEAD
pxui_types::ui::Keyframe::time fn const (&self) -> f32 @HEAD
pxui_types::ui::Keyframes impl Clone @HEAD
pxui_types::ui::Keyframes impl Copy @HEAD
pxui_types::ui::Keyframes impl Debug @HEAD
pxui_types::ui::Keyframes impl PartialEq @HEAD
pxui_types::ui::Keyframes impl StructuralPartialEq @HEAD
pxui_types::ui::Keyframes struct @HEAD
pxui_types::ui::Keyframes::DEFAULT const Keyframes @HEAD
pxui_types::ui::Keyframes::autoreverse fn const (self) -> Keyframes @HEAD
pxui_types::ui::Keyframes::cycles fn const (&self) -> Repeat @HEAD
pxui_types::ui::Keyframes::offset fn const (self, seconds: f32) -> Keyframes @HEAD
pxui_types::ui::Keyframes::repeat fn const (self, repeat: Repeat) -> Keyframes @HEAD
pxui_types::ui::Keyframes::reverses fn const (&self) -> bool @HEAD
pxui_types::ui::Keyframes::start fn const (&self) -> f32 @HEAD
pxui_types::ui::LineStep enum @HEAD
pxui_types::ui::LineStep impl Clone @HEAD
pxui_types::ui::LineStep impl Copy @HEAD
pxui_types::ui::LineStep impl Debug @HEAD
pxui_types::ui::LineStep impl Eq @HEAD
pxui_types::ui::LineStep impl PartialEq @HEAD
pxui_types::ui::LineStep impl StructuralPartialEq @HEAD
pxui_types::ui::LineStep::Close variant @HEAD
pxui_types::ui::LineStep::Next variant @HEAD
pxui_types::ui::LineStep::None variant @HEAD
pxui_types::ui::LineStep::Previous variant @HEAD
pxui_types::ui::ListResponse impl Clone @HEAD
pxui_types::ui::ListResponse impl Copy @HEAD
pxui_types::ui::ListResponse impl Debug @HEAD
pxui_types::ui::ListResponse impl Default @HEAD
-pxui_types::ui::ListResponse impl Eq @U2b.2
-pxui_types::ui::ListResponse impl PartialEq @U2b.2
-pxui_types::ui::ListResponse impl StructuralPartialEq @U2b.2
pxui_types::ui::ListResponse struct @HEAD
pxui_types::ui::ListResponse::activated field Option<usize> @U2b.2
pxui_types::ui::ListResponse::drop_at field Option<usize> @U2b.2
-pxui_types::ui::ListResponse::focused field bool @U2b.4.18
pxui_types::ui::ListResponse::handle field PxuiBox @U2b.2
pxui_types::ui::ListResponse::moved field Option<(usize, usize)> @U2b.2
pxui_types::ui::ListResponse::row field Option<PxuiBox> @U2b.2
pxui_types::ui::ListResponse::secondary field Option<usize> @U2b.2
pxui_types::ui::ListResponse::selected field Option<usize> @U2b.2
pxui_types::ui::Menu impl Clone @HEAD
pxui_types::ui::Menu impl Copy @HEAD
pxui_types::ui::Menu impl Debug @HEAD
pxui_types::ui::Menu impl PartialEq @HEAD
pxui_types::ui::Menu impl StructuralPartialEq @HEAD
pxui_types::ui::Menu struct <'a> @HEAD
pxui_types::ui::Menu::DEFAULT const Menu<'a> @HEAD
pxui_types::ui::Menu::focus_on_click field bool @HEAD
pxui_types::ui::Menu::focus_on_click fn const (self, focus: bool) -> Menu<'a> @HEAD
pxui_types::ui::Menu::max_h field Option<f32> @HEAD
pxui_types::ui::Menu::max_h fn const (self, h: f32) -> Menu<'a> @HEAD
pxui_types::ui::Menu::max_rows field Option<usize> @HEAD
pxui_types::ui::Menu::max_rows fn const (self, rows: usize) -> Menu<'a> @HEAD
pxui_types::ui::Menu::name field Option<&'a str> @HEAD
pxui_types::ui::Menu::name fn const (self, name: &'a str) -> Menu<'a> @HEAD
pxui_types::ui::Menu::padding field f32 @HEAD
pxui_types::ui::Menu::padding fn const (self, padding: f32) -> Menu<'a> @HEAD
pxui_types::ui::Menu::row_h field Option<f32> @HEAD
pxui_types::ui::Menu::row_h fn const (self, row_h: f32) -> Menu<'a> @HEAD
pxui_types::ui::Menu::variant field u16 @HEAD
pxui_types::ui::Menu::variant fn const (self, variant: u16) -> Menu<'a> @HEAD
pxui_types::ui::Menu::w field Option<f32> @HEAD
pxui_types::ui::Menu::w_px fn const (self, w: f32) -> Menu<'a> @HEAD
pxui_types::ui::MenuItem impl Clone @HEAD
pxui_types::ui::MenuItem impl Copy @HEAD
pxui_types::ui::MenuItem impl Debug @HEAD
pxui_types::ui::MenuItem struct <'a> @HEAD
pxui_types::ui::MenuItem::checked field Option<bool> @HEAD
pxui_types::ui::MenuItem::checked fn const (self, checked: bool) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::danger field bool @HEAD
pxui_types::ui::MenuItem::danger fn const (self) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::disabled fn const (self, disabled: bool) -> MenuItem<'a> @U1b.6
pxui_types::ui::MenuItem::enabled field bool @HEAD
-pxui_types::ui::MenuItem::enabled fn const (self, enabled: bool) -> MenuItem<'a> @U1b.6
pxui_types::ui::MenuItem::id field Option<i64> @HEAD
pxui_types::ui::MenuItem::id fn const (self, id: i64) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::new fn const (text: &'a str) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::selected field bool @HEAD
pxui_types::ui::MenuItem::selected fn const (self, selected: bool) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::shortcut field &'a str @HEAD
pxui_types::ui::MenuItem::shortcut fn const (self, shortcut: &'a str) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::submenu field &'a [MenuItem<'a>] @HEAD
pxui_types::ui::MenuItem::submenu fn const (self, rows: &'a [MenuItem<'a>]) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::swatch field Option<Rgba> @HEAD
pxui_types::ui::MenuItem::swatch fn const (self, color: Rgba) -> MenuItem<'a> @HEAD
pxui_types::ui::MenuItem::text field &'a str @HEAD
pxui_types::ui::Overlay impl Clone @HEAD
pxui_types::ui::Overlay impl Copy @HEAD
pxui_types::ui::Overlay impl Debug @HEAD
pxui_types::ui::Overlay impl PartialEq @HEAD
pxui_types::ui::Overlay impl StructuralPartialEq @HEAD
pxui_types::ui::Overlay struct <'a> @HEAD
pxui_types::ui::Overlay::DEFAULT const Overlay<'a> @HEAD
pxui_types::ui::Overlay::at field (f32, f32) @HEAD
pxui_types::ui::Overlay::at fn const (self, at: Point) -> Overlay<'a> @U1b.5
-pxui_types::ui::Overlay::at fn const (self, x: f32, y: f32) -> Overlay<'a> @U1b.5
pxui_types::ui::Overlay::attached field bool @HEAD
pxui_types::ui::Overlay::attached fn const (self) -> Overlay<'a> @HEAD
pxui_types::ui::Overlay::dismiss field DismissOpts @U2a.4
pxui_types::ui::Overlay::dismiss fn const (self, dismiss: DismissOpts) -> Overlay<'a> @U2a.4
pxui_types::ui::Overlay::dismiss_initial field bool @HEAD
pxui_types::ui::Overlay::dismiss_initial fn const (self) -> Overlay<'a> @HEAD
pxui_types::ui::Overlay::h field Option<f32> @HEAD
-pxui_types::ui::Overlay::h fn const (self, h: f32) -> Overlay<'a> @U1b.5
pxui_types::ui::Overlay::h_px fn const (self, h: f32) -> Overlay<'a> @U1b.5
-pxui_types::ui::Overlay::look field Option<Look> @U4b.6
-pxui_types::ui::Overlay::look fn const (self, look: Look) -> Overlay<'a> @U4b.6
pxui_types::ui::Overlay::look field Option<&'a Look> @U4b.6
pxui_types::ui::Overlay::look fn const (self, look: &'a Look) -> Overlay<'a> @U4b.6
pxui_types::ui::Overlay::max_h field Option<f32> @HEAD
-pxui_types::ui::Overlay::max_h fn const (self, h: f32) -> Overlay<'a> @U1b.5
pxui_types::ui::Overlay::max_h_px fn const (self, h: f32) -> Overlay<'a> @U1b.5
pxui_types::ui::Overlay::name field Option<&'a str> @HEAD
pxui_types::ui::Overlay::name fn const (self, name: &'a str) -> Overlay<'a> @HEAD
pxui_types::ui::Overlay::padding field f32 @HEAD
pxui_types::ui::Overlay::padding fn const (self, padding: f32) -> Overlay<'a> @HEAD
pxui_types::ui::Overlay::place field Placement @HEAD
pxui_types::ui::Overlay::place fn const (self, place: Placement) -> Overlay<'a> @HEAD
pxui_types::ui::Overlay::present field Presentation @U2a.4
pxui_types::ui::Overlay::present fn const (self, presentation: Presentation) -> Overlay<'a> @U2a.4
pxui_types::ui::Overlay::stroke field Option<Rgba> @HEAD
pxui_types::ui::Overlay::stroke fn const (self, color: Rgba) -> Overlay<'a> @HEAD
pxui_types::ui::Overlay::w field Option<f32> @HEAD
-pxui_types::ui::Overlay::w fn const (self, w: f32) -> Overlay<'a> @U1b.5
pxui_types::ui::Overlay::w_px fn const (self, w: f32) -> Overlay<'a> @U1b.5
pxui_types::ui::Overlay::window field bool @HEAD
pxui_types::ui::Overlay::window fn const (self) -> Overlay<'a> @HEAD
pxui_types::ui::PanelOpts impl Clone @HEAD
pxui_types::ui::PanelOpts impl Default @HEAD
pxui_types::ui::PanelOpts struct @HEAD
pxui_types::ui::PanelOpts::at fn (self, x: f32, y: f32) -> PanelOpts @HEAD
pxui_types::ui::PanelOpts::height field Option<f32> @HEAD
pxui_types::ui::PanelOpts::height fn (self, height: f32) -> PanelOpts @HEAD
pxui_types::ui::PanelOpts::look field Option<Look> @HEAD
pxui_types::ui::PanelOpts::max_height field Option<f32> @HEAD
pxui_types::ui::PanelOpts::max_height fn (self, height: f32) -> PanelOpts @HEAD
pxui_types::ui::PanelOpts::padding field f32 @U1b.3
-pxui_types::ui::PanelOpts::padding field i64 @U1b.3
-pxui_types::ui::PanelOpts::padding fn (self, padding: i64) -> PanelOpts @U1b.3
pxui_types::ui::PanelOpts::padding fn const (self, padding: f32) -> PanelOpts @U1b.3
pxui_types::ui::PanelOpts::stroke field Option<Rgba> @HEAD
pxui_types::ui::PanelOpts::width field f32 @HEAD
pxui_types::ui::PanelOpts::width fn (self, width: f32) -> PanelOpts @HEAD
pxui_types::ui::PanelOpts::window field bool @HEAD
pxui_types::ui::PanelOpts::window fn (self, window: bool) -> PanelOpts @HEAD
pxui_types::ui::PanelOpts::x field f32 @HEAD
pxui_types::ui::PanelOpts::y field f32 @HEAD
pxui_types::ui::Pick enum @HEAD
pxui_types::ui::Pick impl Clone @HEAD
pxui_types::ui::Pick impl Copy @HEAD
pxui_types::ui::Pick impl Debug @HEAD
pxui_types::ui::Pick impl Eq @HEAD
pxui_types::ui::Pick impl PartialEq @HEAD
pxui_types::ui::Pick impl StructuralPartialEq @HEAD
pxui_types::ui::Pick::Back variant @HEAD
pxui_types::ui::Pick::Cancel variant @HEAD
pxui_types::ui::Pick::Delete variant (i64) @HEAD
pxui_types::ui::Pick::None variant @HEAD
pxui_types::ui::Pick::Pick variant (i64) @HEAD
pxui_types::ui::Pick::Submit variant @HEAD
pxui_types::ui::PickArt enum @U2a.4
pxui_types::ui::PickArt impl Clone @U2a.4
pxui_types::ui::PickArt impl Copy @U2a.4
pxui_types::ui::PickArt impl Debug @U2a.4
pxui_types::ui::PickArt impl Eq @U2a.4
pxui_types::ui::PickArt impl PartialEq @U2a.4
pxui_types::ui::PickArt impl StructuralPartialEq @U2a.4
pxui_types::ui::PickArt::Glyph variant (char) @U2a.4
pxui_types::ui::PickArt::Image variant (usize) @U2a.4
pxui_types::ui::PickArt::Draw variant (u32) @U2b.4.19
pxui_types::ui::PickRow impl Clone @U2a.4
pxui_types::ui::PickRow impl Copy @U2a.4
pxui_types::ui::PickRow impl Debug @U2a.4
pxui_types::ui::PickRow impl Eq @U2a.4
pxui_types::ui::PickRow impl PartialEq @U2a.4
pxui_types::ui::PickRow impl StructuralPartialEq @U2a.4
pxui_types::ui::PickRow struct <'a> @U2a.4
pxui_types::ui::PickRow::art field Option<PickArt> @U2a.4
pxui_types::ui::PickRow::art fn const (self, art: PickArt) -> PickRow<'a> @U2a.4
pxui_types::ui::PickRow::detail field &'a str @U2a.4
pxui_types::ui::PickRow::detail fn const (self, detail: &'a str) -> PickRow<'a> @U2a.4
pxui_types::ui::PickRow::heading field &'a str @U2a.4
pxui_types::ui::PickRow::heading fn const (self, heading: &'a str) -> PickRow<'a> @U2a.4
pxui_types::ui::PickRow::label field &'a str @U2a.4
pxui_types::ui::PickRow::new fn const (label: &'a str) -> PickRow<'a> @U2a.4
pxui_types::ui::PickRows impl Debug @HEAD
pxui_types::ui::PickRows impl Default @HEAD
pxui_types::ui::PickRows struct @HEAD
pxui_types::ui::PickRows::clear fn (&mut self) @HEAD
-pxui_types::ui::PickRows::get fn (&self, i: usize) -> Option<(&str, &str)> @U2a.4
pxui_types::ui::PickRows::get fn (&self, i: usize) -> Option<PickRow<'_>> @U2a.4
pxui_types::ui::PickRows::is_empty fn (&self) -> bool @HEAD
pxui_types::ui::PickRows::len fn (&self) -> usize @HEAD
pxui_types::ui::PickRows::new fn const () -> PickRows @HEAD
-pxui_types::ui::PickRows::push fn (&mut self, label: &str, detail: &str) @U2a.4
pxui_types::ui::PickRows::push fn (&mut self, row: PickRow<'_>) @U2a.4
pxui_types::ui::PickerResponse impl Clone @HEAD
pxui_types::ui::PickerResponse impl Copy @HEAD
pxui_types::ui::PickerResponse impl Debug @HEAD
pxui_types::ui::PickerResponse impl Eq @HEAD
pxui_types::ui::PickerResponse impl PartialEq @HEAD
pxui_types::ui::PickerResponse impl StructuralPartialEq @HEAD
pxui_types::ui::PickerResponse struct <'a> @HEAD
pxui_types::ui::PickerResponse::pick field Pick @HEAD
pxui_types::ui::PickerResponse::query field &'a str @HEAD
pxui_types::ui::Presentation enum @U2a.4
pxui_types::ui::Presentation impl Clone @U2a.4
pxui_types::ui::Presentation impl Copy @U2a.4
pxui_types::ui::Presentation impl Debug @U2a.4
pxui_types::ui::Presentation impl Eq @U2a.4
pxui_types::ui::Presentation impl PartialEq @U2a.4
pxui_types::ui::Presentation impl StructuralPartialEq @U2a.4
pxui_types::ui::Presentation::Bottom variant @U2a.4
pxui_types::ui::Presentation::Centered variant @U2a.4
pxui_types::ui::Presentation::Full variant @U2a.4
pxui_types::ui::Presentation::Top variant @U2b.5
pxui_types::ui::ROOT_KEY const i64 @HEAD
pxui_types::ui::Repeat enum @HEAD
pxui_types::ui::Repeat impl Clone @HEAD
pxui_types::ui::Repeat impl Copy @HEAD
pxui_types::ui::Repeat impl Debug @HEAD
pxui_types::ui::Repeat impl Eq @HEAD
pxui_types::ui::Repeat impl PartialEq @HEAD
pxui_types::ui::Repeat impl StructuralPartialEq @HEAD
pxui_types::ui::Repeat::Count variant (u16) @HEAD
pxui_types::ui::Repeat::Forever variant @HEAD
pxui_types::ui::ScrubPhase enum @HEAD
pxui_types::ui::ScrubPhase impl Clone @HEAD
pxui_types::ui::ScrubPhase impl Copy @HEAD
pxui_types::ui::ScrubPhase impl Debug @HEAD
pxui_types::ui::ScrubPhase impl Eq @HEAD
pxui_types::ui::ScrubPhase impl PartialEq @HEAD
pxui_types::ui::ScrubPhase impl StructuralPartialEq @HEAD
pxui_types::ui::ScrubPhase::Done variant @HEAD
pxui_types::ui::ScrubPhase::Live variant @HEAD
pxui_types::ui::SectionList impl Clone @HEAD
pxui_types::ui::SectionList impl Copy @HEAD
pxui_types::ui::SectionList impl Debug @HEAD
pxui_types::ui::SectionList impl PartialEq @HEAD
pxui_types::ui::SectionList impl StructuralPartialEq @HEAD
pxui_types::ui::SectionList struct @HEAD
pxui_types::ui::SectionList::count field usize @HEAD
pxui_types::ui::SectionList::header fn const (self, h: f32) -> SectionList @HEAD
pxui_types::ui::SectionList::header_height field f32 @HEAD
pxui_types::ui::SectionList::lead fn const (self, h: f32) -> SectionList @HEAD
pxui_types::ui::SectionList::leading_height field f32 @HEAD
pxui_types::ui::SectionList::measured field bool @HEAD
pxui_types::ui::SectionList::measured fn const (self) -> SectionList @HEAD
pxui_types::ui::SectionList::new fn const (count: usize) -> SectionList @HEAD
pxui_types::ui::SectionList::overlay_scrollbar field bool @HEAD
pxui_types::ui::SectionList::overlay_scrollbar fn const (self) -> SectionList @HEAD
pxui_types::ui::SectionList::reorder field bool @U2b.3
pxui_types::ui::SectionList::reorder fn const (self, on: bool) -> SectionList @U2b.3
pxui_types::ui::SectionList::row_h fn const (self, h: f32) -> SectionList @HEAD
pxui_types::ui::SectionList::row_height field f32 @HEAD
pxui_types::ui::SectionList::trail fn const (self, h: f32) -> SectionList @HEAD
pxui_types::ui::SectionList::trailing_height field f32 @HEAD
pxui_types::ui::SectionPart enum @HEAD
pxui_types::ui::SectionPart impl Clone @HEAD
pxui_types::ui::SectionPart impl Copy @HEAD
pxui_types::ui::SectionPart impl Debug @HEAD
pxui_types::ui::SectionPart impl Eq @HEAD
pxui_types::ui::SectionPart impl PartialEq @HEAD
pxui_types::ui::SectionPart impl StructuralPartialEq @HEAD
pxui_types::ui::SectionPart::Header variant @HEAD
pxui_types::ui::SectionPart::Leading variant @HEAD
pxui_types::ui::SectionPart::Row variant (usize) @HEAD
pxui_types::ui::SectionPart::Trailing variant @U2b.1
pxui_types::ui::Segment impl Clone @HEAD
pxui_types::ui::Segment impl Copy @HEAD
pxui_types::ui::Segment impl Debug @HEAD
pxui_types::ui::Segment impl Eq @HEAD
pxui_types::ui::Segment impl PartialEq @HEAD
pxui_types::ui::Segment impl StructuralPartialEq @HEAD
pxui_types::ui::Segment struct <'a> @HEAD
pxui_types::ui::Segment::detail field &'a str @U1b.1
pxui_types::ui::Segment::detail fn const (self, detail: &'a str) -> Segment<'a> @U1b.1
pxui_types::ui::Segment::id field Option<i64> @U1b.1
pxui_types::ui::Segment::id fn const (self, id: i64) -> Segment<'a> @U1b.1
pxui_types::ui::Segment::name field Option<&'a str> @HEAD
pxui_types::ui::Segment::name fn const (self, name: &'a str) -> Segment<'a> @HEAD
pxui_types::ui::Segment::new fn const (text: &'a str) -> Segment<'a> @HEAD
pxui_types::ui::Segment::text field &'a str @HEAD
pxui_types::ui::Segmented impl Clone @HEAD
pxui_types::ui::Segmented impl Copy @HEAD
pxui_types::ui::Segmented impl Debug @HEAD
pxui_types::ui::Segmented impl PartialEq @HEAD
pxui_types::ui::Segmented impl StructuralPartialEq @HEAD
pxui_types::ui::Segmented struct <'a> @HEAD
pxui_types::ui::Segmented::DEFAULT const Segmented<'a> @HEAD
pxui_types::ui::Segmented::disabled fn const (self, disabled: bool) -> Segmented<'a> @U1b.6
pxui_types::ui::Segmented::enabled field bool @HEAD
-pxui_types::ui::Segmented::enabled fn const (self, enabled: bool) -> Segmented<'a> @U1b.6
pxui_types::ui::Segmented::focus_on_click field bool @HEAD
pxui_types::ui::Segmented::focus_on_click fn const (self, focus: bool) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::grow field bool @HEAD
pxui_types::ui::Segmented::grow fn const (self) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::h field Option<f32> @HEAD
pxui_types::ui::Segmented::h_px fn const (self, h: f32) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::look field Option<&'a States> @HEAD
pxui_types::ui::Segmented::look fn const (self, look: &'a States) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::min_segment field f32 @HEAD
pxui_types::ui::Segmented::min_segment fn const (self, w: f32) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::name field Option<&'a str> @HEAD
pxui_types::ui::Segmented::name fn const (self, name: &'a str) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::padding field f32 @HEAD
pxui_types::ui::Segmented::padding fn const (self, padding: f32) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::variant field u16 @HEAD
pxui_types::ui::Segmented::variant fn const (self, variant: u16) -> Segmented<'a> @HEAD
pxui_types::ui::Segmented::w field Option<f32> @HEAD
pxui_types::ui::Segmented::w_px fn const (self, w: f32) -> Segmented<'a> @HEAD
pxui_types::ui::SelectMode enum @U2b.4.33
pxui_types::ui::SelectMode impl Clone @U2b.4.33
pxui_types::ui::SelectMode impl Copy @U2b.4.33
pxui_types::ui::SelectMode impl Debug @U2b.4.33
pxui_types::ui::SelectMode impl Eq @U2b.4.33
pxui_types::ui::SelectMode impl PartialEq @U2b.4.33
pxui_types::ui::SelectMode impl StructuralPartialEq @U2b.4.33
pxui_types::ui::SelectMode::Multi variant = 1 @U2b.4.33
pxui_types::ui::SelectMode::None variant = 3 @U2b.4.33
pxui_types::ui::SelectMode::Single variant = 0 @U2b.4.33
pxui_types::ui::SelectMode::Toggle variant = 2 @U2b.4.33
pxui_types::ui::SelectionList impl Clone @HEAD
pxui_types::ui::SelectionList impl Copy @HEAD
pxui_types::ui::SelectionList impl Debug @HEAD
-pxui_types::ui::SelectionList impl Eq @U2b.1
pxui_types::ui::SelectionList impl PartialEq @HEAD
pxui_types::ui::SelectionList impl StructuralPartialEq @HEAD
pxui_types::ui::SelectionList struct @HEAD
pxui_types::ui::SelectionList::DEFAULT const SelectionList @HEAD
pxui_types::ui::SelectionList::bare field bool @U2b.4.3
pxui_types::ui::SelectionList::bare fn const (self) -> SelectionList @U2b.4.3
pxui_types::ui::SelectionList::drop_look field Option<Look> @U2b.2
pxui_types::ui::SelectionList::drop_look fn const (self, look: Look) -> SelectionList @U2b.2
pxui_types::ui::SelectionList::fit field bool @U2b.1
pxui_types::ui::SelectionList::fit fn const (self) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::header fn const (self, h: f32) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::header_height field f32 @U2b.1
pxui_types::ui::SelectionList::lead fn const (self, h: f32) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::leading_height field f32 @U2b.1
pxui_types::ui::SelectionList::measured field bool @U2b.1
pxui_types::ui::SelectionList::measured fn const (self) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::mode field SelectMode @U2b.4.33
pxui_types::ui::SelectionList::mode fn const (self, mode: SelectMode) -> SelectionList @U2b.4.33
-pxui_types::ui::SelectionList::multi field bool @U2b.4.33
-pxui_types::ui::SelectionList::multi fn const (self) -> SelectionList @U2b.4.33
pxui_types::ui::SelectionList::open_on_click field bool @U2b.1
pxui_types::ui::SelectionList::open_on_click fn const (self, on: bool) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::overlay_scrollbar field bool @U2b.1
pxui_types::ui::SelectionList::overlay_scrollbar fn const (self) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::reorder field bool @U2b.3
pxui_types::ui::SelectionList::reorder fn const (self, on: bool) -> SelectionList @U2b.3
pxui_types::ui::SelectionList::row_h fn const (self, h: f32) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::row_height field f32 @U2b.1
pxui_types::ui::SelectionList::trail fn const (self, h: f32) -> SelectionList @U2b.1
pxui_types::ui::SelectionList::trailing_height field f32 @U2b.1
pxui_types::ui::SelectionList::variant field u16 @U2b.1
pxui_types::ui::SelectionList::variant fn const (self, variant: u16) -> SelectionList @U2b.1
pxui_types::ui::Slider impl Clone @HEAD
pxui_types::ui::Slider impl Copy @HEAD
pxui_types::ui::Slider impl Debug @HEAD
pxui_types::ui::Slider impl Eq @HEAD
pxui_types::ui::Slider impl PartialEq @HEAD
pxui_types::ui::Slider impl StructuralPartialEq @HEAD
pxui_types::ui::Slider struct @HEAD
pxui_types::ui::Slider::DEFAULT const Slider @HEAD
pxui_types::ui::Slider::variant field u16 @HEAD
pxui_types::ui::Slider::variant fn const (self, variant: u16) -> Slider @HEAD
-pxui_types::ui::TableResponse impl Clone @U2b.1
-pxui_types::ui::TableResponse impl Copy @U2b.1
-pxui_types::ui::TableResponse impl Debug @U2b.1
-pxui_types::ui::TableResponse impl Default @U2b.1
-pxui_types::ui::TableResponse impl Eq @U2b.1
-pxui_types::ui::TableResponse impl PartialEq @U2b.1
-pxui_types::ui::TableResponse impl StructuralPartialEq @U2b.1
-pxui_types::ui::TableResponse struct @U2b.1
-pxui_types::ui::TableResponse::row field Option<usize> @U2b.1
-pxui_types::ui::TableResponse::sort field Option<usize> @U2b.1
-pxui_types::ui::TableView impl Clone @U2b.1
-pxui_types::ui::TableView impl Copy @U2b.1
-pxui_types::ui::TableView impl Debug @U2b.1
-pxui_types::ui::TableView impl PartialEq @U2b.1
-pxui_types::ui::TableView impl StructuralPartialEq @U2b.1
-pxui_types::ui::TableView struct @U2b.1
-pxui_types::ui::TableView::DEFAULT const TableView @U2b.1
-pxui_types::ui::TableView::sort field Option<(usize, bool)> @U2b.1
-pxui_types::ui::TableView::sort fn const (self, column: usize, ascending: bool) -> TableView @U2b.1
-pxui_types::ui::TableView::text field Option<Rect> @U2b.1
-pxui_types::ui::TableView::text fn const (rect: Rect) -> TableView @U2b.1
pxui_types::ui::Target enum @HEAD
pxui_types::ui::Target impl Clone @HEAD
pxui_types::ui::Target impl Copy @HEAD
pxui_types::ui::Target impl Debug @HEAD
pxui_types::ui::Target impl PartialEq @HEAD
pxui_types::ui::Target impl StructuralPartialEq @HEAD
pxui_types::ui::Target::Look variant (Look) @HEAD
pxui_types::ui::Target::Offset variant (f32, f32) @HEAD
pxui_types::ui::Target::Opacity variant (f32) @HEAD
pxui_types::ui::Target::Scale variant { x: f32, y: f32, anchor: (f32, f32) } @HEAD
pxui_types::ui::TextEdit impl Clone @HEAD
pxui_types::ui::TextEdit impl Debug @HEAD
pxui_types::ui::TextEdit impl Eq @HEAD
pxui_types::ui::TextEdit impl PartialEq @HEAD
pxui_types::ui::TextEdit impl StructuralPartialEq @HEAD
pxui_types::ui::TextEdit struct @HEAD
pxui_types::ui::TextEdit::anchor field usize @HEAD
pxui_types::ui::TextEdit::caret field usize @HEAD
pxui_types::ui::TextEdit::text field String @HEAD
pxui_types::ui::Toast impl Clone @HEAD
pxui_types::ui::Toast impl Copy @HEAD
pxui_types::ui::Toast impl Debug @HEAD
pxui_types::ui::Toast impl Eq @HEAD
pxui_types::ui::Toast impl PartialEq @HEAD
pxui_types::ui::Toast impl StructuralPartialEq @HEAD
pxui_types::ui::Toast struct <'a> @HEAD
pxui_types::ui::Toast::id field i64 @HEAD
pxui_types::ui::Toast::new fn const (id: i64, text: &'a str) -> Toast<'a> @HEAD
pxui_types::ui::Toast::text field &'a str @HEAD
pxui_types::ui::TreeResponse impl Clone @HEAD
pxui_types::ui::TreeResponse impl Copy @HEAD
pxui_types::ui::TreeResponse impl Debug @HEAD
pxui_types::ui::TreeResponse impl Default @HEAD
pxui_types::ui::TreeResponse impl Eq @HEAD
pxui_types::ui::TreeResponse impl PartialEq @HEAD
pxui_types::ui::TreeResponse impl StructuralPartialEq @HEAD
pxui_types::ui::TreeResponse struct @HEAD
pxui_types::ui::TreeResponse::activated field Option<usize> @HEAD
pxui_types::ui::TreeResponse::shown field usize @HEAD
pxui_types::ui::UndoKind enum @HEAD
pxui_types::ui::UndoKind impl Clone @HEAD
pxui_types::ui::UndoKind impl Copy @HEAD
pxui_types::ui::UndoKind impl Debug @HEAD
pxui_types::ui::UndoKind impl Eq @HEAD
pxui_types::ui::UndoKind impl PartialEq @HEAD
pxui_types::ui::UndoKind impl StructuralPartialEq @HEAD
pxui_types::ui::UndoKind::Redo variant @HEAD
pxui_types::ui::UndoKind::Undo variant @HEAD
pxui_types::ui::display fn (text: &str) -> String @HEAD
pxui_types::ui::ellipsis fn <'a>(width: &mut dyn FnMut(&str) -> f32, limit: f32, label: &'a str) -> Cow<'a, str> @HEAD
-pxui_types::ui::find_marker fn (text: &str, marker: &str) -> Option<usize> @U1a.1
pxui_types::ffi::PxuiDiagnostics::calls field u64 @U5.9
pxui_types::ffi::PxuiDiagnostics::has_cpu field u8 @U5.9
pxui_types::ffi::PxuiDiagnostics::has_text_layouts field u8 @U5.9
pxui_types::ffi::PxuiDiagnostics::new fn () -> PxuiDiagnostics @U5.9
pxui_types::ffi::PxuiDiagnostics::phase_cpu_nanos field [u64; 7] @U5.9
pxui_types::ffi::PxuiDiagnostics::phase_nanos field [u64; 7] @U5.9
pxui_types::ffi::PxuiDiagnostics::rebuilt field u64 @U5.9
pxui_types::ffi::PxuiDiagnostics::rebuilt_prefix field Str @U5.9
pxui_types::ffi::PxuiDiagnostics::reserved field u8 @U5.9
pxui_types::ffi::PxuiDiagnostics::size field u32 @U5.9
pxui_types::ffi::PxuiDiagnostics::text_layouts field u64 @U5.9
pxui_types::ffi::PxuiDiagnostics::timing field u8 @U5.9
pxui_types::ffi::PxuiMenuOptions::focus_on_click field u8 @U5.7
pxui_types::ffi::PxuiMenuOptions::has_name field u8 @U5.7
pxui_types::ffi::PxuiMenuOptions::max_h field f32 @U5.7
pxui_types::ffi::PxuiMenuOptions::max_rows field u32 @U5.7
pxui_types::ffi::PxuiMenuOptions::name field Str @U5.7
pxui_types::ffi::PxuiMenuOptions::padding field f32 @U5.7
pxui_types::ffi::PxuiMenuOptions::reserved field u16 @U5.7
pxui_types::ffi::PxuiMenuOptions::reserved2 field [u8; 6] @U5.7
pxui_types::ffi::PxuiMenuOptions::reserved3 field u32 @U5.7
pxui_types::ffi::PxuiMenuOptions::row_h field f32 @U5.7
pxui_types::ffi::PxuiMenuOptions::size field u32 @U5.7
pxui_types::ffi::PxuiMenuOptions::variant field u16 @U5.7
pxui_types::ffi::PxuiMenuRow::checked field u8 @U5.7
pxui_types::ffi::PxuiMenuRow::danger field u8 @U5.7
pxui_types::ffi::PxuiMenuRow::enabled field u8 @U5.7
pxui_types::ffi::PxuiMenuRow::has_id field u8 @U5.7
pxui_types::ffi::PxuiMenuRow::has_swatch field u8 @U5.7
pxui_types::ffi::PxuiMenuRow::id field i64 @U5.7
pxui_types::ffi::PxuiMenuRow::new fn (text: &str) -> PxuiMenuRow @U5.7
pxui_types::ffi::PxuiMenuRow::reserved field [u8; 2] @U5.7
pxui_types::ffi::PxuiMenuRow::selected field u8 @U5.7
pxui_types::ffi::PxuiMenuRow::shortcut field Str @U5.7
pxui_types::ffi::PxuiMenuRow::size field u32 @U5.7
pxui_types::ffi::PxuiMenuRow::submenu field Strided<PxuiMenuRow> @U5.7
pxui_types::ffi::PxuiMenuRow::swatch field Rgba @U5.7
pxui_types::ffi::PxuiMenuRow::text field Str @U5.7
pxui_types::ffi::PxuiSegmentedConfig::group field i64 @U5.7
pxui_types::ffi::PxuiSegmentedConfig::grow field u8 @U5.7
pxui_types::ffi::PxuiSegmentedConfig::has_selected field u8 @U5.7
pxui_types::ffi::PxuiSegmentedConfig::has_stored field u8 @U5.7
pxui_types::ffi::PxuiSegmentedConfig::index field usize @U5.7
pxui_types::ffi::PxuiSegmentedConfig::line field PxuiBox @U5.7
pxui_types::ffi::PxuiSegmentedConfig::min_segment field f32 @U5.7
pxui_types::ffi::PxuiSegmentedConfig::node field PxuiBox @U5.7
pxui_types::ffi::PxuiSegmentedConfig::padding field f32 @U5.7
pxui_types::ffi::PxuiSegmentedConfig::reserved3 field [u8; 7] @U5.7
pxui_types::ffi::PxuiSegmentedConfig::reserved4 field [u8; 7] @U5.7
pxui_types::ffi::PxuiSegmentedConfig::selected field usize @U5.7
pxui_types::ffi::PxuiSegmentedConfig::size field u32 @U5.7
pxui_types::ffi::PxuiSegmentedConfig::state field PxuiStateDesc @U5.7
pxui_types::ffi::PxuiSegmentedConfig::states field *const States @U5.7
pxui_types::ffi::PxuiSegmentedConfig::stored field usize @U5.7
pxui_types::ffi::PxuiSegmentedConfig::strip field PxuiBox @U5.7
pxui_types::ffi::PxuiSegmentedConfig::text field Str @U5.7
pxui_types::ffi::PxuiSegmentedConfig::variant field u16 @U5.7
pxui_types::ffi::PxuiTabsConfig::bar field PxuiBox @U5.7
pxui_types::ffi::PxuiTabsConfig::detail field Str @U5.7
pxui_types::ffi::PxuiTabsConfig::index field usize @U5.7
pxui_types::ffi::PxuiTabsConfig::node field PxuiBox @U5.7
pxui_types::ffi::PxuiTabsConfig::size field u32 @U5.7
pxui_types::ffi::PxuiTabsConfig::state field PxuiStateDesc @U5.7
pxui_types::ffi::PxuiTabsConfig::text field Str @U5.7
pxui_types::ffi::PxuiTabsConfig::variant field u16 @U5.7
pxui_types::ffi::PxuiToastsConfig::node field PxuiBox @U5.7
pxui_types::ffi::PxuiToastsConfig::size field u32 @U5.7
pxui_types::ffi::PxuiToastsConfig::state field PxuiStateDesc @U5.7
pxui_types::ffi::PxuiToastsConfig::text field Str @U5.7
pxui_types::ffi::PxuiToastsConfig::variant field u16 @U5.7
pxui_types::ffi::PxuiTooltipConfig::height field f32 @U5.7
pxui_types::ffi::PxuiTooltipConfig::lines field Slice<Str> @U5.7
pxui_types::ffi::PxuiTooltipConfig::size field u32 @U5.7
pxui_types::ffi::PxuiTooltipConfig::tip field PxuiBox @U5.7
pxui_types::ffi::PxuiTooltipConfig::variant field u16 @U5.7
pxui_types::ffi::PxuiTooltipConfig::width field f32 @U5.7
pxui_types::ui::hash_range fn (seed: i64, text: &str, first: usize, last: usize) -> i64 @HEAD
pxui_types::ui::int_key fn (seed: i64, n: i64) -> i64 @HEAD
pxui_types::ui::key_of fn (seed: i64, text: &str) -> i64 @HEAD
pxui_types::ui::shown fn (text: &str) -> &str @HEAD
pxui_types::ui::text_top_for fn (font_size: i64, size: Option<i64>, y: f32, h: f32) -> f32 @HEAD
```
