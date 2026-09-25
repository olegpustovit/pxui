# PXUI interaction and visual contract

PXUI is a wrapped sibling library. It depends on `prismel`; the core library
does not depend on PXUI. Its compact creative-tool character is inspired by
[ofxUI](https://github.com/liquidzym/ofxUI), adapted to Prismel's functional
scene and event model.

## Architecture: one box, one pass, one draw list

`Pxui.Ui` is the only UI engine. The panel kit, the SOP inspector
(`sop_ui`), the workspace chrome (`sketch_ui`), and the graph canvas
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
   `Scene.Private.Ui_batch` instances. The published `Ui.scene` is one
   native-only `Scene.Private.ui` node plus `Scene.text_input_region`
   metadata.

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
instances never split a batch. `Prismel_next_execution.Private.lower_ui`
lowers each batch to one indexed draw with a 24-byte affine uniform (logical
canvas units to clip space) and a logical scissor, which the runtime scales
to physical pixels exactly once. `Ui` draws use direct texture bindings, so
the executor gives them their own attachment class and they never share an
argument-buffer render pass with Scene2 draws. `Scene.Private.ui` layers,
like `view3d`, ignore enclosing Scene transforms and clips: the batch
carries its own.

## Typography and the design kit

`Pxui.Theme` is the design kit: the six-colour palette (`panel`,
`foreground`, `control`, `input`, `track`, `accent`) with derived muted,
border, faint-border, hover, pressed, and invalid colours, and the kit face,
DepartureMono (or `PRISMEL_UI_FONT`), loaded once per logical size. Kit text
defaults to 11 points; panel rows are 24 points with 3 points of padding.

Glyphs are rasterized by SDL_ttf exactly as whole strings were: each code
point is rendered at the backing density (`Font.Private.glyph`), packed
white-with-coverage into one shelf atlas, and placed at the font's pen
advance for that density with the run's origin snapped to a physical pixel.
The atlas uses `Image.upload_rgba`; a failed upload stays dirty for retry.
For the kit face this reproduces whole-string rendering pixel for pixel at
1×, 2×, and 3× (only the RGB of fully transparent pixels differs). Pair
kerning of proportional overrides is not applied.

Kit rows reproduce the retired retained panel exactly: the label column is
`min(140, max(120, inner/3))` capped at half the inner width; value controls
sit at `(label, y + 3)` and are `row_height - 6` tall; toggles are 40 × 18
at the right edge; labels are dark bars with light text 8 points in; text
sits at `y + max 5 ((row_height - font_size - 3) / 2)`. `test_ui_parity`
compares a native 2× render of every kit widget with
`lib/pxui/fixtures/kit_panel_2x.png`, captured from the retained PXUI before
its removal. The only permitted differences are the corner squares of
1-point strokes (the old tessellated stroke left outer corners notched and
blended inner corners two or three times) and the XY knob, now an
anti-aliased circle instead of a 32-gon.

## Coordinate model

Panel position, width, padding, row height, layout, painting, and hit testing
use Prismel logical points. Runtime translates SDL3 logical event
coordinates into that space; PXUI never multiplies positions by
`Frame.pixel_scale`, which only selects the glyph density.

A panel with `max_height` clips rows to its padded content rectangle.
Vertical wheel/trackpad steps over it scroll by one row each and are clamped;
horizontal steps do nothing. The scrollbar is a view of the retained offset.

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
  own shortcuts.
- `PointerCancelled` ends capture but keeps text focus. `WindowFocusLost`
  ends capture and clears hover, focus, and composition.

## Hosts

- `Pxui.Camera_control` / `Camera2_control` build Camera and Render sections
  into the current panel (`widgets`), expose `toggle_ui`/`open_camera` for
  host key bindings, and navigate in a control area (`navigate`); `panel`
  combines them for standalone sketches. Sliders read the camera each frame.
- `Pxui.Settings` persists model values in the original `PXUI1` format.
- `Sop_ui.Node_inspector.widgets` builds a node's parameter rows from its
  schema each frame (folders become accordions, keys are field names) and
  applies edits through `Node.apply_parameters`; nothing is synchronized
  back.
- `Pxui_graph.update view ui frame` builds the graph canvas: a clickable,
  scrollable canvas box and one box per visible tile, keyed by node id, with
  VIEW-button and output-port children. The graph's spatial index still culls
  tiles and resolves input-port and wire hits; wires are Béziers hit by
  distance to the flattened curve, and the dot grid is one quad. Tiles keep
  the retained integer screen geometry so graph labels stay pixel-identical.
  Committed node drags rebuild the edge BVH from stored positions; a click
  without motion leaves it alone. Parameter-only document edits keep layout,
  edges, and the spatial index.
  The node menu (host-opened, `Pxui_graph.open_menu_at`) uses `Ui.popup`
  around `Ui.picker`, whose search row takes focus in the frame it opens; a
  right click opens `Ui.context_menu` for the canvas, a tile, or a wire.
- `Ui.popup` uses the last laid-out rectangle for outside-press dismissal;
  an estimated height is used only until the first layout. `Ui.modal` and
  `Ui.context_menu` share that dismissal path. `Ui.modal` centers a panel
  using its last height. Escape and focus loss also dismiss. The node menu
  uses `Ui.fuzzy_match` on labels, keys, and category breadcrumbs, as the
  preset picker does. `Ui.picker` retains its
  cursor and armed-delete row; the host keeps the query and recomputes rows as
  typing changes it. Their golden is `fixtures/kit_overlays_2x.png`.
- `Sketch_ui` builds the whole workspace — pane backgrounds, splitters,
  headers, graph, inspector, status — in one `Ui.frame` per application
  frame. `Environment3.update_with ~inspector` adds sketch-owned kit widgets
  below the camera sections.

## Regression requirements

Tests for PXUI changes must cover press/release commit and cancellation,
captured drags beyond bounds, final release positions, nearest range
handles, focus loss and pointer cancellation, text entry with UTF-8 and IME,
numeric-label editing, bounded scrolling, accordions, canvas transforms,
cached subtrees, and identical behaviour at 1× and 2× (`lib/pxui/test_ui`);
native pixel parity of the kit (`test_ui_parity`); exact UI-pipeline
coverage against Scene2 geometry (`prismel_next_execution/test_ui_pipeline`);
and the graph, inspector, and workspace contracts (`test/test_pxui_graph`,
`test/test_sop_ui`, `test/test_sketch_ui`).

## Undo history

`Pxui.Undo` is the one bounded immutable history that higher-level editors
share instead of keeping private stacks. `commit` makes the current value
undoable and installs a new one (clearing redo), `amend` replaces the current
value without an entry so a continuous pointer edit collapses into one step,
and `undo`/`redo` walk the stack within a fixed capacity. `Sketch_ui` keeps
its editable `Edit_graph.t` document in one: graph-pane edits, node creation,
paste, delete, and inspector parameter commits are entries, slider drags held
under the primary button are amended into the entry opened at press, and
Command/Ctrl-Z, Shift-Command/Ctrl-Z, and Ctrl-Y step it.
