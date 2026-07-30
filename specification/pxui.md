# PXUI interaction and visual contract

PXUI is a wrapped sibling library. It depends on `prismel`; the core library
does not depend on PXUI. New sketches keep a `Pxui.t` in their immutable model,
feed ordered `Frame.events` through `Pxui.update`, and compose `Pxui.scene` into
their picture. Its compact creative-tool character is inspired by
[ofxUI](https://github.com/liquidzym/ofxUI), adapted to Prismel's functional
scene and event model.

## Visual language

The default theme is deliberately closer to a creative-tool control deck than
a native form:

- a near-black, slightly translucent panel with rounded corners and shadow;
- a bright cyan-green accent and restrained glow line;
- compact system UI typography;
- inset tracks and fields with quiet borders;
- distinct row hover, pressed, selected, focus, and active-drag feedback;
- accented handles and value fills that make control state readable at a
  glance.

The `theme` record keeps the palette small: `panel`, `foreground`, `control`,
`input`, `track`, and `accent`. Geometry derives secondary hover, border, and
pressed colors from those values so a custom palette retains the interaction
language.

`Pxui.create ?font ?font_size` borrows an explicitly supplied `Font.t`, or uses
`Scene.text` and the installed system UI font by default. `font_size` is in
logical points. The default font is density-aware, so text retains its layout
and sharpness on Retina output. Empty labels and field values are valid, and
changing values share Prismel's bounded renderer-local text cache.

## Coordinate model

Panel position, width, padding, row height, rendering, and hit testing all use
Prismel logical points. Event coordinates arrive in that same space because
the SDL renderer logical size performs native-to-logical pointer mapping.
PXUI must never multiply event positions by `Frame.pixel_scale`.

## Pointer state machine

PXUI tracks hover and one active left-pointer interaction:

- A button, toggle, or choice becomes armed when pressed inside its control.
  It commits only if the matching release is also inside. A release outside
  cancels the action; moving out and back in preserves the arm until release.
- A slider captures on press and emits `Slid (name, value)` whenever its
  clamped value changes during motion. Motion continues to update it outside
  the original bounds until release.
- A range chooses the nearest handle on press, captures that handle through
  release, preserves `low <= high`, and emits `Ranged`.
- An XY pad captures both axes, clamps them independently to their declared
  ranges, and emits `Moved2`.
- The final release position is applied to a captured continuous control before
  capture ends.

This state lives in the returned `Pxui.t`; `Pxui.update` never mutates its input
value and returns changes in event order. Compatibility functions (`add_*`,
`handle_event`, and `draw`) use the same semantics.

## Focus and text

Pressing a text field gives it focus. `TextInput` appends committed UTF-8,
`TextEditing` records in-progress IME composition, and Backspace removes one
complete UTF-8 scalar sequence. A press elsewhere moves or clears focus.

`WindowFocusLost` is a hard cancellation boundary: it clears pointer capture,
hover, text focus, and composition without emitting a value change. The core
Input module simultaneously clears held keys and mouse buttons, preventing a
lost release from leaving either layer active.

## Regression requirements

Tests for PXUI changes must cover:

1. press/release-inside commit and release-outside cancellation;
2. multiple captured drag movements, including positions outside the control;
3. final release position for slider, range, and XY controls;
4. nearest-handle range selection and clamping;
5. high-DPI-neutral logical hit testing;
6. focus loss during an armed or dragged interaction;
7. ordered changes from a multi-event update;
8. hover/pressed/drag scene differences and system-font rendering in a
   headless framebuffer.
