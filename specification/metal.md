# Metal binding contract

The Metal binding is an ordinary Dune library under `lib/metal`, published as
`prismel.metal`. It is a foundational library beneath `ogpu_metal`; it has no
dependency on SDL3, OGPU, Runtime, Prismel, or PXUI. Runtime combines an
SDL3-owned `CAMetalLayer` with this binding through `ogpu_metal`, but the
binding itself never owns an SDL window.

## Generated SDK inventory

`tools/metal/generate_inventory.exe` is an OCaml/Dune tool. It invokes Clang's
Objective-C++ AST dump against the selected macOS SDK, parses the dump in OCaml,
and writes `lib/metal/generated_api_inventory.json` plus compiled provenance.
No Python generator or orchestration layer participates in the build.

The inventory is pinned to macOS SDK 26.5, an arm64 macOS 14.0 deployment
target, and the public headers from Metal.framework plus
`QuartzCore/CAMetalLayer.h`. Header paths, sizes, individual hashes, aggregate
hash, compiler identity, declaration identity, signatures, availability, and
classification-source hash are recorded. Anonymous typedef records and enums
are resolved to their public names, empty Objective-C categories are normalized
to their owning class, and generation fails if a reviewed identifier is absent
or duplicated.

Inventory coverage is incremental and honest. A symbol becomes `bound` only
when the ownership-aware safe layer implements it. Declarations unavailable on
macOS are `scope-excluded`; remaining declarations stay `unreviewed` until the
corresponding Phase 2 slice lands. M1 is not green until the generated inventory
contains no unreviewed in-scope declaration.

The schema-2 pinned inventory contains 5,286 declarations: 3,322 `bound`, one
`availability-gated`, 37 `scope-excluded`, and 1,926 `unreviewed`.

Binding generation is a hybrid OCaml/Dune architecture. The declarative binding
plan is keyed by identifiers from that pinned inventory and repeats the expected
owner, kind, pinned normalized signature, and explicit availability contract.
The inventory also records every enum value Clang materializes; a generated enum
argument is accepted only when all cases in its SDK family have recorded values,
and it names that complete expected family and its exact values. Plan resolution
is fail-closed for an absent declaration, SDK/header/signature/enum-value drift,
duplicate identifier, stale plan provenance, exact generated-symbol collision,
or handwritten/generated overlap.

The complete `MTLArgument.h` method/property reflection family is one
101-declaration generated batch: 57 statically typed direct selectors and 44
property companions. Pipeline reflection immediately snapshots nested array,
pointer, structure, texture-reference, and tensor-reference metadata into
immutable OCaml values. Conversion is bounded to depth 32 and 65,536 structure
members. No reflection object escapes the native call; strings and member
arrays are copied during synchronous traversal, and focused success, limit,
and injected-failure tests require zero native-handle delta. A real M1 pipeline
fixture validates buffer-pointer, structure, member lookup, and array ownership
behavior. The plan rejects owner, selector, signature, availability,
classification, or header-closure drift.

Acceleration-structure scalar descriptor state is generated as one 419-ID M7
batch across 23 legacy and Metal 4 owners: 140 immutable public record fields,
140 typed getters, and 139 typed setters. Constructors reject negative
`NSUInteger` representations before the native boundary. Generated direct
Objective-C accessors retain exact SDK types and per-selector availability.
The M1 conformance lane reaches real sizing for triangle, bounding-box, curve,
all three motion geometries, primitive, direct-instance, and indirect-instance
graphs, and executes triangle build, refit, and copy. Its ten Apple9-dependent
Metal 4 owner lanes reject before descriptor/resource allocation; those execute
on the M3+ lane.

The companion 146-ID acceleration ownership batch covers 49 buffer,
buffer-range, copied-string, retained-array, and resource-ID properties plus
97 getter/setter companions across 22 owners. `Metal.Acceleration_structure`
owns a device-attached native handle, and its immutable triangle descriptor
consumes checked `Buffer.t` values. Size, allocation, build, refit, copy,
compacted-size readback, and compact-copy operations enforce live handles,
same-device identity, nonnegative overflow-safe ranges, exact native scratch
requirements, command-encoder state, and completion-owned resource retention.
Generated property materializers use typed direct Objective-C calls; copied
strings/arrays cannot escape autorelease ownership, and injected retain failure
unwinds every acquired child. M1 conformance executes the complete safe command
lifecycle and checks explicit capability rejection for unavailable Metal 4
owners.

Clang's JSON availability attributes identify source expansions rather than
carrying a directly reusable introduced-version payload. The OCaml inventory
generator resolves every direct attribute to an exact expansion site in a
cached public Metal or CAMetalLayer header; malformed, conflicting,
internal-only, or non-public locations fail generation. It parses the exact
header line with a strict OCaml parser for `API_AVAILABLE`, `API_DEPRECATED`,
and `API_DEPRECATED_WITH_REPLACEMENT`, rejecting malformed versions, overflow,
duplicate macOS clauses, and ambiguous macro text instead of guessing.

Availability on a protocol, interface, category, enum, record, or typedef is
carried lexically to its members and combined with each member's direct sites.
The effective macOS introduction is the maximum of those inherited and direct
versions. Duplicate declaration collapse unions the evidence and fails if the
same normalized source line resolves inconsistently. Schema 2 records the
result on every symbol as canonical nullable `macos_introduced` plus a sorted,
deduplicated `availability_sources` array of `{header, line}` objects. An exact
`API_UNAVAILABLE(macos)` source overrides inherited introductions, leaves
`macos_introduced` null, and preserves `scope-excluded`; unavailability for a
different platform does not corrupt the macOS classification. Stable ordering,
pinned inputs, content hashes, and emitted provenance keep generation
deterministic and make Dune's generated-output regression checks fail on drift.

Clang may emit a typedef-owned anonymous enum or record as separate forward,
definition, and typedef roots. The inventory joins only the exact immediate
`ownedTagDecl` and `previousDecl` identity family, unions declaration-level tag
and typedef availability in both directions, and then carries that evidence to
the tag's cases or fields. It never infers ownership from matching type names or
promotes member-only evidence to a sibling. Conflicting owners, malformed sites,
and order-dependent or duplicate-root results fail generation.

The plan schema and direct-call entries are split into independent OCaml modules
for render, compute, and resource work. Their provenance is one sorted,
length-framed digest over every schema, shard, and aggregator source, so moving
an entry out of the aggregator cannot make an untracked plan input. Native
adapters use direct, statically typed Objective-C selectors; neither generated
nor handwritten code dispatches through `objc_msgSend`.

The native receiver catalog accounts for all 37 existing bridge `Handle_kind`
values before bulk selector generation. It has 31 concrete SDK receiver
mappings, explicit `MTLResource` and `MTLAllocation` polymorphic mappings, and
six documented non-SDK/state exclusions. General and placement roles for
`MTL4CommandQueue` remain distinct, while compiler-task, Metal 4 command-buffer,
and argument-table handles record their wrapper-property access instead of
pretending that the wrapper itself is the SDK object. A test compares the exact
catalog partition with the native enum, requires every SDK owner in the pinned
inventory, and checks the declared direct/helper/wrapper recovery evidence in
the bridge.

A handle-backed callable batch is now integrated. It generates 59 statically
typed raw selector calls and records 40 property declarations, covering 99 new
unique inventory IDs. Together with the checked generator plan's prior ten
primary methods and two property companions, the generated callable
primary/companion closure now contains 111 unique declarations. The 99 new
declarations remain private and `unreviewed`; they are not described as safely
bound.

The integrated production-scale mechanical enum batch selects 61 complete
families: 61 `unreviewed` enum declarations, 61 `unreviewed` typedef companions,
and 326 cases, or 448 inventory declarations in total. The cases split into 301
in-scope `unreviewed` declarations and 25 macOS-unavailable `scope-excluded`
declarations, so 423 IDs from the enum closure contribute to in-scope
unreviewed mechanical coverage. Three non-overlapping OCaml family shards feed
one fail-closed selector. It requires canonical identifiers, supported
enum/typedef signature forms, `unreviewed` enum and typedef classification,
case classification of exactly `unreviewed` or `scope-excluded`, a recorded
unsigned value for every case, exact aggregate cardinality, and collision-free
OCaml names. The generated private
`Metal_raw.Enum_constants` modules preserve each unsigned 64-bit bit pattern as
`int64`; this includes `18446744073709551615` as `0xffffffffffffffffL` rather
than overflowing or narrowing through an OCaml `int`. The manifest records all
448 identifiers, families, decimal values, bit patterns, generated names, every
case classification, and the aggregate 25-case scope-excluded count. Selection
separately validates that all 61 enum and 61 typedef companions are
`unreviewed`.

Generator throughput and safe completion remain separate. Raw/private
generation alone moves no declaration to `bound`; generated pure enums,
fixed-layout records, and copied immutable globals move only after their exact
evidence and conformance gates pass. The current safe/bound completion is 2,527
of 5,249 in-scope declarations, or 48.14%, with 51.86% not yet bound. The safe
percentage changes only when ownership, validation, lifetime, and conformance
evidence appropriate to a declaration moves it to `bound`.

Thirty-three immutable, nonnull NSString SDK globals are generated as a closed
safe family: error domains and user-info keys, common counter/counter-set names,
and device notifications. Each native entry uses its exact SDK typedef in a
guarded C++ `static_assert`, observes the pinned availability introduction,
rejects an unexpected nil, and copies UTF-8 bytes into independently owned
OCaml storage. The public `Metal.Global` module converts native failures to
typed `Metal.error` values. Generated conformance reads each value repeatedly,
checks stable nonempty independent copies, and proves no live/created/released
handle-accounting delta. Exact identifier and signature/header/availability
digests fail closed. Ten inventory entries originating from local variables in
inline SDK constructors are scope-excluded rather than falsely bound.

Public types, domain and lifetime checks, ownership and retention, validation,
capability policy, asynchronous completion, complex records, and multi-object
operations remain handwritten in the safe layer. A generated raw declaration
alone never makes an inventory declaration `bound`: the plan requires an
independent safe-operation/test-evidence record, and the generator parses both
OCaml syntax trees, locates the exact safe module/value and conformance function,
and requires direct applications of the planned raw and public paths before
composing the bound identifier set. The conformance function must also be
reachable through the parsed call graph from an executable top-level
initializer, so a defined but uninvoked test does not qualify. Comments,
strings, prefix names, and unrelated calls do not qualify as evidence.

The direct-void scalar template also supports multiple `NSUInteger` arguments.
It emits the raw OCaml externals and statically typed native adapters, including
the plan's nonnegative-minimum and optional multiple constraints, for these four
selectors:

- `-[MTL4ComputeCommandEncoder setThreadgroupMemoryLength:atIndex:]`
- `-[MTL4ComputeCommandEncoder setImageblockWidth:height:]`
- `-[MTL4RenderCommandEncoder setObjectThreadgroupMemoryLength:atIndex:]`
- `-[MTL4RenderCommandEncoder setThreadgroupMemoryLength:offset:atIndex:]`

The compute threadgroup-memory selector now has a handwritten safe operation
and conformance evidence. The imageblock selector and both render
threadgroup-memory selectors deliberately retain `safe_api = null` and remain
`unreviewed`: imageblocks still need pipeline/device sizing policy, while
persistent render threadgroup memory also needs render-stage reflection and
render-pass allocation validation. Generated raw/native glue alone is not a
safe operation and does not make any of those three inventory identifiers
`bound`.

The `Direct_getter` template covers no-argument scalar getters and validates
the primary selector together with companion method/property closure metadata
for the same Objective-C property. Its first completed slice binds
`-[MTLDevice maxThreadgroupMemoryLength]` and
`property:MTLDevice:maxThreadgroupMemoryLength` as one read-only closure. The
native adapter reads `NSUInteger`, rejects values greater than `INT64_MAX`, and
only then copies the value to OCaml `int64`; overflow is a typed error rather
than signed wraparound. The existing safe `Device.info` record exposes the
result as `max_threadgroup_memory_length`.

The second getter slice binds
`-[MTLComputePipelineState staticThreadgroupMemoryLength]` and
`property:MTLComputePipelineState:staticThreadgroupMemoryLength` as another
read-only closure. `Compute_pipeline.static_threadgroup_memory_length` checks
that the pipeline is live, performs the same checked `NSUInteger`-to-`int64`
conversion, and reports native overflow rather than returning a wrapped value.

`Command4.Compute_encoder.set_threadgroup_memory_length` requires a live
encoder with a bound compute pipeline created with reflection. The index must
be in `[0, 31)` and name an exact reflected `Threadgroup_memory_binding`; the
length is nonnegative and a multiple of 16 bytes. Each encoder retains a
31-slot `int64` length ledger across pipeline changes. A successful
reassignment replaces that index's previous value, including zero as a clearing
assignment, and failed validation or native execution leaves the ledger
unchanged. The current dynamic total plus the pipeline's static
threadgroup-memory length must not exceed
`Device.info.max_threadgroup_memory_length`. Replacement subtracts the old slot
before adding the checked OCaml integer, and the budget comparison uses
`dynamic <= maximum - static`, so adversarial inputs cannot wrap.

Missing reflection on the current pipeline is `Invalid_state`; an out-of-range
index, absent or non-threadgroup reflected binding, invalid length, or exceeded
budget is `Invalid_argument`. Native getter or setter failures remain
`Native_error`. Binding a different pipeline first reads its checked static and
device limits, validates the retained total, and requires each nonzero ledger
slot to be an exact reflected threadgroup-memory binding in that candidate;
missing or incompatible candidate reflection is `Invalid_argument`. The bound
pipeline and cached limits change only after the native bind succeeds, just as
only a successful native setter updates the ledger.

The compute static getter closure and safe compute setter move exactly three
declarations from `unreviewed` to `bound`. The current schema-2 inventory has
3,322 `bound`, one `availability-gated`, 37 `scope-excluded`, and 1,926
`unreviewed`. The compute getter and setter allocate no native handles. The
imageblock and render
threadgroup-memory setters above remain raw-only and do not count as bound.

Exact M1 execution reports 16 bytes of static memory for the small/index-one
fixtures, 32 bytes for the large fixture, and a 32,768-byte device limit. The
small pipeline accepts its exact 32,752-byte dynamic ceiling and rejects 32,768
dynamic bytes; a four-thread dispatch with a 16-byte dynamic allocation changes
`[1; 41; 99; -2]` to `[2; 42; 100; -1]`. Replacements
`16 -> 32,752 -> 16` preserve execution, while `32,752 -> 0` permits switching
to the 32-byte-static pipeline. Failed budget and reflected-index switches
leave the prior pipeline usable, destroyed getters and ended-encoder setters
return `Destroyed`, and the library/compiler can be destroyed before encoding.

Migration is gradual. Each small family first receives deterministic per-entry
golden assertions for generated OCaml, Objective-C++, validation domains,
selectors, symbols, errors, and provenance, then replaces the equivalent
handwritten raw/native glue without changing the public API. The
handwritten and planned identifier sets are composed once and remain subject to
the existing duplicate check throughout that migration. The generator,
orchestration, fixtures, and checks remain OCaml/Dune-native; Python glue is not
part of this architecture.

## Thread and ownership model

Every public native operation requires both OCaml domain zero and the platform
main executor. In an ordinary executable those are the initial OCaml domain and
physical main thread. An embedded XPC service is the narrow exception to the
physical-thread test: `NSXPCListener.serviceListener` hands its main dispatch
executor to `xpc_main`, so the bridge registers only that serialized executor
with OCaml domain zero for the dynamic extent of a request callback. No OCaml
code runs on an NSXPCConnection private queue, and the temporary registration is
removed before returning to XPC. Any other wrong-domain call returns
`Wrong_domain` before entering Metal. The only remaining any-domain native
activity is a custom-block finalizer placing an opaque retained Objective-C
pointer into a fixed 65,536-entry queue; it does not release Objective-C objects
or call back into OCaml. A domain-zero safe entry point drains that queue inside
a coarse autorelease pool.

Objective-C++ stubs compile with ARC. Safe handles are opaque and carry a unique
generation, an atomic destroyed flag, device identity, and parent-dependent
count. Explicit destruction is idempotent. Native access after destruction,
parent teardown with live children, cross-device binding, invalid state, and
bounds or cardinality overflow return typed errors before Objective-C. Blocking
command-buffer completion releases the OCaml runtime lock while its native
handle remains rooted and cannot be concurrently destroyed through the safe
API.

`Metal.Release_queue.stats` exposes queued, dropped, live, total-created,
total-released, placement-mapping-operation, and resident-byte facts for
qualification. The mapping counter proves that rejected ranges did not reach
the Objective-C update methods. Queue overflow is a hard typed error rather
than silent reclamation on a GC domain.

## Implemented vertical slices

The current safe slices cover device enumeration and capabilities; shared,
managed, and private buffers; copied and page-aligned no-copy buffer creation;
CPU range transfer and lexical mapped-range handles; textures, shareable private
textures and opaque shared handles, IOSurface-backed textures, buffer-backed
linear textures, texture buffers, and views; samplers; labels; runtime MSL
library compilation with full `NSError` diagnostics; automatic, placement, and
established sparse heaps; macOS-15 residency sets; function lookup;
compute-pipeline creation and limits; command queues and buffers; compute,
resource-state, and narrow buffer-to-texture blit encoding; checked resource
binding and thread dispatch; Metal 4 placement-sparse buffer and texture
mapping; conventional Metal 4 vertex/fragment render-pipeline compilation;
submission; blocking completion; and command status/errors.

A `Buffer.Mapping.t` is valid only inside `Buffer.with_mapping`. It exposes
checked copy operations rather than a Bigarray backed by an escaping native
pointer. Retaining the opaque value is harmless: every operation returns
`Destroyed` after the callback leaves, buffer teardown is rejected while the
scope is active, and `Fun.protect` closes the scope on exceptions.

`Buffer.create_copy` passes a checked OCaml byte range to
`newBufferWithBytes:length:options:` and verifies that the returned buffer owns
the requested length and resource modes. The source remains rooted for the
synchronous copy only; later source mutation cannot change the Metal buffer.

`Buffer.External` owns an opaque, zero-initialized, page-aligned single VM
region allocated with `mmap`. Lengths must be positive page multiples, and all
copy access is range-checked in OCaml and again at the native boundary.
`Buffer.create_no_copy` is exclusive: a second borrow, owner access, or owner
destruction is rejected while the safe Metal buffer is live, and private
storage is rejected before Objective-C. The safe buffer retains both its
originating device and VM owner, so neither can be destroyed while the borrow
is live. Metal's deallocator block strongly retains the VM owner until the
native buffer actually relinquishes the borrow; this is necessary because the
qualified M1 may defer the callback past safe handle destruction. The VM owner
uses `munmap`, so 10,000 create/borrow/destroy cycles settle without
allocator-retained page growth. Callback pointer/length mismatches are counted
as a hard ownership failure.

`Texture.descriptor` models all current Metal texture kinds, explicit
dimensions, mip/sample/array counts, storage/cache/hazard modes, usage, GPU
optimization intent, lossless/lossy compression intent, typed RGBA channel
swizzles, and all 130 concrete, non-deprecated pixel formats in the pinned SDK:
64 numeric, packed, subsampled, extended-range, depth, stencil, and stencil-plane
formats plus 66 BC, EAC/ETC2, and ASTC block-compressed formats.
The deprecated PVRTC family, `Invalid`, and the Metal 4 unspecialized sentinel
are not presented as concrete resource formats. `Texture.format_layout`
exposes each reviewed format's checked block dimensions and byte size.
`Texture.create` validates positive and bounded dimensions, mip cardinality,
array/kind shape, duplicate usage, multisample structure, view-only formats,
4:2:2 shape, device sample-count support, and the device's explicit
Depth24/Stencil8 capability before entering Objective-C. BC creation is gated
by `supportsBCTextureCompression`; EAC/ETC2 and LDR ASTC require Apple family 2
or Metal 4, HDR ASTC requires Apple family 6 or Metal 4, and compressed volume
textures require the reviewed Apple 3, Mac 2, Metal 3, or Metal 4 feature.
Compressed descriptors reject 1D, multisample, texture-buffer, writable,
atomic, and render-target shapes before native allocation. The bridge then
checks that Metal preserved every observable descriptor property, including
GPU-optimization, compression, and all four swizzle channels. Lossy compression
is Apple-family-8 gated and requires private GPU-optimized storage, a compatible
ordinary/extended-range pixel format, a non-1D/non-buffer kind, and no
pixel-format-view, shader-write, or shader-atomic usage. Structural failures and
unsupported devices allocate no native handle. A native rejection remains a
labeled `Native_error`; it never leaves a partially owned safe handle.
Non-default base swizzles also reject shader-write and shader-atomic usage
before native descriptor validation.

Shared and managed texture transfers accept explicit regions, mip levels,
slices, source offsets, row pitches, and image pitches. Region bounds, format
block alignment, cardinality, OCaml byte-buffer limits, and source coverage are
checked in OCaml and defensively repeated at the C boundary through one generic
block-layout path, including partial final blocks at mip edges, exact two-pixel
blocks for packed 4:2:2 formats, and every BC/EAC/ETC2/ASTC block size. Private
and multisample textures reject CPU transfer. Reads initialize the entire
result so Metal's untouched pitch padding cannot expose native memory. Texture
views preserve kind/slice shape, permit the reviewed equal, linear/sRGB,
extended-range/sRGB, compressed linear/sRGB, and depth-stencil/stencil-plane
pairs, and hold their parent alive until explicit destruction or finalization.
Format reinterpretation requires `Pixel_format_view`; a same-format view whose
only change is channel swizzling does not. Prismel composes a requested view
swizzle with the parent's effective swizzle once, records that effective value
in the returned descriptor, and passes it explicitly to Metal. macOS 26 uses
the checked `MTLTextureViewDescriptor` constructor; the macOS 14 deployment
floor uses the swizzled range constructor. Both paths verify native parent,
relative mip, relative slice, and descriptor metadata.
Swizzled views of shader-write or shader-atomic textures are rejected before
the native constructor.

`Texture.minimum_buffer_alignment` distinguishes ordinary 2D linear textures
from the `Texture_buffer` kind and exposes Metal's per-device, per-format
alignment as a checked positive power of two. `Texture.create_from_buffer`
accepts only those two kinds and ordinary numeric or packed color formats;
subsampled, block-compressed, and lossy-compressed formats remain non-linear
resources. It requires
depth, array length, mip count, and sample count of one; normalizes and matches
the buffer's storage/cache/hazard modes; gates render-target usage on Apple GPU
family 1; and checks offset, aligned row pitch, pixel-row cardinality, 64-bit
overflow, and the complete pitched span against the buffer before native
creation. The bridge repeats the checks and verifies `buffer`, `bufferOffset`,
and `bufferBytesPerRow`. Standalone device/heap creation rejects the
`Texture_buffer` kind so it cannot silently take a semantically different
allocation path.

The returned texture retains its typed buffer parent, shares that buffer's
purgeability/aliasing state, and preserves the buffer ancestor through texture
views. Buffer destroy, purge, and alias transitions are rejected while any such
texture is live. A texture cannot apply those transitions independently and
directs the caller to its backing buffer. Heap and no-copy external-memory
ancestry therefore remains intact through the complete
texture-to-buffer-to-owner chain.

`Texture.create_shared` is a distinct private-storage allocation path and
verifies Metal's `isShareable` result before exposing the texture. A
`Texture.Shared_handle.t` records the originating device identity and native
handle label, retains that device, and remains importable after the source
texture is destroyed. `Texture.import_shared` accepts only a live handle on the
same device and re-verifies the imported descriptor and sharing state. Source,
handle, and every import have independent explicit/finalizer lifetimes: handle
destruction prevents later imports but does not invalidate imports that already
succeeded. The handle is deliberately opaque; NSSecureCoding is not exposed as
an untyped byte archive.

`Texture.Shared_handle.Xpc` instead provides a typed, bounded cross-process
transport for embedded application XPC services. A connection names the
service and caps request/reply payload bytes; each synchronous call supplies a
bounded operation name, one live shared handle, versioned descriptor metadata,
opaque application bytes, and an explicit bounded timeout. A timeout
invalidates the underlying connection. The service accepts only connections
with the same effective user ID, caps concurrent active requests, validates the
operation, payload, metadata version, format, dimensions, resource modes,
usage, compression, channel swizzle, and device registry identity, and exposes
a one-shot request. A handler
must call `reply` or `reject` before returning; a return without either, or an
exception, is rejected automatically. Completion releases the incoming safe
handle and a second completion deterministically returns `Destroyed`.

`Texture.Io_surface.Xpc` uses that same bounded native connection, service,
request, timeout, and one-shot completion core for cross-process IOSurface
ownership. It transports the NSSecureCoding surface object rather than a
process-local pointer or globally discoverable numeric ID. Versioned metadata
records the stable IOSurface identity, allocation size, planar flag, and every
plane's width, height, bytes per element, bytes per row, and exact span. The
receiver bounds the plane count before cardinality arithmetic and requires the
metadata to match the received native surface exactly. Debug labels are
process-local and intentionally are not accepted from the peer. A returned
surface retains the shared kernel allocation independently of the source
process's local handle.

`Xpc.serve` is intended as the terminal entry point of the separately launched
service executable and does not normally return. The Dune conformance targets
construct real `.app/Contents/XPCServices/*.xpc` bundles using the reusable
OCaml `build_xpc_bundle` executable. The shared-texture client writes `37` into
a private shareable texture, the separately launched service imports it and
writes `91`, and the client imports the returned handle and observes `91`.
The IOSurface client sends a two-plane allocation, observes the service's write
through the original handle, and verifies that the returned handle remains
valid after destroying that original. Both replies carry a service PID
distinct from the client. Missing-service, configuration, payload, timeout,
rejection, ownership, and stale-handle paths are exercised without a Python
orchestration layer.

`Texture.Io_surface` is a narrow ownership helper rather than a second general
IOSurface binding. The existing OCaml build helper links IOSurface.framework
directly through Dune. It creates checked, automatically aligned single- or
multi-plane surfaces, records the returned allocation and per-plane row layout,
and exposes only lock-bounded byte copies; no native base address escapes into
OCaml. `Texture.create_from_io_surface` accepts one matching ordinary-color 2D
plane in shared/default-cache storage and rejects plane, dimension, pixel-width,
shape, and mode mismatches before Metal. The bridge repeats those checks and
verifies the returned texture's `iosurface` identity and `iosurfacePlane`.

An IOSurface texture retains both its surface owner and creating device, while
views preserve that typed ancestry. The surface therefore cannot be destroyed
while any base texture is live, and purgeability or aliasing cannot be changed
through a texture that does not own its allocation. IOSurface lock/unlock bounds
CPU access but does not replace Metal command synchronization; callers must
still complete or otherwise synchronize GPU use before concurrent CPU access.

`Heap` exposes device size/alignment queries, automatic and explicit-placement
descriptors, live allocation/usage facts, fragmentation queries, and checked
buffer/texture allocation. Heap storage and cache modes must match each child;
the binding normalizes Metal's documented default heap hazard mode to
`Untracked`. Placement offsets are required only for placement heaps and are
checked for negativity, power-of-two alignment, addition overflow, and heap
bounds before Objective-C. A bounded live-allocation ledger rejects overlapping
placement resources until the older resource is destroyed or explicitly made
aliasable. Although Metal reports placement resources as natively aliasable at
creation, `Buffer.make_aliasable` and `Texture.make_aliasable` are the safe
layer's irreversible storage-relinquishment boundary: the old handle rejects
all later data access and its range may then be reused. Automatic-heap storage
can likewise be reused only after that explicit transition. Direct resources,
texture views, resources with live children/mappings/command dependencies, and
resources on a discardable heap reject the transition before Objective-C.

Buffers, base textures, and heaps expose synchronous purgeable-state query and
transition. A transition returns Metal's prior state and immediately queries
the resulting state; this matters because a device may keep a requested
resource nonvolatile. Data access and command binding reject resources or
ancestor heaps whose observed state is `Volatile` or `Empty`. Restoring an empty
resource to `Nonvolatile` permits reinitialization but does not claim its old
contents survived. Heap transitions reject active lexical mappings and tracked
command uses. Command buffers retain each bound resource exactly once until a
terminal status, explicit destruction, or finalization, preventing destroy,
purge, or alias transitions while the GPU may still use it.

Heap children retain the typed heap owner, buffer-backed textures retain their
typed buffer owner, and views retain their typed texture owner, so teardown
with a live descendant is a deterministic
`Parent_has_dependents` error. Automatic resources intentionally report no
placement offset; placement resources round-trip Metal's actual offset.

`Heap.Sparse` implements the established macOS sparse-texture model on macOS 13
or newer devices that report Apple GPU family 6 support. The macOS 13 floor is
required by the binding's explicit page-size query and heap descriptor rather
than inferred from legacy sparse support alone. Sparse heaps require
private/default-cache storage, an explicit 16, 64, or 256 KiB page size
supported by the device, and a heap size that is an exact multiple of that
page. Legacy sparse heaps reject buffers and placement offsets. The binding
preflights each texture kind/format/sample/page
combination through the device tile-layout query before allocation and verifies
that a heap-created texture reports `isSparse`. This includes compressed
EAC/ETC2 and ASTC resources on Apple family 6 or newer; their reported tile
dimensions must remain aligned to the selected format's blocks. Depth and
stencil formats use the same capability query instead of a blanket exclusion.
The qualified M1 accepts `Depth16Unorm`, `Depth32Float`, `Stencil8`, and
`Depth32Float_Stencil8`; its unsupported `Depth24Unorm_Stencil8` path remains
explicitly capability-gated and allocates no safe handle.

`Device.supports_placement_sparse` separately gates the macOS 26.4 placement
sparse model. `Buffer.create_placement_sparse` and
`Texture.create_placement_sparse` create device-owned, initially unbacked
virtual resources and retain their exact 16, 64, or 256 KiB page choice in the
typed OCaml handle. Texture creation preflights the complete
kind/format/sample/page tile layout before allocation. A `Heap.Placement`
descriptor may use `sparse_page_size` as
`maxCompatiblePlacementSparsePageSize`; its size must be a whole number of
those pages. `Placement_mapping` owns the Metal 4 command queue that connects
those independently created physical and virtual resources.

`Placement_mapping.map_buffer` accepts a virtual tile range and a byte-addressed
heap offset. `map_texture` accepts a tile region, mip level, and slice, including
the one-tile packed-tail representation. Both paths validate device identity,
page compatibility, storage/cache modes, positive cardinality, virtual bounds,
page alignment, multiplication overflow, heap bounds, and virtual and physical
overlap before Objective-C. The physical-range ledger is shared with ordinary
placement-heap allocations, so a sparse mapping cannot silently alias a live
placed resource. Mapping copies, which intentionally introduce two virtual
aliases for the same pages, remain unreviewed until the command-access model can
make their mutually exclusive use explicit.

A successful operation returns an opaque mapping token. The queue and resource
each retain that token; the token in turn retains the virtual resource, heap,
and physical range until synchronous `unmap` succeeds. Resource, heap, and queue
teardown therefore fail with `Parent_has_dependents` while pages are mapped.
Unmap also rejects a resource owned by a live command buffer, and mapped base
textures reject new views, preventing a view from escaping the same dependency
check. The placement heap remains nonvolatile for the mapping lifetime, and
its page range becomes reusable exactly once after unmap. Dropping the caller's
token cannot silently release the mapping because the queue and resource
ledgers continue to own it.

Each native update is followed on the same Metal 4 queue by a consumer barrier
from `MTLStageResourceState` with
`MTL4VisibilityOptionResourceAlias`, then an event signal and synchronous
`MTLSharedEvent` wait. This is the explicit completion boundary before a
classic command queue may use the newly backed resource. The OCaml runtime lock
is released during the native wait while all owners remain rooted. The safe
surface uses Metal 4 only for the mapping boundary; it does not fork the
existing compute, blit, or resource ownership APIs.

`Buffer.sparse_tier` and `Texture.sparse_tier` query the macOS 26 native tier
when the concrete driver implements the selector. The qualified M1/macOS 26.4
driver advertises the selector but raises an Objective-C unrecognized-selector
exception when it is invoked. The bridge catches that platform failure and
reports the tier-1 semantic minimum guaranteed by the typed sparse constructor;
it never lets the exception cross the OCaml boundary. Ordinary resources report
`Not_sparse`, placement page identity propagates through texture views, and
stale tier queries remain deterministic `Destroyed` errors.

`Texture.sparse_info` returns the exact device tile dimensions, page bytes,
first mip in the packed tail, and tail bytes for both sparse models while
retaining typed page metadata and, for legacy sparse textures, heap ancestry.
`Resource_state_encoder.update_texture_mapping` accepts tile—not
pixel—coordinates and checks positive cardinality, mip and slice bounds, tile
bounds, packed-tail addressing, device identity, and whether one request can
fit in the heap before Objective-C. A successful map or unmap retains the
texture and ancestor heap until the command buffer reaches a terminal state.
Metal can still fail a mapping silently when concurrent mappings exhaust the
heap; callers that overcommit need a residency-map policy rather than a false
success guarantee from this wrapper.

The conformance path maps a base tile and mip tail, blits initialized bytes
from a checked staging-buffer range into the mapped tile in the same command
buffer, reads the authored texel through a compute shader, unmaps both regions,
and verifies the same shader observes Metal's defined zero result. The narrow
`Blit_encoder.copy_buffer_to_texture` entry point validates source offset,
row/image pitch, total source span, destination mip/slice/region, format stride,
sample count, and device identity before encoding; both resources remain owned
through completion. Bulk/indirect mapping, access-counter residency maps, and
mapping-copy/move operations remain unreviewed rather than being conflated with
either the qualified legacy command-encoder path or the typed Metal 4 placement
path. The legacy encoder still rejects placement resources before Objective-C.
Direct placement-sparse CPU transfer, resource purgeability, and ordinary
resource aliasability remain explicit errors: physical ownership belongs to
the live placement mapping and its heap, not to the virtual resource handle.

`Residency_set` availability-gates the macOS 15 API at runtime because the
library deployment target remains macOS 14. A set accepts only typed `Buffer`,
`Texture`, and `Heap` allocations from its creating device. Adds reject stale,
discardable, or alias-relinquished allocations; duplicate bulk arguments fail
before Objective-C. Every accepted member retains its safe owner and ancestor
heap-use token. A pending removal therefore continues to block resource
destruction and heap purge until `Residency_set.commit` applies the native
change and releases that ownership.

Membership queries cross-check the native `containsAllocation`,
`allAllocations`, and `allocationCount` surfaces against the safe ledger. On
the qualified Apple M1, `allAllocations` and `containsAllocation` reflect a
pending removal immediately while `allocationCount` can retain the pre-removal
count until `commit`; the safe `allocation_count` reports current membership
from `allAllocations` and accepts only those two explained native counts. The
same device also accepts a descriptor label but returns `nil` from the created
set's read-only `label`; the binding exposes that observed native value rather
than synthesizing a round trip.

Command queues retain every attached residency set until explicit removal or
queue teardown. Command buffers independently retain every set passed through
`use_residency_set(s)` until terminal status, wait, destruction, or
finalization, so removing a set from its queue cannot invalidate in-flight
work. Single and bulk add/remove/use entry points, manual request/end residency,
commits, allocation footprints, and set footprints are all availability
guarded and exception-translated.

`Sampler.descriptor` covers min/mag/mip filtering, anisotropy, all current
address modes and border colors, normalized coordinates, finite float32 LOD
clamps, comparison, LOD averaging, argument-buffer support, and the macOS 26,
Apple-family-10 weighted/minimum/maximum reduction modes plus signed LOD bias.
The latter two fields are runtime- and GPU-family-gated together; non-default
requests return `Unsupported` before allocation on older hardware or systems.
The bridge reads every mutable descriptor
property back before creation and verifies the resulting sampler's device and
label. Invalid anisotropy, non-finite or inverted clamps, out-of-range bias,
illegal unnormalized-coordinate combinations, and malformed labels fail before
sampler creation. Sparse placement resource construction, sparse depth/stencil,
shared-handle XPC transport, and IOSurface XPC transport are covered. Metal 4
placement mapping now covers buffer pages, texture tiles and mip tails, exact
unmap, barriers, and ownership on the qualified hardware. This completes the M3
resource gate; the broader command and pipeline surfaces remain tracked by M4
and M5 rather than being implied by that narrower gate.

The M4 shader and pipeline-asset slices keep runtime MSL compilation explicit and
deterministic (`fastMathEnabled = NO`), preserve the complete `NSError`
description, domain, code, and `userInfo`, and prefix compile,
specialization, and link failures with the caller's label. Libraries,
specialized functions, and compute pipelines expose their observed native
labels. Libraries also expose their Metal kind, install name, and sorted entry
point names. `Function.constants` returns sorted
name/type/index/required metadata,
while `Function.specialize` accepts checked bool, signed and unsigned
8/16/32/64-bit, half, and float values and rejects malformed names,
duplicates, and numeric ranges before native compilation. Compute descriptors
accept same-device, unique visible linked functions, preloaded dynamic
libraries, binary archives, fail-on-archive-miss policy, and optional binding
plus buffer-type reflection. `Binding.validate_layout` is the pure OCaml
boundary used to compare generated metadata with reflected
name/index/access/resource/data-type layouts without depending on Metal's
returned array order. The M1 conformance path executes both a linked,
reflected specialization and every supported scalar constant representation
on the GPU.

`Dynamic_library` compiles a typed dynamic source library with an explicit
install name, creates or loads the corresponding device library, serializes it
to an absolute path, links a client library against unique same-device
dependencies, and preloads those dependencies into compute pipelines. The M1
test serializes and reloads a dynamic library, destroys every source and link
handle after pipeline creation, and then executes the retained pipeline.
`Binary_archive` creates or reloads an archive, adds compute descriptors with
the same linked/preloaded inputs used for pipeline construction, serializes the
archive, and supports strict archive-hit pipeline creation. Duplicate,
cross-device, stale, unsupported, malformed-path, and wrong-function-kind
arguments are rejected at the typed boundary before native handle allocation.

`Pipeline_dataset`, `Pipeline_archive`, and `Compiler` provide the first
synchronous Metal 4 compilation path. Dataset serializers have explicit,
nonempty descriptor or binary capture modes and retain no hidden global state.
A compiler retains its optional same-device dataset until compiler destruction;
pipeline and library results remain independently usable after their compiler,
dataset, archive, and source handles are destroyed. The compiler builds runtime
MSL through `MTL4LibraryDescriptor`, creates checked compute descriptors with
required/max threadgroup cardinality, binary-linking and indirect-command
support, archive-resident binary visible/intersection functions,
dynamic-library preloads, call-stack depth, archive lookup, and pipeline-owned
reflection, and preserves full labeled diagnostics. `Binary_function` retains
the validated descriptor identity and source kind, has an independent native
lifetime, and can be linked into a compute pipeline after its compiler,
serializer, archive, and source function are destroyed. Direct
`Pipeline_archive.load_binary_function` is the strict archive-provenance path;
compiler lookup archives remain a cache-assisted compilation path.

The M1/macOS 26.4 qualification uses separate descriptor-only and binary-only
serializers. Descriptor capture produces a nonempty pipeline script; binary
capture flushes a loadable Metal 4 archive, which a second compiler consumes
through `MTL4CompilerTaskOptions.lookupArchives`. Although Apple's API permits
combining both capture flags, this driver returns `false` without an `NSError`
when the combined serializer exports an archive, so the safe API reports the
native failure and the documented portable workflow keeps those captures
separate. The same driver accepts a compiler descriptor label but may return a
`nil` compiler label; the binding exposes that observed value and still uses
the requested label for creation diagnostics. It also returns `nil` and vertex
metadata from the name and function-type properties of successfully compiled
visible binary functions. Those unreliable readbacks are not exposed as
authoritative metadata: the safe value records the checked descriptor identity
and source kind, while archive reload and actual pipeline linking validate the
native result. Compute compilation also accepts an immutable static-linking
value with checked same-device library/name references, public functions,
private functions, and unique nonempty function groups. Public/group linking
is function-pointer gated; private-only linking remains available without that
capability, matching Metal's export distinction. Descriptor sources are needed
only for synchronous compilation and can be destroyed before the resulting
pipeline executes.

`Compiler.compile_source_async`, compiler dynamic-library create/load,
`create_binary_function_async`, `create_compute_pipeline_async`, and
`create_render_pipeline_async`, `create_mesh_pipeline_async`, and
`create_tile_pipeline_async` return typed `Compiler_task.t` values where
applicable.
Metal worker completion blocks never enter the OCaml runtime or retain OCaml
roots: they store the native result in task-local completion state and enqueue
only a monotonically increasing ID into a process-wide 1,024-entry ring.
`Compiler_task.drain_completions` and `poll` run on the initial domain, and only
`poll` materializes the resulting owned library, dynamic library, binary
function, reflected compute pipeline, conventional render pipeline, or
mesh/object render pipeline. Results are one-shot; duplicate consumption is
`Invalid_state`, task/compiler parent ownership is explicit,
queue overflow is reported, and blocking `wait` releases the OCaml
runtime lock. Async binary-function task creation uses the same descriptor and
archive validation as its synchronous peer, and the task natively retains its
descriptor inputs after their OCaml handles are destroyed. Compute validation
and native descriptor construction are likewise shared by
the synchronous and asynchronous entry points; in particular, absent function
names are rejected before reaching Metal's assertion boundary. The ordinary
async compute selector executes on M1 and preserves requested reflection after
the source library is destroyed. The async dynamic-linking selector is
explicitly gated to Apple9/M3+ because macOS 26.4.1's Apple7/M1 driver faults in
Metal's request serializer even while all descriptor inputs remain retained;
the inventory classifies that selector as availability-gated pending the M3+
lane. Compiler dynamic-library creation and URL loading share same-device,
source-kind, install-name, absolute-path, label, and returned-object checks;
their async blocks retain source/URL inputs through native completion, preserve
full missing-file diagnostics, and produce a library that executes through an
ordinary dynamically linked compute pipeline.

Conventional Metal 4 render compilation is now implemented synchronously and
asynchronously through one checked descriptor path. It accepts a same-device
source library, exact vertex and optional fragment entry-point identities,
supported raster sample count, one to eight typed color formats for rasterized
pipelines, a point/line/triangle input topology, indirect-command support, and
unique lookup archives. Native validation additionally resolves each function
and verifies its shader stage before descriptor construction. Rasterized
pipelines require a fragment function; a non-rasterizing pipeline requires a
void-returning vertex function and no color attachments. Optional reflection
materializes independently owned vertex, fragment, tile, object, and mesh
binding lists from the returned pipeline state. The async task retains its
library and descriptor inputs after the OCaml library handle is destroyed.

Metal 4 mesh compilation uses that same synchronous/asynchronous result and
task machinery with a checked `MTL4MeshRenderPipelineDescriptor`. The safe API
requires an exact mesh entry point and accepts optional object and fragment
stages. Object-only threadgroup, payload, and mesh-grid controls require an
object function; explicit threadgroup dimensions are positive and their
cardinality must match an explicit maximum. Payload memory is capped at Metal's
16-KiB object-to-mesh limit. Rasterization, sample-count, color-format, lookup-
archive, label, same-device, and source-retention rules are shared with
conventional render compilation. Indirect mesh command-buffer support is
rejected below Apple9/M3, while direct mesh pipeline compilation and reflection
require Apple7-or-newer or Mac2 and run on the Apple7/M1 qualification lane.
Reflection preserves distinct object, mesh, and fragment binding lists,
including exact read/write object-payload alignment and size metadata. The
async task remains valid after destruction of its source library.

Metal 4 tile compilation is likewise synchronous or asynchronous and shares
the render-pipeline result type. It is gated to Apple4-or-newer GPUs, accepts
kernel- or fragment-based tile entry points, supports zero to eight typed color
attachments, validates optional exact threadgroup constraints, and exposes the
tile-size-match promise, binary-linking preparation, static linking, lookup
archives, and owned reflection. The M1 conformance path compiles a reflected
buffer-writing tile function, verifies stage isolation, accepts a deliberately
attachmentless pipeline, privately static-links a provider library, and proves
that async descriptor inputs survive source-library destruction.

`Command4` is the initial safe Metal 4 execution namespace. Its allocator owns
at most one recording command buffer at a time; ending that buffer permits the
allocator to record another, while reset requires every buffer that references
its command memory to be destroyed. Command buffers use explicit recording,
ended, submitted, completed, and failed states. A queue accepts one to 64
unique, ended, same-device buffers in caller order and returns a submission
handle. Submission wait releases the OCaml runtime lock and preserves the full
`MTL4CommitFeedback` error. OCaml resource dependencies remain attached until
wait observes completion, and a native submission state independently retains
the queue, command buffers, render targets, and pipelines so premature OCaml
finalization cannot release live GPU inputs.

The first `Command4.Render_encoder` slice records conventional primitive draws
against one to eight unique, base-level, single-sample 2D color attachments.
It exposes checked clear/load and store/don't-care actions, a finite in-bounds
viewport, and point/line/strip/triangle primitive selection. Pipeline binding
requires an ordinary render pipeline whose recorded sample count and ordered
color formats exactly match the pass. The M1 conformance test executes a
full-screen triangle into an 8×8 shared BGRA target and verifies every stored
pixel after commit feedback completes. Invalid labels, attachments, viewports,
draw ranges, state transitions, duplicate submissions, and premature teardown
are rejected before native handle allocation or submission where applicable.

`Command4.Argument_table` owns checked buffer, texture, and sampler binding
slots at Metal's 31/128/16 limits. Buffer bindings use a validated in-buffer
GPU-address offset and optionally a positive dynamic attribute stride when the
table opted into stride storage. Binding replacement and explicit clear update
both native state and OCaml lifetime dependencies. A render encoder can assign
one table to any nonempty unique set of vertex, fragment, tile, object, or mesh
stages. Each draw snapshots the tables currently assigned to its stages and
retains those exact resources through commit feedback, so later table mutation
cannot release already encoded inputs. The offscreen conformance draw obtains
its fragment tint from an ordinary OCaml-created shared buffer at argument-table
slot 1; the pixel-exact green result therefore exercises the binding rather
than a constant-color shader.

`Command4.Render_encoder.draw_indexed_primitives` records direct 16- or 32-bit
indexed draws for a bound conventional render pipeline. The OCaml boundary
requires a positive index count, a nonnegative naturally aligned byte offset,
an exact in-buffer byte span, and a live same-device index buffer; the native
boundary repeats the range and GPU-address overflow checks before encoding.
Each draw retains both its index buffer and the argument-table resource snapshot
through commit feedback. The M1 conformance path executes both formats with
nonzero aligned offsets into separate 8×8 targets and verifies exact blue and
red BGRA pixels after completion.

The instanced render slice uses the base-aware conventional and indexed Metal 4
selectors. Direct draws validate a positive vertex and instance range plus a
nonnegative base instance. Indexed draws add the same checked buffer ownership,
alignment, byte-span, and GPU-address rules, accept a signed base vertex, and
pass Metal the largest aligned accessible range remaining after the selected
offset. The M1 conformance path renders exact left/right color splits for a
nonzero direct vertex start and base instance, then for a 2-byte-offset `uint16`
buffer with base vertex `-2` and base instance `8`.

Indirect conventional and indexed draws consume the standard packed
`MTLDrawPrimitivesIndirectArguments` and
`MTLDrawIndexedPrimitivesIndirectArguments` layouts from checked,
4-byte-aligned 16- and 20-byte buffer ranges. Indexed commands additionally
take an explicit positive, naturally aligned index-buffer range so Metal can
bound GPU-selected `indexStart` and `indexCount` values. Both buffers and every
bound argument-table snapshot remain completion-owned. The M1 conformance path
uses nonzero indirect offsets and an indexed record with `indexStart = 1`, base
vertex `-2`, and base instance `8`, then verifies two exact split-color 8×8
targets.

Metal 4 render passes may additionally carry one checked, base-level,
single-sample 2D depth attachment whose dimensions match the color targets.
Depth clears must be finite and inside `[0, 1]`; load/store policy, render-target
usage, depth-capable format, device ownership, and lifetime are checked at both
the OCaml and native boundaries. `Depth_stencil` owns immutable compare/write
state, and an active comparison or write requires a depth attachment. Binding
snapshots that state through command completion. The M1 conformance path clears
an 8×8 `Depth32_float` target to one, draws a near red triangle before a far
green triangle with `Less` and writes enabled, and verifies that every final
pixel remains exactly red.

Stencil passes use the same checked dimension, sample-count, usage, device, and
lifetime contract with `Stencil8`, packed depth/stencil, or stencil-plane
attachments. `Depth_stencil.face` exposes independent front/back compare,
stencil-failure, depth-failure, pass, and 32-bit read/write-mask policy; the
encoder accepts either one shared reference or distinct front/back references.
Face policy requires a stencil attachment and immutable states remain retained
through completion. The M1 conformance path clears an 8×8 `Stencil8` target,
replaces its value with reference one during a red draw, changes to an `Equal`
test with independent references of two, and verifies that the rejected green
draw leaves every pixel exactly red.

Conventional and mesh render pipelines accept typed color-attachment policies
that combine the pixel format, concrete enabled/disabled blend state, separate
RGB/alpha factors and operations, and a unique per-channel write mask. The
legacy `color_formats` list remains a default-state shorthand and is mutually
exclusive with typed attachments. Synchronous and asynchronous compiler paths
copy the complete policy into the retained native descriptor. Render encoders
also accept a finite float32 blend constant. The M1 conformance path clears an
8×8 target to cyan, blends a red fragment with component-wise blend-color and
one-minus-blend-color factors, suppresses green writes, and verifies every BGRA
pixel is exactly `(64, 255, 64, 255)`. A blend-enabled mesh pipeline separately
executes with the default one/zero equation.

Conventional and mesh pipeline compilation also accepts typed descriptor-wide
alpha-to-coverage, alpha-to-one, maximum vertex-amplification, and logical-to-
physical color-attachment mapping state. Safe defaults preserve Metal's ordinary
behavior: both alpha transformations are disabled, the maximum amplification
count is one, and color-attachment mapping is `Identity`. Amplification counts
must be positive, and both the public device query and pipeline validation use
`MTLDevice.supportsVertexAmplificationCount:` before accepting a requested
count. `Inherited` is represented as an explicit typed mapping choice without
exposing a raw enum.

The native boundary poisons and resets each fresh conventional, mesh, or tile
descriptor, then verifies the observed reset state before applying any request.
On the M1 driver, render reset retains a non-`nil` but cleared vertex descriptor,
and conventional and mesh indirect-command support resets to enabled; the check
preserves those native semantics and still assigns and reads back the final
requested indirect state. It then sets and reads back the exact alpha states,
amplification count, and mapping state on conventional and mesh descriptors;
the synchronous and asynchronous compiler paths share that checked
construction.

The Metal 4 command path keeps amplification and color remapping explicit.
Render-pass creation defaults color-attachment mapping support off and must opt
in before binding an `Inherited` pipeline. `MTL4RenderPassDescriptor` has no
reset method, so the bridge checks a fresh false value, a poison true set/read,
and the separate final descriptor's exact requested value.
`set_color_attachment_map` interprets each list position as a logical output and
its value as the corresponding physical attachment; an explicit map must cover
every attachment exactly once, remain in range, and therefore be a complete
permutation. Supplying no map deterministically installs an explicitly reset
identity map rather than relying on undocumented `nil` semantics. The native
boundary verifies the map object's set/get, reset-to-identity, and copy
semantics before encoding it, and retains the checked object through command
completion.

`set_vertex_amplification_count` requires a bound conventional or mesh pipeline
and accepts only Metal's documented command-time count of one or two. The count
must also fit the compiled pipeline maximum and the device capability. Optional
view mappings either remain absent or contain exactly one entry per amplified
output; both viewport-array and render-target-array offsets must be nonnegative
and fit `uint32`. Rebinding a pipeline whose maximum is below the active count
is rejected. These checks are repeated at the native boundary before passing
the exact `MTLVertexAmplificationViewMapping` records to Metal; no conventional
`MTLRenderCommandEncoder` or `MTLRenderPassDescriptor` path is involved.

The M1 conformance shader uses `amplification_id` with count two and two explicit
zero-offset view mappings to produce one exact 8×8 vertical split: left BGRA
blue `(255, 0, 0, 255)` and right green `(0, 255, 0, 255)`. A second shader
writes logical output zero red and one green. The explicit identity reset sends
them to physical targets zero and one respectively, while permutation `[1; 0]`
produces physical zero green and physical one red for both conventional and
mesh pipelines. Count zero or three, count two against a pipeline maximum of
one, wrong-cardinality, negative, or over-`uint32` explicit view mappings,
missing pass opt-in or pipeline, an `Identity` pipeline, and incomplete,
duplicate, or out-of-range attachment maps are rejected by the typed boundary
without new native handles. Source library and compiler destruction before
recording, pipeline/target destruction rejection while command-owned,
ended-encoder rejection, and successful completion together cover the lifetime
boundary.

Metal 4 raster state is explicit and encoder-local. A live render encoder
accepts typed front-face winding, cull, depth-clip, and triangle-fill modes plus
single or array viewport and scissor state; none of these operations mutates the
immutable pipeline or pass descriptor or installs process-global state. The
array forms contain between one and sixteen entries. Every viewport has finite
values, a nonnegative origin, positive dimensions wholly inside the render
target, and ordered near/far depth values in `[0, 1]`. Every integer-pixel
scissor likewise has a nonnegative origin and positive dimensions wholly inside
the target. Both viewport and scissor validation compare each extent against
the remaining target dimension by subtraction, avoiding an overflowing
endpoint.

Depth bias accepts only finite constant, slope-scale, and clamp components
through the exact IEEE-754 `FLT_MAX` boundary (`0x1.fffffep+127`). Depth-test
bounds are finite values ordered within `[0, 1]`. The disabled `[0, 1]` pair is
valid on the qualified M1 and still reaches the native selector; any active pair
requires both a depth attachment and an Apple10-or-newer GPU, matching Apple's
February 2026 Metal Feature Set Tables. Safe and native validation reject active
bounds as `Unsupported` before that selector on the Apple7/M1. Invalid geometry,
counts, numeric values, missing depth state, or an ended encoder likewise fail
at the typed boundary before the corresponding native setter. This surface is
intentionally limited to
`MTL4RenderCommandEncoder`; the conventional `MTLRenderCommandEncoder`
selectors are not part of the binding.

The M1 raster-state conformance target observes exact BGRA results: a
counter-clockwise front face with back culling and nonzero depth bias is solid
green `(0, 255, 0, 255)`, front culling is transparent `(0, 0, 0, 0)`, and
clockwise winding with front culling is green. Filled-triangle center is green,
line-mode center is transparent with an exact green edge pixel, and a `z = 2`
triangle is transparent under `Depth_clip` but green under `Depth_clamp`. The
scissor target is green only for `x = 2..4, y = 1..4`; two viewport/scissor
entries produce blue `(255, 0, 0, 255)` in the top-left quadrant, green in the
bottom-right, and transparent cross-quadrants. Active depth bounds with or
without an attachment return typed `Unsupported` on Apple7/M1 without new
handles, while `[0, 1]` succeeds; the conditional Apple10 branch retains exact
stored-depth pass/drop assertions. Invalid arrays, geometry, depth, nonfinite or
over-float32 values, and ended-encoder calls allocate no handles. The source
library and compiler are destroyed before encoding, bound pipeline/color/depth
destruction is rejected, and all resources release after completion.

Metal 4 render-pass creation optionally installs one visibility-result buffer
and chooses `Visibility_reset` or `Visibility_accumulate` behavior. A supplied
buffer must be live, belong to the command buffer's device, and contain at least
one eight-byte result slot. The default reset behavior remains valid without a
buffer, but accumulate requires a configured buffer. The native descriptor
boundary assigns and reads back the exact buffer identity and result-type enum
before encoder creation, and the command buffer retains that buffer through
submission completion.

`set_visibility_result_mode` selects disabled, boolean, or counting queries for
subsequent draws. Every offset must be nonnegative and eight-byte aligned. An
active boolean or counting query additionally requires the pass buffer and a
complete in-range eight-byte slot; disabled mode needs neither a buffer nor a
range-covered slot. An attachment may be created with `Store_deferred`, which
maps narrowly to `MTLStoreActionUnknown` and promises a later dynamic decision.
The color, depth, and stencil setters accept only `Store_dont_care` or `Store`,
and only once for the corresponding unresolved deferred attachment. A color
action requires the named color-attachment index, while depth and stencil
actions require their corresponding pass attachments; already-final attachment
state and a second finalization are rejected. Ending an encoder with any
deferred action still unresolved is likewise rejected. All setters remain local
to a live `MTL4RenderCommandEncoder` and reject invalid state before the native
selector; no conventional `MTLRenderCommandEncoder` or
`MTLRenderPassDescriptor` surface is bound.

The M1 conformance shader writes one full 8×8 target. Its boolean visibility
slot is `1` (the portable assertion remains the SDK-defined nonzero result), its
counting slot is exactly `64`, disabling the query preserves a prior `64`, and a
reset pass followed by an accumulate pass produces exactly `128`. A deferred
color action finalized to `Store` yields solid BGRA green `(0, 255, 0, 255)`.
Deferred depth finalized to `Store` preserves the red seed pass's depth so a
later loaded far/`Less` green draw leaves a transparent target; deferred stencil
likewise preserves reference `7`, allowing a later loaded `Equal` draw to yield
solid green. The visibility render target is also exactly green.

Undersized and destroyed buffers, plus a foreign-device buffer when available;
accumulation or active queries without a buffer; invalid offsets or attachment
indices; missing depth/stencil attachments; deferred setter arguments; repeated
finalization; and partially unresolved encoder ends all reject without
increasing the native handle count. The source library and compiler are
destroyed before encoding. The result buffer remains command-owned while the
encoder is active, after it ends, and before submission wait, then releases
successfully after completion.

The M1 capability scan accepts amplification counts through eight and first
rejects nine; synchronous and asynchronous conventional and mesh compilation
reject zero or nine before native handle allocation. Separate four-sample
compile-only pipelines verify `alpha_to_coverage = true`,
`alpha_to_one = false`, and `Inherited` mapping independently of command
execution. For executed single-sample `Bgra8_unorm` pipelines, the
conventional and mesh fragments output RGBA `(0.25, 0.5, 0.75, 0.25)` with
blending disabled. Controls preserve exact BGRA `(191, 128, 64, 64)`, while
asynchronously compiled alpha-to-one pipelines force exact BGRA
`(191, 128, 64, 255)` after their source library is destroyed.

`Vertex_descriptor` represents all 53 concrete macOS vertex formats, constant/
per-vertex/per-instance/per-patch step policies, and static or command-time
dynamic strides as canonical immutable OCaml data. Construction rejects empty,
duplicate, out-of-range, unmatched, unused, or static-stride-overrunning
attribute/layout combinations. Conventional synchronous and asynchronous
render-pipeline compilation copies an optional descriptor into Metal and reads
every format, offset, buffer index, stride, step function, and step rate back
before compilation proceeds. A draw using such a pipeline requires a vertex
argument table with every layout buffer bound; dynamic layouts additionally
require the table's checked dynamic stride. The M1 conformance path fetches
interleaved `Float2` positions and normalized `Uchar4` colors, then verifies a
static-stride direct draw as exact BGRA `(51, 34, 17, 255)` and a dynamic-stride
indexed draw as exact BGRA `(47, 101, 203, 255)` across separate 8×8 targets.

All three Metal 4 render-pipeline families accept immutable, stage-local static
and dynamic linking policy. Conventional pipelines expose independent vertex
and fragment policy, mesh pipelines expose object, mesh, and fragment policy,
and tile pipelines expose tile policy. A static and a dynamic descriptor may be
used independently or together at the same stage. Static policy uses the shared
checked public, private, and grouped-function representation; each supplied
`Compiler.stage_linking` dynamic value creates a stage descriptor with a
positive maximum call-stack depth, checked binary functions, and checked
preloaded dynamic libraries.

Linking policy for an absent optional object or fragment stage is rejected
before native allocation. Public/group static functions and render-stage binary
functions require `MTLDevice.supportsFunctionPointersFromRender`; the safe
layer does not substitute the compute-oriented `supportsFunctionPointers`
capability. Object- and mesh-stage binary functions additionally require
Apple9/M3. A binary function also requires the corresponding per-stage support
flag. Private-only static linking does not add that requirement. Every input
handle must be live, same-device, and unique within the relevant category,
while dynamic-library preloads additionally require dynamic-library support and
unique nonempty install names. The native boundary copies each static
descriptor to its exact pipeline-stage property, sets and reads back every
support flag, fills only the requested children of
`MTL4RenderPipelineDynamicLinkingDescriptor`, and verifies the copied static
function/name/group structure plus dynamic depth and exact binary-function and
preloaded-library identities.

Synchronous static/dynamic compilation and ordinary or static-only asynchronous
compilation execute on the Apple7/M1 qualification lane. Any asynchronous
render, mesh, or tile request containing dynamic linking is rejected without a
native allocation before Apple9/M3 because the older driver cannot safely
serialize it. M1 conformance combines static and dynamic stage inputs, then
destroys the source `Function`, `Binary_function`, `Dynamic_library`, source,
provider, and client `Library` values, completed `Compiler_task` values, and
`Compiler` before using the resulting pipelines. Conventional and direct-mesh
pipelines produce exact 8×8 BGRA green `(0, 255, 0, 255)` targets, the
object-to-mesh pipeline produces exact 8×8 BGRA red `(0, 0, 255, 255)`, and the
tile pipeline writes the exact shared-buffer `int32` value `23`. The M1 async
negative cases prove typed rejection without allocation; positive asynchronous
dynamic linking requires the Apple9/M3 lane.

`Command4.Compute_encoder` binds a checked compute pipeline and an optional
argument table, then dispatches positive direct grid/threadgroup dimensions
whose threadgroup cardinality fits the compiled pipeline limit. Each dispatch
snapshots the table's current resources into both the OCaml dependency graph
and native completion-owned state. The M1 conformance kernel increments four
shared-buffer values through argument-table slot 0, clears that slot immediately
after encoding, proves the buffer remains protected until feedback, and verifies
the exact four-value result. Missing pipelines, invalid dimensions, overflow,
oversized threadgroups, open-encoder teardown, and premature pipeline/table/
buffer destruction are rejected deterministically.

`Command4.Render_encoder.draw_mesh_threadgroups` executes both direct mesh and
object-to-mesh pipelines. Pipeline binding queries the compiled state's object
and mesh thread maxima, execution widths, and object-stage mesh-grid maximum;
the OCaml value also retains its exact required threadgroup dimensions and
compiler optimization promises. A draw requires positive, non-overflowing grid
and mesh dimensions, requires an object threadgroup exactly when the pipeline
has an object stage, checks every explicit required size and maximum, and
enforces execution-width multiples when the compiler was promised them. The
same per-draw argument-table snapshot used by conventional rendering retains
object, mesh, and fragment inputs after the table is cleared or rebound. The M1
conformance path executes a full-screen direct mesh pipeline into an 8×8 green
target and an object-payload mesh pipeline into a separate 8×8 red target, then
verifies every BGRA pixel after commit feedback.

`Command4.Render_encoder.dispatch_threads_per_tile` executes tile pipelines and
snapshots the tile-stage argument table through the same completion-owned
resource graph. Encoder creation captures Metal's positive native tile width and
height, which `tile_size` exposes without another native call. Dispatch requires
positive width and height, depth one, dimensions inside that native tile,
cardinality within the compiled pipeline maximum, the exact required size when
configured, and a full-tile match when compilation promised one. Pipeline
binding verifies Metal preserved that promise and records its observed maximum.
The M1 conformance path compiles an exact 32×32 full-tile pipeline, clears its
argument-table slot immediately after dispatch, and verifies the retained shared
buffer contains the tile shader's exact value `23` after commit feedback.

`test_metal.exe` runs a real M1 compute kernel, wrong-domain and invalid-state
cases, full labeled shader diagnostics, function-constant introspection and
specialization, linked visible functions, reflected binding-layout validation,
dynamic-library source/client linking and serialize/reload retention, binary
archive population and serialize/reload strict-hit creation, compiled-library
metadata, Metal 4 compiler library and
reflected compute creation, descriptor and binary dataset capture, serialized
Metal 4 archive reload/strict binary lookup, binary-function compilation and
dynamic pipeline linking, public/private/grouped static-link descriptors,
asynchronous library success/error, dynamic-library source/file success/error,
binary-function, and reflected compute completion with bounded ID draining and
M1 dynamic-link rejection, synchronous reflected and non-rasterizing Metal 4
render compilation, asynchronous reflected render completion after source
library destruction, synchronous reflected mesh and object/mesh compilation,
exact object-payload reflection, non-rasterizing mesh compilation, invalid
stage/threadgroup/payload rejection without handle allocation, and asynchronous
reflected mesh completion after source-library destruction, reflected and
attachmentless Metal 4 tile compilation, private tile static linking, invalid
tile stage/threadgroup rejection, asynchronous tile completion after
source-library destruction, allocator/queue/buffer/submission lifecycle,
argument-table buffer/texture/sampler ownership and render-stage snapshots, plus
pixel-exact conventional Metal 4 offscreen render execution and exact
16-/32-bit indexed execution with two nonzero-offset pixel-exact targets,
base-aware direct/indexed instancing with two exact split-color targets,
direct/indexed indirect execution from checked packed OCaml buffers,
immutable depth compare/write state and a pixel-exact depth-ordered target,
front/back stencil policy and a pixel-exact reference-rejected target,
typed render/mesh blend policy and a pixel-exact constant-blended target,
reset-verified render/mesh/tile descriptor state, amplification capability and
no-allocation rejection, four-sample alpha/mapping compilation, and exact
render/mesh alpha-to-one control and enabled targets,
command-time two-output amplification with an exact split target, deterministic
identity reset and conventional/mesh attachment swaps, typed rejections, and
completion-owned pipeline, target, and native-map lifetimes,
encoder-local raster, viewport, and scissor state with exact cull/fill/clip and
array targets, Apple10-gated depth bounds, no-allocation validation, and
completion-owned pipeline and attachment lifetimes,
one-shot deferred color/depth/stencil store finalization with exact persisted
targets, exact reset/accumulate/disabled visibility slots, typed no-allocation
rejections, and completion-owned result-buffer lifetime,
typed static/dynamic vertex layouts with exact direct/indexed targets,
typed static/dynamic vertex, fragment, object, mesh, and tile linking with
destroyed source handles, exact green conventional/direct-mesh targets, an
exact red object-to-mesh target, and exact tile value `23`,
argument-table-backed Metal 4 compute execution, direct and object-stage mesh
command execution with two pixel-exact offscreen targets, and exact full-tile
command execution through a completion-retained output buffer,
compiler/task/dataset parent ownership, and complete Metal 4 compile diagnostics,
copied/no-copy external buffer ownership,
shareable texture/handle/import lifetimes, single- and multi-plane IOSurface
ownership and byte visibility, buffer-backed 2D/texture-buffer creation across
shared/managed/private storage,
configured cache/hazard modes, and shared transfer, buffer and texture bounds,
stride, and cardinality checks, texture mip transfer, effective channel-swizzle
composition with GPU sampling, lossy-compression policy/capability gating,
views, sampler validation, multisample capability gating, heap alignment and
placement,
aliasing, purgeability, residency
membership/commit/queue/command retention, the complete 130-format matrix,
generic format-block transfers, encoded-block CPU round trips for all 66
capability-supported compressed formats, all 42 bidirectional compressed
linear/sRGB views, compressed private blits and volume creation, Depth24 and BC
capability gating, sparse page and tile capability queries, compressed sparse-tile
alignment, depth/stencil creation, legacy map/blit/read/unmap behavior,
placement-sparse capability, all page-size constructors, typed tiers, texture
views, sparse-compatible placement heaps, unmapped-state rejection, Metal 4
buffer/texture/tail map and unmap, mapped GPU write/read visibility, physical
range reuse, command-retained unmap rejection, wrong-domain and stale-queue
handling, and proof that invalid or overlapping mappings do not enter the
Objective-C update methods,
all sampler reduction modes and LOD-bias validation,
command-resource retention, parent ownership, idempotent destruction, stale
access, and GC-finalizer release. The
separate XPC conformance apps exercise real cross-process shared-texture and
two-plane IOSurface mutation, typed metadata, and transport failures. The
separate ownership stress performs
warm-up followed
by 100,000 measured buffer create/destroy cycles, 100,000 measured
texture/sampler create/destroy cycles, and 50,000 Metal 4 argument-table/buffer
bind/destroy cycles covering 100,000 measured handles. Ten thousand
heap/purge/alias/replacement
cycles covering 30,000 measured heap/child-resource handles, and 10,000
residency add/commit/remove/commit cycles covering 20,000 measured
set/resource handles. Ten thousand sparse heap/color-texture cycles and a
separate 10,000 sparse heap/depth-texture cycles each cover 20,000 measured
handles on the qualified M1. Ten thousand placement-sparse
heap/buffer/texture/queue cycles cover another 40,000 measured handles and
40,000 exact native map/unmap operations. Another
10,000 base/swizzled-view cycles cover 20,000 handles. Another
10,000 buffer/linear-texture ownership cycles cover 20,000 handles; 10,000
shareable-source/handle/import cycles cover 30,000 handles; 10,000
IOSurface/texture cycles cover 20,000 handles; and 10,000 external-memory/no-copy
cycles cover 20,000 handles and exact
deferred-deallocator layout. Each lane requires exact
created/released balance, zero pending or dropped releases, stable live-handle
count, and bounded settled RSS growth. The stress executable launches each lane
in a fresh OCaml worker process. This keeps the RSS baseline and Metal resource
budget local to the resource kind under test; the Leaks qualification wraps
each worker directly rather than inspecting only the coordinator process.

`PRISMEL_METAL_SANITIZERS` is parsed by OCaml build configuration and accepts
`address`, `undefined`, or `thread`; ThreadSanitizer is deliberately exclusive,
while AddressSanitizer and UndefinedBehaviorSanitizer may be combined. The same
configuration instruments the ARC bridge, its dynamic bytecode stub, and the
final executable link. `tools/metal/check_memory.exe` rejects sanitizer reports,
nonzero Leaks summaries, and Guard Malloc errors for the conformance and
resource-stress tests. Guard Malloc runs the conformance subset because giving
every stress allocation its own protected VM region would test the tool's
intentional memory amplification rather than Metal lifetime settling.
Before launching a sanitizer lane, the checker inspects each executable with
`otool -L` and rejects missing selected Clang runtime dylibs. Apple clang links
combined AddressSanitizer/UndefinedBehaviorSanitizer programs through the ASan
dynamic runtime, whose Darwin implementation supplies the UBSan handlers, so
the `address-undefined` preflight also requires imported `ubsan_handle_` symbols
from `nm -u`. That mode installs both runtime option families and rejects either
diagnostic family. This prevents an ordinary or partly instrumented executable
from producing a false sanitizer pass.

AddressSanitizer qualification disables its allocation quarantine and uses a
256 MiB per-worker RSS ceiling for the ownership stress. ThreadSanitizer uses a
384 MiB per-worker ceiling, including for the `mmap`/`munmap` external-memory
lane. Those sanitizer ceilings cover measured allocator, shadow-memory, and
Metal-driver metadata high-water marks; they are not the production memory
budget. The ordinary unsanitized workers keep the default 8 MiB limit. Every
mode retains exact handle balance, zero pending or dropped releases, zero
deallocator-layout mismatches, and sanitizer diagnostics.
The separately launched XPC conformance bundles are also built and run under
combined AddressSanitizer/UndefinedBehaviorSanitizer and under ThreadSanitizer;
each 64-call reuse loop requires exactly 64 created and 64 released client
handles with no live-count drift.

`tools/bench_metal_ffi.exe` is the release-profile M9 baseline. It measures one
Objective-C property query per OCaml call, one batched call containing the same
number of queries, the equivalent loop timed inside Objective-C++, and the
current ownership-aware `Device.info` path. Every sample records wall time,
FFI-call count, minor/major/promoted allocation, collection count, and an exact
checksum. The executable rejects a batched median more than 5% slower than the
native loop. The live timing check runs through the explicit `@metal-bench`
alias, outside Dune's parallel functional/stress suite; ordinary `@runtest`
still validates the frozen machine-readable M9 evidence. This keeps the 5%
comparison under controlled conditions instead of measuring contention from
unrelated tests. The result makes the later descriptor/command batching
decision measurable instead of applying `[@@noalloc]` or per-item calls
speculatively.

The native runtime compiles MSL source directly through the public Metal API.
Prismel has no offline `.air`/`.metallib` artifact pipeline and no dependency on
the Xcode command-line shader tools.

Run the current binding checks with:

```sh
opam exec -- dune runtest lib/metal --force
opam exec -- dune build @all @doc
opam exec -- dune build @metal-bench

PRISMEL_METAL_SANITIZERS=address,undefined opam exec -- dune build \
  --build-dir /tmp/prismel-metal-asan \
  lib/metal/test_metal.exe lib/metal/test_metal_stress.exe
opam exec -- dune exec tools/metal/check_memory.exe -- \
  --mode address-undefined --artifacts /tmp/prismel-metal-asan/default
PRISMEL_METAL_SANITIZERS=thread opam exec -- dune build \
  --build-dir /tmp/prismel-metal-tsan \
  lib/metal/test_metal.exe lib/metal/test_metal_stress.exe
opam exec -- dune exec tools/metal/check_memory.exe -- \
  --mode thread --artifacts /tmp/prismel-metal-tsan/default
PRISMEL_METAL_SANITIZERS=address,undefined opam exec -- dune build \
  --build-dir /tmp/prismel-metal-xpc-asan \
  lib/metal/metal_xpc_conformance.app \
  lib/metal/metal_io_surface_xpc_conformance.app
ASAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
  /tmp/prismel-metal-xpc-asan/default/lib/metal/metal_xpc_conformance.app/Contents/MacOS/test_metal_xpc_client
ASAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
UBSAN_OPTIONS=halt_on_error=1:print_stacktrace=1 \
  /tmp/prismel-metal-xpc-asan/default/lib/metal/metal_io_surface_xpc_conformance.app/Contents/MacOS/test_metal_io_surface_xpc_client
PRISMEL_METAL_SANITIZERS=thread opam exec -- dune build \
  --build-dir /tmp/prismel-metal-xpc-tsan \
  lib/metal/metal_xpc_conformance.app \
  lib/metal/metal_io_surface_xpc_conformance.app
TSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
  /tmp/prismel-metal-xpc-tsan/default/lib/metal/metal_xpc_conformance.app/Contents/MacOS/test_metal_xpc_client
TSAN_OPTIONS=abort_on_error=1:halt_on_error=1 \
  /tmp/prismel-metal-xpc-tsan/default/lib/metal/metal_io_surface_xpc_conformance.app/Contents/MacOS/test_metal_io_surface_xpc_client
opam exec -- dune exec --profile release tools/bench_metal_ffi.exe -- \
  --iterations 1000000 --samples 7 --profile release
```
