# ABI history

The current wire contract is [pxui.v1.txt](../abi/pxui.v1.txt); numeric layouts are in
[layout.v1.txt](../abi/layout.v1.txt). Historical rows stay frozen. Change the exports,
header, wrapper, snapshots and C layout assertions together; rebuild app and dylib together.
Static/dynamic validation, pixel and warm-allocation checks remain required.

| Version | Field/function | Purpose |
| --- | --- | --- |
| 1.0 | Initial public UI/text/renderer ABI | Opaque owners, sized descriptors and callback-scoped borrows. |
| 1.1 | `view_backdrop_saturation` | Control saturation after backdrop blur. |
| 1.2 | `renderer_resize` | Prepare reusable blur targets before command recording. |
| 1.3 | `TextStyle.features` | Enable OpenType tabular numbers without changing style size. |
| 1.4 | `TextSpan`, `TextSpans`, inline-text calls | Apply bounded UTF-8 text spans and decorations. |
| 1.5 | `TextStyle.ellipsis` modes | Add head and middle ellipsis, retaining full accessible labels. |
| 1.6 | `shaper_system_fonts`, `text_system_fonts` | Enable native missing-cluster font discovery explicitly. |
| 1.7 | Atlas codes 2/3, `GLYPH_PAGE_2` | Add second bounded mask/colour pages without draw breaks. |
| 1.8 | `FrameIn.appearance/reduced_motion`, theme calls | Cache host preferences and replace parsed themes between frames. |
| 1.9 | Retained view controls, `list_measured` | Expose opacity/scale/clip and variable-height virtual rows. |
| 1.10 | `Ui::signal_state` | Read scalar keyboard state while retaining routed event buffers. |
| 1.11 | `carrying_ref`, `drop_target_ref` | Borrow carry/drop payloads without owned copies. |
| 1.12 | `transfer_carry` | Keep capture on a stable ancestor as virtual rows retire. |
| 1.13 | `renderer_forget_image` | Release interned native wrappers after completed sampling serials. |
| 1.14 | `Paint::fill_fractional` | Preserve subpixel rectangle coverage in the shared shape writer. |
| 1.15 | `ui_key_down` | Query raw held keys without consuming or allocating events. |
| 1.16 | `ui_now`, retained view offset | Expose host time and compose retained motion transforms. |
| 1.17 | `ui_text_line_box` | Give text explicit line metrics and intrinsic block dimensions. |
| 1.18 | `TextStyle::WRAP_WORD_SPACES` | Retain omitted separator advances in fitting metrics. |
| 1.19 | `ui_density` | Expose the existing integer rendering density. |
| 1.20 | `ui_section_list` | Combine scrolling lead-in, pinned heading and virtual rows. |
| 1.21 | `ui_section_list_measured` | Support wrapped measured rows in section lists. |
| 1.22 | Accessible text values, analytic play shape | Expose text semantics and one-instance triangle painting. |
| 1.23 | `ui_key_pressed_with` | Query non-consuming press-time modifier combinations. |
| 1.24 | `FrameIn.environment` | Append safe-area/platform environment to frozen frame prefixes. |
| 1.25 | Numeric `Touch` event payload | Deliver typed bounded contacts without mouse emulation. |
| 1.26 | `FrameIn.keyboard`, usable-content queries | Expose keyboard geometry and docked usable bounds. |
| 1.27 | `TextInputOptions`, text-input option calls | Configure native text traits and return-key behavior. |

Frame extensions retain the frozen 48/56/128-byte prefixes. Validate leading sizes, active
records, flags and UTF-8 boundaries before mutation; partial tails never justify reading
past supplied storage. Heap ownership and unwinding never cross the ABI.
