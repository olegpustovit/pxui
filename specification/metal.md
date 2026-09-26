# Metal binding contract

`prismel.metal` is the foundational Apple-Silicon Metal binding under
`ogpu_metal`. It does not depend on SDL3, OGPU, the runtime, Prismel, or UI
libraries. The runtime obtains a Metal layer through OGPU; the binding itself
never owns an SDL window. There is no CPU, browser, or OpenGL fallback.

## Build and registry

`lib/metal/discover.ml` rejects an installed macOS SDK older than 26.0 before
writing framework link flags. `lib/metal/build_bridge.ml` compiles Objective-C++
with ARC, a macOS 14 deployment target, and `-Wall -Wextra -Werror`. The build
uses the installed public SDK as the type and availability oracle. It needs the
Metal, QuartzCore, CoreGraphics, IOSurface, MetalFX, and Foundation frameworks;
Prismel does not ship their headers or binaries.

`lib/metal/gen/registry.ml` is the source for generated binding declarations.
Each entry names its Metal SDK symbol, OCaml name, type, availability, and OGPU
capability. Dune writes `metal_gen.ml`, `metal_gen.mli`, and
`metal_gen_stubs.inc` plus feature-map checks only under `_build`. Its 23-entry
`Caps.feature` map names the Metal SDK types backing each capability; the
unavailable timeline-fence feature has an explicit empty entry. The map rejects
duplicates and missing entries, and the bridge compiles its type checks against
the installed SDK. Four retained enum families are
checked against SDK constants with native `static_assert`s. `Method` entries
declare any selector over scalar, enum, NSString and handle arguments, with
an optional trailing `NSError**` and a scalar, string or owned-handle result;
`Property` entries declare getters and setters. Each becomes an external in
`Metal_raw.Registry` and a stub with a typed Objective-C receiver and direct
send, explicit availability guards, checked scalar conversion, and a
result-returning native exception boundary. The generator rejects duplicate
names/selectors, selector arity mismatches, and registry calls without a
safe-layer call; it parses OCaml source so a comment cannot satisfy the
reference check.

Blocks, callbacks, descriptor graphs, handle arrays and ownership transfer
remain handwritten in `metal_bridge.mm` and `metal_raw.ml`. No whole-SDK inventory, provenance ledger, or generated raw module
participates in the build. The registry declares `MTLSize` as a fixed scalar
record, emits its OCaml type, and checks its SDK field types. The safe
compute-pipeline `size3` re-exports that shape without another allocation.
The capability map covers every known `Caps.feature`; the unsupported timeline
fence is explicit, and OGPU conformance checks runtime capability truth.

## Safe layer and ownership

`Metal` exposes opaque handles and typed `result` errors. Before a native call,
it validates live state, same-device ownership, ranges, alignment, cardinality,
encoder state, and capability where applicable. Expected rejection does not
reach an Objective-C update method. Unknown native failures become typed
`Native_error`; Metal unavailability is an explicit `Unsupported` or startup
error rather than a no-op. Public `.mli` files hide raw values.

Native operations run on OCaml domain zero and the platform main executor. A
call from another domain returns `Wrong_domain`. The only any-domain
finalizer action enqueues an opaque retained pointer in the (unbounded)
release queue; every safe domain-zero entry drains it inside an autorelease
pool, so no Metal object is released on a GC domain.

Handles track generation, destruction, device identity, and parent dependents.
Explicit destroy is idempotent; stale use, cross-device binding, or parent
destruction with live children fails before native access. Submission resources
remain retained through completion. A blocking wait releases the OCaml runtime
lock while keeping its native command handle rooted. Copied strings, arrays,
reflection metadata, and asynchronous callback roots do not escape their
native owners. `Metal.Release_queue.stats` reports handle and queue accounting
for lifecycle checks.

The safe API covers device and capability queries, buffers and mapped ranges,
textures and views, samplers, libraries and functions, compute/render/mesh/tile
pipelines, command encoders and buffers, heaps, residency,
fences and shared events, counters, acceleration structures and function tables,
MetalFX scaling, and presentation. Metal 4
operations check their availability before use. `ogpu_metal` owns the portable
capability profile and returns typed `Unsupported` for unavailable features.

## Verification and extension

The focused binding loop is `dune build @lib/metal/runtest`; adapter behavior is
covered by `dune build @lib/ogpu_metal/runtest`. `dune build @check` typechecks,
and `dune build @all` links all targets. The native bridge is compiled under
`-Werror` during these builds. Safe tests exercise success and rejection,
resource retention, exact readback, and zero live-handle deltas. The ownership
stress runs measured handle cycles in isolated workers; presentation
fixtures exercise native lifecycle boundaries. The M1 AGX driver crashes in a
qualified Metal 4 static-linking fixture, so that fixture stays outside default
`runtest`.

`PRISMEL_METAL_SANITIZERS` accepts `address`, `undefined`, or `thread` for the
bridge build. ThreadSanitizer is exclusive; AddressSanitizer and
UndefinedBehaviorSanitizer may be combined. The MSL source path compiles at
runtime through the public Metal API; there is no offline shader artifact
pipeline.

A new binding begins with an `ogpu_metal` consumer and capability. Reuse an
existing safe operation when possible. Otherwise declare the call in the
registry (`Method` or `Property`) and let the generator write it; only
blocks, callbacks, descriptor graphs, handle arrays and ownership transfer
stay in the handwritten bridge. Add safe validation, typed errors, and a
success and rejection check. `.claude/skills/metal-workflow/SKILL.md` records
the workflow end to end; `tools/codemod` removes a binding once its last
consumer is gone.
