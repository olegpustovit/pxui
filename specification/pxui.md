# PXUI interaction and visual contract

PXUI is a wrapped sibling library. It depends on `rays`; the core library
does not depend on PXUI. Its compact creative-tool character is inspired by
[ofxUI](https://github.com/liquidzym/ofxUI), adapted to Rays's functional
scene and event model.

## Architecture: one box, one pass, one draw list

`Pxui.Ui` is the only UI engine. The panel kit, the parameter inspector
(`Pxui_shell.Inspector`), the workspace chrome (`pxui_shell`, `rays_editor`), and the graph canvas
(`pxui_graph`) all build boxes in the same `Ui` frame, share one pointer
capture, one focus, and one hit list, and paint into one instance list.

```ocaml
let update model frame =
  Pxui.Ui.frame model.ui frame @@ fun ui ->
  Pxui.Ui.panel ui "Motion" @@ fun () ->
  let animate = Pxui.Ui.toggle ui "Animate" model.animate in
  let radius = Pxui.Ui.slider ui "Radius" ~range:(10., 120.) model.radius in
  if Pxui.Ui.button ui "Quit" then Sketch.quit ();
  { model with animate; radius }

let view model _frame = scene_of model @ Pxui.Ui.scene model.ui
```

Widget values live in the sketch model; widgets return them. There are no
named change lists, value getters, or setters to keep in sync. `Ui.t` is a
mutable cache handle threaded through the model like `Assets`, released with
`Ui.destroy` from `Sketch.run_state ~on_stop`.

Each frame runs four steps:

1. **Route.** The frame's ordered events are resolved against the previous
   frame's hit list (paint order, clipped). The topmost box under a press
   becomes the single `active` box and captures the pointer until the
   matching release; clickable presses also update keyboard focus. Wheel
   steps go to the topmost `scroll` box under the pointer unless a
   `blocking` box covers it. Key, text, and IME events go to the focused box.
   One frame of input latency buys a single build pass.
2. **Build.** User code creates boxes. A box key hashes its label (text after
   `##` is key-only, `###id` replaces the key) with the enclosing box, so
   state follows labels. Duplicate keys in one frame receive order-stable
   distinct keys. Per-key state — rectangles, scroll offsets, accordion
   state, text-editing buffers, double-click timing, cached subtrees — lives
   in a retained cache: an open-addressing integer table into structure-of-
   arrays pools. A key not built for a frame is pruned.
3. **Layout.** A bottom-up pass computes `Px`, `Text`, and `Fit` sizes; a
   top-down pass resolves `Pct`, `Rel`, and `Grow`, reserves a 10-point
   gutter in overflowing scroll boxes, clamps scroll offsets, and positions
   flow and floating (`at`) children. A box with `xform (scale, tx, ty)` is a
   canvas whose children use canvas units.
4. **Paint.** Boxes paint depth-first with nested clip rectangles, culling
   boxes outside their clip. Painters receive the final rectangle and emit
   `Scene.Private.Ui_batch` instances. `Ui.to_front ~order` raises a root in both
   painting and hit testing. The published `Ui.scene` contains native-only UI
   batches plus `Scene.text_input_region` metadata. `Ui.scene ~under` can insert
   a host Scene immediately before a raised root's UI batch, letting floating
   native viewports share the UI's paint order without another hit-test path.

`Ui.cached ~key ~stamp` replays last frame's boxes and painters for a
non-interactive subtree while its stamp is unchanged.

## Renderer

A UI instance is 64 bytes: a quad plus a kind. One Metal pipeline family,
`Ui`, draws them with vertex pulling (four vertices per instance) and one
signed-distance fragment stage:

- **Rect.** Radius-0, non-anti-aliased rects rely on quad rasterization, so
  fills cover exactly the pixels a triangle rectangle covers under the
  top-left rule. A 1-point stroke is a separate band instance whose inner
  test biases the sample point by +10⁻³ toward +x/+y, reproducing the
  top-left rule on half-integer edges at every integer density. Rounded
  rects, circles, and borders on them use an anti-aliased rounded-box SDF.
- **Textured.** Glyphs and images sample one texture with nearest filtering.
  Instances carry texel coordinates, normalized by the texture's size in the
  fragment stage, so the atlas can grow without invalidating earlier
  instances.
- **Wire.** A cubic Bézier becomes 4–48 segment instances by length; the
  vertex stage evaluates the curve and strokes each chord as an anti-aliased
  capsule. Chords outside the batch clip are not emitted.
- **Grid.** One quad draws a dot every `spacing` points from an origin.

`Ui_batch` groups instances by `{clip, xform, texture}`; untextured
instances never split a batch. `Rays_execution.Private.lower_ui`
lowers each batch to one indexed draw with a 24-byte affine uniform (logical
canvas units to clip space) and a logical scissor, which the runtime scales
to physical pixels exactly once. `Ui` draws use direct texture bindings, so
the executor gives them their own attachment class and they never share an
argument-buffer render pass with Scene2 draws. `Scene.Private.ui` layers,
like `view3d`, ignore enclosing Scene transforms and clips: the batch
carries its own.

## Typography and the design kit

PXUI records the nearest hit ancestor of each box. A pane root's
`Ui.signal.subtree_press` reports the last press on that root or a nested
control, while `pressed` remains exclusive to the topmost box.
`Ui.last_press_within` reads that same hit tree before frame building so a
click and a scoped key in one event batch use the clicked pane.

`Pxui.Theme` is the design kit, revision 3. The reference is the "Rays UI Redesign" canvas
(foundations, widgets, workspace, overlays and one sheet per panel); `sketches/ws_layout` is the
layout it is compared with, and `tools/ui_shot.exe FILE.rays OUT.png` renders any workspace's
editor to a PNG without a window.

Tokens. The six base colours are unchanged (`panel` the ground, `foreground` ink, `control` the
current row, `input` the sheet, `track` the well, `accent` signal orange `#f0481f`); invalid is
magenta `#c2255c` and the port colours are unchanged. Derived: text is ink, `ink_2` (labels and
secondary text, 4.8:1 on the ground; `muted` is its old name) and `ink_3` (disabled, placeholder,
keys); lines are the foreground at 30, 15 and 8 percent (`border`, `edge`, `faint_border`: the one
outlined button and switches, panes and fields, inside a list); fills are `hover_fill`,
`pressed_fill` and `tint` (the accent at 16 percent: a selection in text, a dock target). Type has
four sizes, 40, 20, 13 and 11 (`display_size`, `title_size`, `font_size`, `label_size`); a label is
upper case with a point of tracking (`Ui.Paint.cap`). One unit, 24 points, is a row, a header, a
bar and the graph's grid pitch; a control is 20. Nothing has a radius, a shadow, a gradient or an
ink fill: separation comes from space first, a hairline second, a rectangle last.

Marks (`Ui.Paint`): corner `brackets` in the accent for the selected object (a node, a list row),
an accent underline for keyboard focus, a `cross`ed box for nothing displayed, a `hatch` for
bypassed or stale, a dot every 24 points and a register cross every 480 by 192 on the canvas,
`dashed` lines for scopes and drop targets, and the drawn `chevron`.

Widgets. A field is a value on an `edge` hairline (accent while it holds the keyboard, invalid
when refused); the 2-point `ink_2` line under a number is its position in the soft range. A
button is its text and, in `ink_3`, its key: hover and pressed fills, the `control` fill while
active, and one outlined primary button per panel (`Pxui_shell.Kit.button`). Tabs are text, the
one in use underlined (`Kit.segments`). A switch is 28 by 14 with an 8-point knob, accent when on.
A section is a label in `ink_3` with a chevron under 16 points of space (4 at the top of its
parent, none after a closed section). A list row is 24 points: hover is `faint_border`, the
keyboard cursor the `control` fill, the selection that fill inset by 4 with accent brackets. A
menu or a window is the `input` sheet with one hairline; a docked panel is the ground. The
scrollbar is a 4-point `border` thumb with no track.

Chrome and panels. The status strip reads, left to right: the workspace, a dot for its state
(checked, cooking, refused), the status line, a hairline, the focused pane's context as a label
and each of its keys before what it does, then the layout in use and the frame rate
(`Pxui_shell.Status_bar`). A docked timeline is one bar with the ruler at its end; a taller one
puts the ruler under the bar with numbered major ticks and the last frame at the bar's end. The
leader is a sheet along the bottom with its sections in columns; the key sheet has a filter
field. The Outline lists, under a search field and an add button, the scene with its objects
and their visible and render flags, geometry and materials with how many graphs use each, the
World, the layouts (a row switches to it), the active graph's inputs and its data flow. Over a
view the host draws, on patches of the ground, the render frame (a hairline with ink corner
brackets and its resolution) where it is not the whole pane, the focused object's accent
brackets and name, the render camera at the upper right, the scene's size and the frame rate
at the lower right, and a traced view's samples with a 4-point progress bar at the upper left.
A window's panes are built on the `input` sheet (`Core.on_sheet`, `Scope.with_theme`).

The kit face is Pragmasevka (or `RAYS_UI_FONT`). Each `Ui.t` loads it once per logical
size, found from the working directory or the executable upward, caches a
failed load (falling back to the system face), and frees its faces in
`Ui.destroy`. Kit text defaults to 13 points and rows are `max 24 (size + 11)`, so 24 up to that
size; `RAYS_UI_FONT_SIZE` overrides the editor's size (the tests set 11, the size their pointer
positions were written for); chrome and inspector text take the kit size, never a literal one.
A line of `Ui.text_area` is `Ui.text_line_height`, one and a half times the text (17 points at 11,
20 at 13), not a 24-point control row; its gutter holds label-size line numbers in `ink_3` behind
a `faint_border` rule, the caret's line is tinted with `faint_border`, an error line with the
invalid colour at 7 percent, and the bracket pair at the caret is outlined in the accent.
Compact string fields (including node names and notes) align left; compact numeric
fields align right. Choice fields carry a drawn chevron, and their popup
marks the current choice with a 6-point accent square and spans at least the control's width,
capped to the frame.
The editor inspector uses zero outer panel padding. Its shared
`Ui.inspector_header` (the selected thing's name at the display size, once per panel),
`inspector_section`, `inspector_row`, `inspector_toggle`, `inspector_button`,
`inspector_readout`, and `inspector_message` keep the
selected node, empty selection, settings, viewport, camera, and render
contexts on one grid: 12 points, a 6-point slot for the pin dot (filled once the row is on the
card), the label in `ink_2`, the control, 12 points. Parameter rows are 24 points high; a label
too long for its column takes a 48-point row with the control underneath. A switch row gives the
label the row and puts the switch at its end.

Glyphs are rasterized by SDL_ttf exactly as whole strings were: each code
point is rendered at the backing density (`Font.Private.glyph`), packed
white-with-coverage into one shelf atlas, and placed at the font's pen
advance for that density with the run's origin snapped to a physical pixel.
The atlas uses `Image.upload_rgba`; a failed upload stays dirty for retry.
For the kit face this reproduces whole-string rendering pixel for pixel at
1×, 2×, and 3× (only the RGB of fully transparent pixels differs). Pair
kerning of proportional overrides is not applied.

Kit rows: 12 points at each side, a label column of 112 points (half of a narrow row), then
the control at `y + 2`, `row_height - 4` tall; a switch sits at the right edge; text sits at
`y + max 4 ((row_height - font_size - 3) / 2)`. `test_ui_parity`
compares a native 2x render of every kit widget with
`lib/pxui/fixtures/kit_panel_2x.png` pixel for pixel; the goldens are rendered with
DepartureMono (the test sets `RAYS_UI_FONT`) and are regenerated only when the kit's design
changes, by running the test with `RAYS_UPDATE_FIXTURES=<dir>`.

## Coordinate model

Panel position, width, padding, row height, layout, painting, and hit testing
use Rays logical points. Runtime translates SDL3 logical event
coordinates into that space; PXUI never multiplies positions by
`Frame.pixel_scale`, which only selects the glyph density.

A panel with `height` fills its pane; `max_height` caps a content-sized panel.
Both clip rows to the padded content rectangle.
Vertical wheel/trackpad steps over it scroll by one row each. Beyond an edge,
the content stretches with the macOS rubber curve and springs back after input
stops. A positive vertical SDL delta moves the content down, matching a
downward natural trackpad or Magic Mouse gesture; an ordinary wheel follows
SDL's normal direction. Horizontal steps do nothing. The scrollbar shows the
bounded offset.

## Pointer and keyboard contract

- Buttons, toggles, choices, and accordion headers commit only when the press
  and release both land inside their control; a release outside cancels.
- Sliders, ranges, and XY pads follow the captured pointer outside their
  bounds and clamp to their drag range; the release position applies before
  capture ends. A range keeps the nearer handle chosen at press.
- Integer sliders snap and return integers.
- Double-clicking a slider's label (0.35 s, 5 points) opens an inline
  numeric editor that takes focus; the first numeric character replaces the
  value, Enter commits a finite (or strict integer) value even beyond the
  soft range, Escape cancels, and a press elsewhere commits valid text and
  drops invalid text.
- Pressing a text field focuses it: `TextInput` appends UTF-8, `TextEditing`
  shows IME composition, Backspace/Delete remove one scalar value. A press
  elsewhere clears focus. `Ui.text_input_focused` lets hosts suppress their
  own shortcuts. Long values scroll horizontally to keep the caret visible;
  Command-Left/Right reveal the start/end, and dragging past either edge
  extends the selection while scrolling.
- `PointerCancelled` ends capture without a release (no click, drag commit,
  or context click) and keeps text focus. `WindowFocusLost` ends capture the
  same way and clears hover, focus, and composition.

- A carry holds one payload on the handle: a `kind` and a `value`, both strings. `Ui.carry ~from`
  holds it once the pointer is 4 points from the left press of the capture owner (a widget calls it
  every frame it is pressed); `Ui.carry` without `~from` holds it at once from a key, with no
  capture, and a left press is then the put (nothing else sees that press). `Ui.drop_target ui box`
  answers `Hover payload` while the topmost box under the pointer is the box or inside it (hover is
  computed for every box during someone else's drag) and `Dropped payload` on the frame of the
  release there, once. The payload is gone after the release, a `PointerCancelled` or a
  `WindowFocusLost`; `Ui.cancel_carry` is the host's cancel and keeps the pressed box from
  carrying again before its next press. The ghost (the value as a label by the pointer) is a root
  box raised with `to_front ~order:max_int` and painted with `draw_over` at the end of `Ui.frame`;
  a frame with nothing carried builds and paints nothing new, so the parity fixtures do not move.
  `Ui.text_area_submit ~on_drop` reports, while a payload is held over the area, the byte under the
  pointer and whether it hovers or was released there (the carry's text caret). There is still one hit
  list and one capture. See `flow.md` §7.12.

## Hosts

- `Pxui.Camera_control` / `Camera2_control` build Camera and Render sections
  into the current panel (`widgets`), expose `toggle_ui`/`open_camera` for
  host key bindings, and navigate in a control area (`navigate`); `panel`
  combines them for standalone sketches. Their controls use the shared
  inspector rows and read the camera each frame.
- `Editor_core.Store.Settings` persists model values in the versioned Rays JSON
  envelope and reads existing `PXUI1` files.
- `Pxui_shell.Inspector.fields` builds standalone parameter rows from a
  schema each frame (folders become inspector sections, keys are field names) and
  applies edits through `Node.apply_parameters`. `Inspector.flow_fields`
  builds editor rows from a Flow node: card pins (●/○), grouped vec3 controls
  and `xyz` split, drive source and applied-value display, and reset. It emits
  typed requests for the host to apply after the PXUI frame.
- `Pxui_graph.update view ui frame` builds the graph canvas: a clickable,
  scrollable canvas box and one box per visible tile, keyed by node id, with
  VIEW-button and output-port children. The graph's spatial index still culls
  nodes and resolves input-port and wire hits. Nodes flow left to right;
  geometry sockets sit in the header or input rows. Named compound outputs
  appear in the header and output rows; a new geometry wire retains its source
  output name. Wires are polylines with
  14-point stubs and authored bends, hit by exact segment distance through
  the existing wire BVH. The dot grid is one quad with antialiased dots on a
  continuous 24-point graph pitch. Scrolling zooms at the pointer, and dragged
  tile boxes move in the frame that receives the pointer motion.
  Compound bodies hide VIEW controls; their display selection stays in the
  enclosing SOP network.
  Committed node drags rebuild the edge BVH from stored positions; a click
  without motion leaves it alone. Parameter-only document edits keep layout,
  edges, and the spatial index unless exposure changes the node's height.
  The node menu (host-opened, `Pxui_graph.open_menu_at`) uses `Ui.popup`
  around `Ui.picker`, whose search row takes focus in the frame it opens; a
  right click opens `Ui.context_menu` for the canvas, a tile, or a wire.
  Levels point/chip/card/full have zoom caps and explicit pins; cards
  show primary and changed parameters. Numeric `Ui.value_field` controls set
  soft-range values from pointer position; Option-click or label double-click
  opens the shared text editor. Alt-click adds
  or removes bends, Alt-drag pans, and Command/Ctrl-drag cuts crossed wires
  in one transaction. `o`/`p` and their Shift variants change detail levels.
  Contextual Tab, repeat, letter hints, connection walking, bypass, dissolve,
  find and framing share the Command table with guide membership. The host's
  status strip and grouped key sheet read that table; 380 ms tooltips use
  `Ui.hover_delay` and noninteractive PXUI overlays. The workspace pane (`Scope`) adds a
  marquee (a left drag on empty canvas), titled frames (Shift-G) and inline name, input-default
  and frame-title fields, all built from `Ui.box` and `Ui.value_field`. Value tiles, parameter
  sockets and drive wires use the same hit tree and spatial index. Tab search
  filters destinations by compatible value ports when started from a value
  output.
- `Ui.popup` uses the last laid-out rectangle for outside-press dismissal;
  an estimated height is used only until the first layout. `Ui.modal` and
  `Ui.context_menu` share that dismissal path. `Ui.modal` centers a panel
  using its last height. Escape and focus loss also dismiss. The node menu
  uses `Ui.fuzzy_match` on labels, keys, and category breadcrumbs, as the
  preset picker does. `Ui.picker` retains its
  cursor and armed-delete row; the host keeps the query and recomputes rows as
  typing changes it. Their golden is `fixtures/kit_overlays_2x.png`.
- `Rays_editor` builds the whole workspace — pane backgrounds, splitters,
  headers, graph, inspector, status — in one `Ui.frame` per application
  frame.  The panes are the leaves of a `Pxui_shell.Layout` tree (W10): every
  leaf has a 24-point header (its kind as a label, a breadcrumb, its tools, the accent square of
  the focused pane), a right-click menu on it (split, close, retype)
  and a collapse chevron (a window has dock and close); a splitter is a one-point gutter whose seven-point drag
  target is built after the panes, so a neighbour's hit rectangle never covers it,
  and a drag is view state until release (one edit of the editor graph, one history
  entry; the same three edits are the keys `Space o h/v/x` and `Space o g/l/t/i/u/m/w` on the
  focused panel).  Every leaf is an instance with its own view state, however many of a kind the layout has
  (`flow.md` §11.11); box keys are seeded by the enclosing box, an explicit `###id` included, so two
  instances never share widget state.  The graph canvas paints its grid, zones and wires in a clipped
  child, so a pane beside it is never painted over. Its read-only Flow text projection uses the same pane hit tree for
  binding selection and a scrollable, clipped body; the Flow list and text
  views use PXUI's retained elastic scroll state. It caches canonical text
  until the network, definitions, display, or qualified-name setting changes.
  `Editor3.update_with ~inspector` adds sketch-owned kit widgets
  below the camera sections.

## Regression requirements

Tests for PXUI changes must cover press/release commit and cancellation,
captured drags beyond bounds, final release positions, nearest range
handles, focus loss and pointer cancellation, text entry with UTF-8 and IME,
numeric-label editing, bounded scrolling, accordions, canvas transforms,
cached subtrees, and identical behaviour at 1× and 2× (`lib/pxui/test_ui`);
native pixel parity of the kit (`test_ui_parity`); exact UI-pipeline
coverage against Scene2 geometry (`rays_execution/test_ui_pipeline`);
and the graph, inspector, and workspace contracts (`test/test_pxui_graph`,
`test/test_sop_ui`, `lib/pxui_shell/test_shell`, `test/test_rays_editor`).

## Undo history

`Editor_core.History` is the one bounded immutable history that higher-level editors
share instead of keeping private stacks. `commit` makes the current value
undoable and installs a new one (clearing redo), `amend` replaces the current
value without an entry so a continuous pointer edit collapses into one step,
and `undo`/`redo` walk the stack within a fixed capacity. `Rays_editor` keeps
its editable `Edit_graph.t` document in one: graph-pane edits, node creation,
paste, delete, and inspector parameter commits are entries, slider drags held
under the primary button are amended into the entry opened at press, and
Command/Ctrl-Z, Shift-Command/Ctrl-Z, and Ctrl-Y step it.

## Graph zones (workspace pane, W4)

The graph pane of a workspace document adds tokens only: `Theme.zone_for`,
`zone_fold`, `zone_sum`, `zone_fn` and `zone_let` (fill, edge, dashed; light
and dark from the study's palette), and `Theme.ports` entries `text`, `fn`
and `record` (a list draws its element colour on a stacked socket, a function
a diamond, a record a wide pill). `Theme.dark` tells the two palettes apart.
`test_ui_parity` guards them with `fixtures/kit_zones_1x.png`. The
iteration selector is built from ordinary boxes; no widget was added to `Ui`.
The W5 footers (value, sparkline, tags, hoist button) are painted and hit-tested
the same way: no new token or widget.

`Ui.text_area` (W7) is the multiline sibling of `text_field`: one box (clickable,
focusable, scrolling) with a line-number gutter, sharing `text_field`'s focus,
IME composition, clipboard and edit events; it adds Enter, Up/Down, line-scoped
Home/End, Escape to leave, `?readonly`, `?errors` (gutter marks) and `?spans`
(tinted ranges). It draws only additive pixels, so no kit fixture changed. Gap B added Tab (two
spaces; Shift-Tab takes them off; the box carries `Ui.keep_tab`, so focus traversal leaves it alone),
`~wrap` (rows of a monospaced line, the gutter numbers logical lines) and `text_area_submit`, which also
reports Command/Ctrl-Enter (the text pane's apply). `Ui.key_pressed` lets a dialog see its Enter.
A `?language` (`Ui.language`: `colorize`, `brackets`, `indent`, `pairs`) makes it a code editor without
teaching it a language: coloured runs by byte span, the bracket pair at the caret lit, Enter followed by
the language's indentation, openers typed in pairs (wrapping a selection), closers stepping over
themselves and Backspace taking an empty pair; `?on_context` reports a right-click for the host's menu.
Every text widget edits as macOS does (word selection by double click, line by triple, Option and
Command with the arrows, Backspace and Delete, Shift extending, Command-Z/Shift-Command-Z undo and
redo kept per focused text with consecutive typing as one step); `Ui.signal.clicks` counts a
double or triple click.  A language's optional `rewrite` runs after each edited frame and maps
the caret: the editor's Lisp supplies parinfer's indent mode (`Lisp_text.parinfer_text`), toggled
from the text pane's right-click menu.
`Ui.context_menu` is as wide as its longest row and an empty label is a separator. `Ui.set_font_size`
scales every kit text and row between frames (the editor's Command +/-).
