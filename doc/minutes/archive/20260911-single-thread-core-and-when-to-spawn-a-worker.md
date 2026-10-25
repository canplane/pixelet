# A single-threaded core, and when to spawn a worker

**2026-09-11** — after the cooperative scheduler reached its first milestone
(one tick, one derived budget, every chunk step small by construction, one
priority order), the question of the parked fifth item: should work move to
workers, and what does that require of the memory model. The answer is a design
rule and a set of prerequisites, not an implementation.

## The question, and why it was open

The architecture before April was preemptive: several WASM instances with a
central IPC switch between them. It was set down on noticing that Figma does not
work that way, and the codebase has moved since toward one core on one thread
with cooperative scheduling. That leaves workers as the one place preemption
could come back in, and the reasons to be wary of them are specific:
SharedArrayBuffer, and the COOP/COEP headers it requires, are to be avoided; and
everything except the UI is meant to end up inside WASM.

## What Figma does, checked rather than assumed

**Confirmed, from Figma's own writing:**

- The renderer is C++ compiled to WASM with Emscripten, "in order to use it in
  the main Figma application". The 2025 WebGPU migration post says nothing about
  workers or OffscreenCanvas.
  ([Figma rendering: powered by WebGPU](https://www.figma.com/blog/figma-rendering-powered-by-webgpu/))
- Plugins run in QuickJS compiled to WASM, in a sandbox on the **main thread**,
  chosen for performance; only the plugin UI is split out, into an iframe.
  ([How we built the Figma plugin system](https://www.figma.com/blog/how-we-built-the-figma-plugin-system/),
  [How plugins run](https://developers.figma.com/docs/plugins/how-plugins-run))
- The document lives in one reservation: "All C++ objects are just reserved
  ranges in a pre-allocated typed array so the JavaScript GC is never involved."
  Large allocations leave that heap through an `IndirectBuffer` handing C++ an
  external typed array, to hold fragmentation down over a long session. Evan
  Wallace, who wrote this, also wrote `evanw/buddy-malloc` — the allocator
  `alloc/buddy_tree.rs` is a port of, which is why the prior art lines up as
  closely as it does.
  ([Building a professional design tool on the web](https://www.figma.com/blog/building-a-professional-design-tool-on-the-web/))
- Code Layers runs "most of the development toolchain in a Web Worker" — esbuild
  for bundling, Tailwind v4 with Lightning CSS for styles, partly native code
  compiled to WASM.
  ([Building Figma's Code Layers](https://www.figma.com/blog/building-figmas-code-layers/))

**Not confirmed:** a claim that Figma moves audio and video frames between an
AudioWorklet and workers over SharedArrayBuffer appears in secondary sources
only. Against it: `https://www.figma.com/` and `https://www.figma.com/design/`
were fetched on this date and carry neither `Cross-Origin-Opener-Policy` nor
`Cross-Origin-Embedder-Policy`, so SharedArrayBuffer is unavailable on those
pages. A signed-in file page was not checked, so this is strong evidence rather
than proof.

**The pattern:** the document model and the renderer — the core — are WASM on
the main thread. Workers are used for a detachable, batch-shaped subsystem
(compiling code) whose results are not needed within a frame.

A former Figma engineer's notes add the memory-side cautions that matter here
([Notes From Figma II](https://andrewkchan.dev/posts/figma2.html)):
SharedArrayBuffer was disabled across browsers from 2018 to 2020, so message
passing was for years the only way workers could communicate; WASM memory is
grow-only, so a burst grows the footprint for good; growing at runtime is
unreliable enough on mobile to crash, so the practice is to reserve what the app
will need up front; and wasm32 addresses 4 GB.

## Why one thread is not a ceiling

SharedArrayBuffer exists to share memory **between threads**. On one thread, JS
and WASM already share a `WebAssembly.Memory` with no SharedArrayBuffer, no
copies, and no atomics. Cooperative scheduling then stands in for preemption:

|                        | threads + SharedArrayBuffer | one thread, shared memory, cooperative          |
| ---------------------- | --------------------------- | ----------------------------------------------- |
| zero-copy sharing      | yes                         | yes                                             |
| locks / atomics        | needed                      | not needed                                      |
| COOP / COEP            | required                    | not required                                    |
| multi-core parallelism | yes                         | no                                              |
| a long step            | other threads keep running  | the frame slips — steps must be small by design |

The one thing given up is parallelism, and its price is the last row. The
scheduler work of 2026-09-10 and -11 — the POI index swept a page per step, the
chunk scan and bake stepped a few rows at a time — is that price being paid by
construction rather than hoped away.

## What a worker is worth

A dedicated worker is a real OS thread in the renderer process, and OS
schedulers divide CPU by thread. So a worker buys roughly one more core **if one
is idle**, and nothing past `navigator.hardwareConcurrency` except time-slicing
and context switches.

Fewer cores are idle than it looks: the renderer already runs a compositor
thread, raster worker threads and V8's parallel and concurrent GC helpers, and
the GPU is a process of its own — the `offthread` share of the ledger's worst
frames is time spent over there. On mobile, extra threads can lower total
throughput: big.LITTLE placement follows QoS, and heat lowers clocks. Each
worker also carries its own V8 isolate, a few megabytes before it does anything,
and — if it runs WASM — its own reserved memory, which the mobile caution above
multiplies by the worker count.

A worker is therefore a way to use a core already there, not a way to be given
more.

## What transfer can and cannot move

Checked directly under Deno, which follows the same structured-clone rules:

```
plain ArrayBuffer transfer:          ok       sender's byteLength becomes 0
WebAssembly.Memory buffer transfer:  THROWS   "item in transferredArrayBuffers is not transferable"
slice of a Memory (a copy):          ok
a view after memory.grow():          length 0 — detached
```

Three consequences, and they are the prerequisites:

1. **The big memory cannot lend a window of itself to a worker.** A
   `WebAssembly.Memory` is not transferable, and there is no way to hand over
   part of one. Anything bound for a worker lives in a plain `ArrayBuffer`
   outside it, and data that starts inside it crosses with one copy.
2. **So memory is heterogeneous: one big, N little.** The big one is the core's:
   JS and WASM on the main thread, reserved generously up front, addressed by
   handles (offset and length) rather than long-lived views, because a view dies
   on `grow`. The little ones are transferable staging buffers, pooled, each
   either at home or away.
3. **And something has to know which are away.** That is a central scheduler,
   and it is half built. Synchronous CPU work already runs in the tick. Worker
   jobs are asynchronous, quota-limited and cancellable — the same shape as a
   fetch — so they belong in something shaped like fetch_scheduler.ts (quota,
   wishlist replacement, abort) plus a ledger of ownership: which buffers are
   out, for which job, and what happens to them when that job's chunk is
   disposed before it returns.

## The rules

- **One core, one thread, one memory.** JS and WASM share it and take turns
  under the tick.
- **A worker only for work that is pure, transferable and not needed within the
  frame.** Moved by ownership, never shared. No SharedArrayBuffer, no COOP/COEP.
- **Spawned once, when first needed, and kept.** Spawning costs tens of
  milliseconds and an isolate's worth of memory, so never per task.
- **How many is decided by how many kinds of detachable work there are, not by
  the core count.**

## The first candidate, and when

The chunk bake meets all three conditions. It is pure: a run set in, a mesh out,
which stepping it made explicit. Both are typed arrays over pooled blocks. And
its result is not needed within the frame — a chunk already completes over
several frames while its previous mesh stays on screen.

It is also the lucky case for copies. The mesh goes to the GPU, and a GPU upload
reads from any `ArrayBuffer`, so a mesh baked into a little buffer can reach the
GPU without ever entering the big memory. The only crossing is the run set going
out, which is hundreds of kilobytes.

It is not needed now. At the time of writing the ledger reads a scheduler p95 of
about 2 ms and 1% of frames over budget. The signal to move the bake is the
visible phase spending its whole share frame after frame while the pending count
does not fall — the point where one core has stopped being enough, read off the
ledger rather than guessed.

This workload may reach that point sooner than Figma's does. Figma's document is
bounded and loaded, and most frames redraw what changed; pixelet streams an
unbounded world and meshes it, and meshing is independent per chunk — the most
parallel-friendly shape there is.

## Against January to April

This is not the earlier architecture returning. That one put preemption and an
IPC switch at the center, with every WASM instance holding its own memory, so
everything between instances was a copy or a message. Here the center is one
address space on one thread, and workers are peripheral, detachable, and fed by
ownership transfer — closer to a large process with a few helpers than to a
microkernel. What comes back from April is the part that was always sound: the
CommandPort contract for mutation, and a wire format (XTP) for boundaries where
memory is **not** shared — the network, the disk, a worker. Where memory is
shared, commands carry handles and nothing is serialized.

## Keeping views alive across growth — measured, and one answer rejected

Everything the core holds is typed-array views into pooled blocks: a
`ColumnRunSet`'s thirteen lanes, a `BakedChunkMesh`'s eight spans, the LOD
`visibility` span — and three.js holds the mesh spans for the life of a geometry
as `BufferAttribute.array`. A `WebAssembly.Memory` detaches its buffer on
`grow`, and every one of those views dies with it. Three ways out were on the
table: re-point every view after a grow, drop CPU copies after upload, or never
grow. A fourth turned up: `memory.toResizableBuffer()`, whose views survive
`grow`. It was checked first, because it would have made the problem disappear.

Measured under Deno 2.7.11, V8 14.7:

```
reserve a 512 MB Memory:            RSS 58 -> 58 MB untouched; +64 MB after touching 64 MB
plain Memory, fixed view, grow:     length 0 — detached
toResizableBuffer, fixed view, grow: length 16, value kept, same buffer object
```

It works — and it is the wrong answer here, for two independent reasons.

**It makes the hot loops two and a half times slower.** A loop shaped like the
mesher's lane reads, four million elements, median of eleven, run in both
orders:

```
plain ArrayBuffer, fixed views           7.8 - 8.8 ms
wasm Memory (fixed length), fixed views  7.9 ms
wasm Memory via toResizableBuffer       19.1 - 19.7 ms
  ... with a length-tracking view       17.1 ms
ES2024 resizable ArrayBuffer            19.1 - 21.0 ms
resizable, copied to plain first         8.9 - 9.0 ms
```

The penalty belongs to resizable buffers in general in this engine, not to WASM.
The mesher and the scan make about a hundred lane accesses between them and are
the center of the frame budget; this is not a cost to take.

**And the GPU will not accept them.** WebIDL's buffer-view conversion reads: "If
the conversion is not to an IDL type associated with the [AllowResizable]
extended attribute, and IsFixedLengthArrayBuffer(V.[[ViewedArrayBuffer]]) is
false, then throw a TypeError." ([WebIDL](https://webidl.spec.whatwg.org/))
WebGL's `bufferData`/`bufferSubData` take `AllowSharedBufferSource` and
`[AllowShared] ArrayBufferView`, with no `[AllowResizable]`
([webgl.idl](https://registry.khronos.org/webgl/specs/latest/1.0/webgl.idl),
[webgl2.idl](https://registry.khronos.org/webgl/specs/latest/2.0/webgl2.idl));
WebGPU's `writeBuffer` and `writeTexture` the same, and the WebGPU specification
does not contain the word `AllowResizable` at all
([WebGPU](https://gpuweb.github.io/gpuweb/)).

For the record, support is Chrome and Edge 144, Firefox 145, Safari 26.2
including iOS, Chrome for Android 152 — 82.6% of global usage
([caniuse](https://caniuse.com/mdn-webassembly_api_memory_toresizablebuffer),
[WebKit, Safari 26.2](https://webkit.org/blog/17640/webkit-features-for-safari-26-2/)).
Availability was never the obstacle.

**So: a fixed-length memory, reserved generously, never grown.** Reservation is
lazily committed on desktop, as measured above. Fixed-length views are as fast
as views into a plain buffer, the GPU accepts them, and since the buffer never
detaches, the existing view-holding code stays correct unchanged.

Which turns the handle migration proposed earlier in this document from a
prerequisite into an option. It was going to touch about a hundred and fifteen
access sites to survive `grow`; with no `grow`, nothing needs to survive it. The
concrete step is smaller: the buddy allocator carves its regions out of the
reserved memory instead of calling `new ArrayBuffer` per region, and falls back
to plain buffers — its existing spill path — once the reservation is exhausted.

The price, recorded so it is not rediscovered:

- **The WASM side must not grow the memory either**, once it shares it. Its
  build fixes initial equal to maximum (Emscripten's `ALLOW_MEMORY_GROWTH=0`, or
  the equivalent linker limits for Rust), and its allocator works inside what it
  was given.
- **Nothing in the reservation is ever returned.** WASM memory is grow-only and
  this one does not even grow, so the pool's habit of handing idle regions back
  to the collector frees nothing for the regions inside it. The reservation is
  therefore sized from the ledger's peak, not guessed high.
- **Mobile is unmeasured.** Lazy commit was observed on desktop. Whether a large
  initial reservation succeeds, and what it counts against, on iOS and Android
  needs checking on a device before the size is set.

## Generators, and why the kernels are cursors instead

The stepped scan and bake keep their state in a small record — a phase, a row,
an emit position, a builder — advanced by a `step` function. That record is
exactly what a generator compiles to, and a generator would have written it in a
line: `for (row) { ...; yield; }`. The kinship with the rest of the field is
direct: Tokio bounds its cooperative tasks with a per-task budget, which is
`FrameBudget`; React's scheduler asks `shouldYield()`, which is
`budget.remaining() > 0`; Unity's coroutines resume next frame, which is a chunk
finishing over several.

They stay hand-written, for the reason the rest of this document keeps arriving
at. A cursor's state is data — a few numbers and handles — so it can sit in
linear memory, be inspected, be aborted by resetting a field, and be ported
one-to-one to a Rust struct with a `step` method. A generator's state is an
opaque frame on the JS heap that cannot enter linear memory, cannot cross into
WASM, and cannot be transferred. Rust's own raw coroutines are still
nightly-only, and its stable `async` would bring an executor along for a handful
of kernels.

Where a generator would fit is orchestration that stays in JS: a chunk's
"compose, step the scan, step the bake, attach" reads more plainly as one
generator than as the `advance_scan` / `advance_bake` / `holds_slot` trio it is
now. If chunk nodes move into WASM with everything else, the same argument
applies to them, so it is not worth changing yet. `scheduler.yield()` and
`postTask()` do not fit this model: the tick does budgeted work inside one
`requestAnimationFrame` callback, and yielding to the browser mid-frame would
scatter one frame's work across several tasks.

## Big and little, decided

**Big: one fixed-length memory of 256 MB, left exactly as it is.** The size is
fixed from the first frame — `initial` equals `maximum` — and it never grows.
What changes is how much of it the operating system backs with RAM: a page is
committed the first time it is touched and stays committed, since nothing here
can hand it back. After the first dense area the tab holds the whole 256 MB even
while the pool is using a fraction of it. On 2026-09-11 the ledger read
`pool live 73 MB, idle 218 MB` at rest after a drag whose peak had been 182 MB
live.

That is the working budget, not a leak, and it is left alone on purpose. The
bytes are never cleared: the pool's contract is that a borrower overwrites every
span before reading it, so stale contents are never observed — a quick format,
not a full one. Writing zeros would only commit more pages. And the committed
pages cannot be returned at all: neither JS nor WASM exposes the operating
system's discard, and the memory-control proposal that would is not available.
Every WASM application that follows the "reserve what you will need" advice
above has this shape. What is managed is the number: it is set from the ledger's
peak, and it moves when that peak does.

**Little: plain ArrayBuffers in a capped slab pool.** Little buffers have the
one freedom big lacks — the collector can take them back — and there is no
reason to give it up. So:

- **Fixed-size slabs**, each one job's worth: 4 MB, against a largest pooled
  block of 2 MB. A transfer moves a whole buffer, so a slab is the unit of a
  job.
- **Created when first needed, recycled when they come back**, released after
  sitting idle — the policy `max_idle_regions` already applies to plain regions.
- **Capped, and the cap is the backpressure.** When every slab is away, no new
  job starts; the slab count does for worker jobs what `MAX_INFLIGHT` does for
  fetches, and the ledger of which slabs are away is the ownership record the
  central scheduler needs anyway.

Not fully dynamic — a megabyte-scale allocation per job is collector pressure of
the kind that once stalled a frame for 4.9 s, and it has no backpressure. Not
preallocated either — nothing uses a little buffer until a worker exists, and
memory that can be returned should not be held for it.

One consequence to settle when the bake moves: three.js keeps
`BufferAttribute.array` for the life of a geometry, because a LOD blend
re-uploads the visibility span. A mesh baked into a slab therefore cannot return
to the pool while three holds it. Either the CPU copy is dropped after upload
and visibility kept separately, or the finished mesh is copied into big. Nothing
about little needs building before then.

## Open

- Whether a signed-in Figma file page is cross-origin isolated —
  `crossOriginIsolated` in the console of an open file answers it.
- The size of the reservation: the ledger's pool peak sets it, and the 100k per
  kind load in
  [20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md)
  says how far that peak will move; whether wasm32's 4 GB is a constraint at
  that load.
- How a large initial reservation behaves on iOS and Android.
- Whether other engines pay the same resizable-buffer penalty. It was measured
  only in V8; it does not change the decision, since the GPU rejection alone
  settles it.

## Update 2026-09-12: what of this exists, and the little memory's skeleton

Taking stock after the linear-memory work
([20260912-linear-memory-what-is-left.md](./20260912-linear-memory-what-is-left.md)),
against the model above:

- **Big**: exactly as decided — one fixed `WebAssembly.Memory`, 512 MB now
  (eight regions; the pool's peak moved it), never grown, every lane and block
  the client reasons over inside it. Past the reservation the pool spills to
  plain regions; that spill has to become a budget signal before a kernel reads
  the memory (BACKLOG.md), and is harmless until then.
- **The central scheduler, main-thread half**: built. The tick with its frame
  budget (sched/tick.ts) runs the synchronous work; the fetch scheduler
  (sched/fetch_scheduler.ts) runs the asynchronous, quota-limited, cancellable
  work, and its queue is lanes; the ledger reports both.
- **Little**: the skeleton is in — sched/slabs.ts, the capped, lazily made,
  idle-released slab pool with the ownership record of which slab is away for
  which job. Nothing borrows from it. The ledger's `little` line shows it beside
  the pool and the fetch quota, so the three resources the model names are read
  in one place.
- **Worker jobs**: not built, on purpose. A job is a Demand through the fetch
  scheduler whose `run` borrows a slab, fills it from big, posts it, copies the
  result back and gives the slab back; the worker itself is spawned once when
  first needed. The first job is the chunk bake, and the signal is the ledger's:
  the visible phase spending its whole share frame after frame while pending
  does not fall. Today's reading is a scheduler p95 at the still budget with
  pending at zero, so the signal is not there, and building the executor ahead
  of it would be the runtime-tier generalization the 2026-09-09 guardrail names.

So the "unified resource manager" is the tick, the fetch scheduler, the big pool
and the little pool, each reporting into the ledger — not a layer above them.
That is the shape the guardrail allows, and it is what the day the bake moves
will find in place.

## Status 2026-09-13

The first candidate — the chunk bake — was built for a worker and measured on
the phone, and not taken
([experiment/20260913-bake-worker-on-the-phone.md](../experiment/20260913-bake-worker-on-the-phone.md)):
the bake steps under the frame budget, in the module, on the main thread. Of the
open items: the iOS reservation is measured (256 MB on a handheld holds; what
happens past it is in
[20260912-why-the-destination-is-wasm.md](../20260912-why-the-destination-is-wasm.md));
the others are unchanged and not blocking.
