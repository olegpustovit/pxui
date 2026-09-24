# SDL3 binding contract

The SDL3 bindings are ordinary Dune libraries under `lib/sdl3`,
`lib/sdl3_image`, `lib/sdl3_ttf`, and `lib/sdl3_mixer`.  They do not load
functions dynamically and they do not execute OCaml from a native callback.
The generated symbol inventories describe the pinned native headers; the
ownership-aware `.mli` files are the only API available to Runtime.

## Thread classes

Every public operation belongs to one of the following classes.  Runtime must
not bypass these classifications through `Private_raw`.

| Class | Public operations | Enforcement |
| --- | --- | --- |
| Pure or any-thread query | error printers; generated `Version` facts; `Version.validate`; `Version.linked`, `revision`, and extension version queries; `Thread.is_initial_domain`; `Thread.is_sdl_main_thread`; `Display.id`; immutable handle generations and modes; `Event.mouse_delta`; TTF installed-font path discovery; dropped-release counters | These either do no native work or call an SDL function whose pinned header says it is safe from any thread.  They never acquire or destroy a resource. |
| Initial OCaml domain and SDL main thread | core `Init`, `Display` except `id`, `Window`, `Clipboard`, `Text_input`, event polling/waiting, `Surface`, `Metal_view`, and explicit release draining; all image decode, TTF init/font, and mixer init/mixer/audio/track operations | The safe entry point checks both `Domain.is_main_domain` and `SDL_IsMainThread` before its native call and returns `Wrong_domain` on failure. |
| Any-domain deferred release | GC finalizers for windows, Metal views, surfaces, fonts, mixers, audio values, and tracks | A finalizer only appends an opaque release token to a bounded mutex-protected queue.  It never calls SDL.  The initial-domain safe boundary drains children before parents. |
| Blocking initial-domain call | `Sdl3.Event.wait` with a nonzero timeout and `Sdl3.Window.sync` | The stubs release the OCaml runtime system only around `SDL_WaitEventTimeout` or `SDL_SyncWindow`.  The wait timeout is copied by value, the reusable event union stays in native storage, and a synchronized window remains rooted and cannot be destroyed from another domain.  No OCaml heap pointer is retained by SDL, and the runtime is reacquired before copying or returning anything to OCaml.  A zero event timeout does not release the runtime system. |

`Init.initialized` is deliberately a result-returning initial-domain query:
the pinned SDL header marks `SDL_WasInit` as not thread-safe.  Extension init
queries follow the same safe-boundary rule even when their current state is
also mirrored in OCaml.

Handle inspection such as `destroyed` is diagnostic only.  It does not make
concurrent ownership mutation valid; create/use/destroy operations remain in
the initial-domain class.

## Callback policy

The core, image, TTF, and mixer stubs install no application callback and
contain no `caml_callback*` call.  Events are polled into one reusable native
`SDL_Event` union.  Image and font results are returned synchronously as copied
CPU bytes.  Mixer device work remains inside SDL_mixer; Prismel supplies no
OCaml audio callback.  A future native callback must use a bounded native
queue, contain no OCaml value, and be drained on the initial domain before it
can enter the safe API.

## Ownership and errors

Owned native handles have explicit idempotent destruction and a generation.
Access after destruction returns `Destroyed`; a parent with live children
returns `Parent_has_dependents`.  Borrowed strings and event payloads are
copied before returning.  Each fallible stub copies native error text into its
result before the safe layer can issue another SDL call.  Dimension, stride,
length, numeric-range, and embedded-NUL checks happen before native allocation
or decoding.

The constructor/decoder failure matrix is executable, not inferred from happy
paths:

| Boundary | Injected failure |
| --- | --- |
| `Init.init` | nonexistent SDL video driver in an isolated process |
| `Window.create` | video subsystem absent, plus invalid dimensions and embedded-NUL title |
| `Surface.create_rgba` / `of_rgba` | cardinality overflow, invalid dimensions, short stride, and short source |
| `Metal_view.create` | dummy-video window without a native Metal layer |
| SDL3_image file/byte decoders | missing file and malformed input with an explicit hint for every supported still format |
| `Font.open_file` and system discovery | missing/empty font path, invalid size, and invalid `PRISMEL_UI_FONT` |
| device/memory mixer creation | nonexistent audio driver and invalid sample-rate/channel facts |
| audio file/byte/synthesis creation | missing/malformed/empty input and invalid frequency/amplitude/duration |
| `Track.create` | destroyed parent mixer |

Native error strings are copied into immutable OCaml error records before the
next native call; the failure tests retain an error across a subsequent
version query and compare it exactly.

The binding tests run the wrong-domain matrix, a blocking-wait system-thread
probe, copied event traces, parent/child teardown, stale access, malformed
input, and 100,000-cycle ownership stress.  `tools/bench_sdl3.exe` reports the
FFI call count, wall time, OCaml allocation, collection counts, heap size, and
dropped release tokens for the same lifecycle categories.

## Extension parity fixtures

SDL3_image conformance decodes all 19 still-image formats enabled by the pinned
stable distribution: AVIF, BMP, CUR, GIF, ICO, JPEG, JPEG XL, ILBM, PCX, PNG,
PNM, QOI, SVG, TGA, TIFF, WebP, XCF, XPM, and XV.  Each fixture is decoded by
path and from copied bytes into tightly packed RGBA8.  Separate fixtures cover
alpha, exact EXIF orientation, malformed input for every decoder, and the
atomic watched-reload rule: failure preserves the borrowed wrapper, previous
surface, pixels, and generation; success changes content and generation while
preserving the borrowed wrapper.  Animation formats are outside Prismel's
existing still-image API and are not silently advertised by this binding.

SDL3_ttf conformance discovers an installed platform UI font with
`PRISMEL_UI_FONT` override semantics, then covers empty text, UTF-8, family and
style names, metrics, RGBA rasterization, mutation, and 72/144-DPI rendering.
A density-aware raster cache proves borrowed identity, immediate mutation
invalidation, and destructive least-recently-used eviction at exactly 256
entries. The high-level Font adapter uploads those bounded snapshots through
the renderer-local OGPU texture cache.

SDL3_mixer conformance covers copied-memory and file-backed sound/music loads,
device and memory mixers, play, loops, gain, fades, pause/resume, stop, generated
PCM, parent/child ownership, and invalid-driver device failure. Native runtime
tests cover the same typed sample/music operations through SDL3_mixer without a
transport or alternate audio backend.

## Packaging and discovery

All four bindings are ordinary `prismel.*` Dune libraries.  The standalone
`packaging/conf-sdl3*` opam definitions own only exact stable native dependency
probes; they do not contain implementation or build glue.  Each probe checks
pkg-config, while `tools/packaging/check_sdl3_conf.ml` additionally compiles,
links, and runs a header/runtime version probe.

One shared OCaml configurator implements discovery for the core and extension
libraries.  Dynamic pkg-config linkage is the default.  With
`PRISMEL_SDL3_LINK_MODE=static`, it requests private dependency flags and
replaces the component's `-lSDL3*` flag with a resolved archive path.  Missing
metadata or an absent archive is an error rather than a dynamic fallback.
Component-specific `*_INCLUDE_DIR` and `*_LIB_DIR` variables provide validated
explicit paths, with the explicit path ordered before pkg-config headers.

The hermetic discovery test runs every component through dynamic, static, and
explicit-path cases without depending on the host's Homebrew state, in both
development and release profiles.  The installed-consumer checker installs a
relocatable package prefix, confirms `ocamlfind` resolves every SDL3 package
inside it, and builds and executes an independent Dune project outside the
checkout.  This distinguishes source-tree success from a usable installed
package.

## Native memory qualification

`PRISMEL_SDL3_SANITIZERS` applies `address`, `undefined`, or both to every
binding stub compilation and native link.  The committed memory driver runs
the same ten tests in all lanes: core ownership, typed events, constructor
failures, 100,000-cycle stress, real CAMetalLayer lifecycle, every image
decoder, font parity/failure, and mixer parity/failure.  It treats sanitizer or
Leaks diagnostics as failures even if the child process returns success.

On the qualified Apple M1/macOS 26 host, Apple ASan's alternate signal stack
cannot be torn down cleanly after OCaml Domain tests on 16 KiB pages, so the
driver sets `use_sigaltstack=0`; ordinary, UBSan, and Leaks runs retain the full
signal-stack behavior.  AppKit window resizing also exposes a system
CoreGraphics `pdf_lexer_scan` over-read while CoreUI loads an Apple-owned theme
PDF.  The address lane uses a function-scoped interceptor suppression for that
system frame only.  The binding does not call the suppressed function, and the
same real native window lifecycle passes without suppression under UBSan and
Instruments Leaks.  No Prismel or SDL3 stub suppression is present.

The release, ASan, and UBSan builds consume the same checked
`generated_layout.json`; its byte hash is compared as part of Phase 1 evidence.
