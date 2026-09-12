# Why the destination is WASM, and why three.js leaves with it

**2026-09-12** — [20260708-vision-and-scope-pivot.md](./20260708-vision-and-scope-pivot.md#corrections-to-earlier-readings)
recorded that parking the WASM core reopened the problem Rust was going to
solve, and named two ways out: keep BigInt in plain JS and accept the cost,
or replace PVF3's exactness with floating-origin rebasing onto float64.
Neither is being taken. The original posture — the core inside WASM, the JS
heap holding glue — is, and this records why that is a decision rather than
a reversal, and what leaves with it.

The BigInt cost itself is already recorded in that document and is not
re-argued here. What was missing is why the reasons for parking have
expired, and why three.js is part of the move rather than a separate
question.

## Why the pivot was right then

The pivot was not about whether Rust would be faster. It was about what a
port would have had to touch. In April the client's state was ordinary JS:
per-chunk fields on node objects, `Map`s keyed by strings, `Set`s of chunk
names, entry objects on bands, instance objects per decor placement. A core
rewritten in Rust would have owned none of that, so every subsystem would
have been rewritten twice — once to move its state somewhere a kernel can
see, and once to move the kernel. Parking it and shipping a client-server
MVP was the right call against that cost.

## What changed

Three prerequisites converged, none of them for this reason alone:

- **The client's state already lives in one `WebAssembly.Memory`.**
  [20260912-linear-memory-what-is-left.md](./20260912-linear-memory-what-is-left.md)
  is nearly closed: `CORE_POOL` is one fixed memory with `BuddyAllocator`
  and `CORE_ALLOC` over it, and chunk build data, the scheduler's per-chunk
  table, the object store's entries, decor instances and banks, the page
  and anchor registries, the fetch scheduler, the ray cache and the badge
  pipeline are lanes in it. That document states the reason in its own
  words — structures move "so that it can move to WASM without a second
  rewrite." The double cost the pivot was avoiding has been paid down
  deliberately, item by item.
- **The concurrency question is settled in a way that needs no shared
  memory.** [20260911-single-thread-core-and-when-to-spawn-a-worker.md](./20260911-single-thread-core-and-when-to-spawn-a-worker.md)
  fixed one core on one thread with cooperative scheduling, and ruled out
  SharedArrayBuffer and the COOP/COEP headers it requires. A main-thread
  core owning one linear memory is exactly the arrangement that needs
  neither: there is one owner, so there is nothing to share. Workers, if
  any, get their own small memory and exchange transferables.
- **The wire format was kept for a Rust reader.**
  [20260910-why-xtp-and-what-would-end-it.md](./20260910-why-xtp-and-what-would-end-it.md)
  kept XTP on the stated expectation of a Rust implementation of both
  halves. Its C layout, relative offsets and lazy reads mean the format
  does not change when the reader does.

## What the move is actually for

Resource ownership, in the sense the linear-memory document already learned
the hard way. Its first consequence — **a lane set has one owner, and a
consumer borrows it for the duration of a call** — was written after the
decor field kept a node's lanes past their release and drew trees the size
of a district. That rule is enforceable by convention in TypeScript and by
the type system in Rust; the same class of bug is a borrow-checker error
rather than a district of trees.

The second reason is that the language stopped helping. Writing a buddy
allocator, a typed allocator, `Vector` and `IndexedMap` over a raw
`WebAssembly.Memory` in TypeScript is writing the runtime a systems
language already has. The bake pipeline's arena-backed scratch is the same
tell. None of it is wasted — the lanes are the record format's fields
either way — but it is a reimplementation, and the implementation it
imitates is the one WASM gives access to.

Third: BigInt, as recorded. Worth adding the mechanics, which were not.
In V8 a BigInt is a heap object — a sign bit and a digit array — and is
immutable, so every arithmetic operation allocates a new one. There is no
small-value path. A PVF3 operation in a per-frame loop therefore allocates,
and the same operation in Rust is `i64` and `u32` in registers. The
alternative the pivot named, rebasing onto float64, is the field's standard
answer and remains available; what it costs is the exactness the coordinate
system was designed around, and the reason to prefer registers over a
rebase is that the design does not have to give anything up.

## Why three.js leaves with it

This is the part that was not previously argued, and it follows from the
linear-memory rule rather than from anything about rendering quality.

That rule keeps in the JS heap "the objects three.js and React require to
exist … and the handles that name a slot in linear memory." Two of the
remaining items exist for three.js alone: the deferred layout buckets,
where the note is that a `BuiltChunkBucket` is typed views plus a refcount
and `BufferAttribute` takes exactly those views, "which makes the handle
the glue boundary itself"; and the glue left as is by rule — `gltf_cache`,
`asset_store`, `highlight_boxes`, the billboard sprites and materials.
Strip three.js and the render-side glue layer does not shrink, it
disappears: what is left on the JS side is input, the UI, and the channel.

The second reason is the one-owner rule again. three.js owns geometry,
material and texture lifetimes, and a scene graph describing objects the
core already describes. Keeping it means two owners of the same facts,
which is the arrangement the decor bug was. A core that owns its memory and
talks to WebGPU directly has one.

React Three Fiber is not a separate decision. It is a reconciler over
three.js, and the declarative surface it is actually used for is small —
28 JSX 3D nodes across 14 files, six of them `<primitive object={...}>`,
which is three.js objects built imperatively and mounted. It goes when
three.js goes, and dropping it earlier is available as its own small step.
[20260816-csr-vs-ssr.md](./20260816-csr-vs-ssr.md) turns on R3F's inability
to server-render; raw WebGPU strengthens that conclusion rather than
disturbing it, since there is then no server-side path at all.

## Already de-risked

- The wire format, per XTP above.
- The state layout, per the linear-memory checklist.
- The concurrency model: one owner, no SAB, no cross-origin isolation.
- `CORE_POOL` is **one fixed memory**, which sidesteps the standard
  footgun of this design: `memory.grow()` detaches the old buffer and
  invalidates every cached typed-array view over it. A fixed reservation
  has no grow, so views stay valid for the life of the allocation.
- Float determinism improves rather than degrades. WASM specifies IEEE-754
  strictly and has no x87-era variance, so the exactness PVF3 protects for
  integers extends to the float math around it. One caveat to carry:
  transcendental functions (`sin`, `exp`) come from libm and can differ
  between toolchain versions, so determinism across versions means pinning
  that implementation rather than assuming it.

## What it still costs, and what is open

- **The three.js exit is larger than the renderer.** `Frustum`, `Matrix4`
  and `Raycaster` are already renderer-independent, and exact bounds and
  voxel ray tests are ours. What is not: glTF loading (`gltf_cache.ts`,
  `asset_store`), texture arrays, the postprocessing chain, and the
  billboard depth trick — `render/scene_depth.ts`'s world-depth copy plus
  `billboard/anchor_occlusion.ts` writing `gl_FragDepth` from an upright
  twin. That last one is a real shader and a real pass, not a library call.
  Cost it before the cut, not during.
- **The read direction has no protocol yet.**
  [20260908-command-channel-for-state-mutation.md](./20260908-command-channel-for-state-mutation.md)
  settles how state is mutated. It does not settle how the UI learns that
  state changed once the truth is in the core: settings panels, markers,
  POI badges and chat currently read stores. A core has no idea what the UI
  cares about, so without a subscription or dirty-flag protocol over the
  channel the default failure is copying the world into JS every frame.
  This is the piece most likely to be underdesigned and should be designed
  with the channel, not after it.
- **XTP packets need their alignment respected on arrival.** The format's
  C layout only pays off if a packet lands on an 8-byte boundary in linear
  memory (16 if SIMD reads it); a copy into an arbitrary offset breaks the
  cast. Allocate packet blocks aligned, and read them through checked casts
  (`bytemuck`/`zerocopy`) rather than unchecked ones.
- **Spill past the reservation becomes load-bearing.** BACKLOG already
  records it: past 512 MB the buddy pool hands out plain `ArrayBuffer`
  regions "which a kernel over the wasm memory cannot see," harmless while
  everything computes in JS. When a kernel lands, a spill has to trigger
  reclaim or refuse. That item is a prerequisite of this move, not a
  neighbour of it.
- **WebGL reach versus a hard cut to WebGPU** is undecided and deliberately
  not decided here. Two renderers is two of everything; one renderer is a
  reach cost on whatever has not shipped WebGPU. Name it when the exit is
  scheduled.

## What would say no

The move is wrong if the linear-memory checklist stalls with the world's
per-frame state still split between lanes and the heap, because then the
double-rewrite cost the pivot avoided is back. It is also wrong if it is
argued on performance: the linear-memory document's own framing applies
here unchanged — the state is linear because the design says so, and the
core moves because the design says the core owns its memory. A benchmark is
not the case for it and a disappointing one is not the case against it.
