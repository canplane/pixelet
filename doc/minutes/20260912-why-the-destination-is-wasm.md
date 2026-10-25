# Why the destination is WASM, and why three.js leaves with it

**2026-09-12** — [20260708-vision-and-scope-pivot.md](./archive/20260708-vision-and-scope-pivot.md#corrections-to-earlier-readings)
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
  [20260912-linear-memory-what-is-left.md](./archive/20260912-linear-memory-what-is-left.md)
  is nearly closed: `CORE_POOL` is one fixed memory with `BuddyAllocator`
  and `CORE_ALLOC` over it, and chunk build data, the scheduler's per-chunk
  table, the object store's entries, decor instances and banks, the page
  and anchor registries, the fetch scheduler, the ray cache and the badge
  pipeline are lanes in it. That document states the reason in its own
  words — structures move "so that it can move to WASM without a second
  rewrite." The double cost the pivot was avoiding has been paid down
  deliberately, item by item.
- **The concurrency question is settled in a way that needs no shared
  memory.** [20260911-single-thread-core-and-when-to-spawn-a-worker.md](./archive/20260911-single-thread-core-and-when-to-spawn-a-worker.md)
  fixed one core on one thread with cooperative scheduling, and ruled out
  SharedArrayBuffer and the COOP/COEP headers it requires. A main-thread
  core owning one linear memory is exactly the arrangement that needs
  neither: there is one owner, so there is nothing to share. Workers, if
  any, get their own small memory and exchange transferables.
- **The wire format was kept for a Rust reader.**
  [20260910-why-xtp-and-what-would-end-it.md](./archive/20260910-why-xtp-and-what-would-end-it.md)
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
[20260816-csr-vs-ssr.md](./archive/20260816-csr-vs-ssr.md) turns on R3F's inability
to server-render; raw WebGPU strengthens that conclusion rather than
disturbing it, since there is then no server-side path at all.

## Already de-risked

> History: this list and the next name the TypeScript client of this date.
> `CORE_POOL`, `BuddyAllocator`, `gltf_cache.ts`, `render/scene_depth.ts`
> and `billboard/anchor_occlusion.ts` are gone with it; the client is the
> Rust under `rust/client/` (`kernel`, `render`, `shader`).

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
  [20260908-command-channel-for-state-mutation.md](./archive/20260908-command-channel-for-state-mutation.md)
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
  neighbor of it.
- **WebGL reach versus a hard cut to WebGPU** is undecided and deliberately
  not decided here. Two renderers is two of everything; one renderer is a
  reach cost on whatever has not shipped WebGPU. Decided below, 2026-09-13:
  WebGPU only.

## What would say no

The move is wrong if the linear-memory checklist stalls with the world's
per-frame state still split between lanes and the heap, because then the
double-rewrite cost the pivot avoided is back. It is also wrong if it is
argued on performance: the linear-memory document's own framing applies
here unchanged — the state is linear because the design says so, and the
core moves because the design says the core owns its memory. A benchmark is
not the case for it and a disappointing one is not the case against it.

## Update 2026-09-13: what a phone's memory said, for the renderer that replaces this one

A Safari memory timeline and heap snapshots, on an iPhone 13 mini and on
the desktop, were read at the end of a day of phone work. What they said
is recorded here rather than acted on: the numbers belong to the three.js
build, which this document says is leaving, and a ceiling set on it now
would be thrown away with it. The facts carry.

- **The page's budget on the phone is a gigabyte and change.** The mini
  took memory-pressure events at a reported 1.6–2.1 GB and reloaded the
  tab. GPU memory counts against the same budget: unified memory, and
  WebKit's GPU process is charged to the page.
- **Where the memory actually is**, from the snapshots (the live heap
  after a collection, not the timeline's counter): the core reservation's
  touched pages (~300 MB), the pool's spill past it on the desktop (256
  MB in plain regions), the badge sheet's canvas (16–64 MB), the JS glue
  (30–40 MB of objects — three's, React's, ours), and on the GPU side the
  chunk meshes, textures and the composer's targets.
- **A multisampled target is pixels × samples × (color + depth bytes).**
  At 3.2 Mpx, four samples and half-float that is 150 MB, and it cost a
  millisecond of frame time — the composer's defaults set it, not a
  decision. The renderer that replaces this one creates that target
  itself, with this formula and the phone's budget in hand.
- **A large canvas handed to the browser is read whole.** Draw small,
  upload small; a glyph sheet is drawn per rectangle into a scratch and
  uploaded from there.
- **Safari's "JavaScript" memory is a GC counter, not a footprint.** WebKit
  charges GPU uploads and canvas backing to it and recounts at
  collection, so a bake storm reads as gigabytes of sawtooth over a live
  heap of a third the size. A heap snapshot is the number to read; the
  timeline is the number to ignore.
- **What the glue hides.** Every one of the day's blind spots sat at the
  three.js boundary: whole-canvas texture uploads, a `clone()` that
  re-uploaded the sheet, a composer default that sized the largest
  allocation on the page, buffer lifetimes we trust rather than count.
  None was visible to the ledger; each was found by measuring around the
  glue. This is the control argument for the destination, with cases.
- **It is not the engine.** The same page's heap snapshot in Chrome read
  628 MB (542 of it ArrayBuffer, 40 of objects) against Safari's 1.2–1.4
  GB, and the difference was the scene, not the runtime: the Safari
  session was zoomed out with the pool spilled past the core (`reserved
  768 MB`, a value Chrome's own ledger report had also shown at that
  zoom). Chrome's JS heap over twenty seconds was flat at 74–102 MB.
  Neither engine leaks; both hold the same working set, and the phone
  dies because that working set exceeds its budget.
- **Steady-state CPU is 2 ms a frame; the bake is 14–21 ms a unit on the
  phone.** WASM has nothing to buy in the steady state. It buys the load
  — the bake, off the thread — and, with WebGPU, the glue.

## Decided 2026-09-13, late: one graphics path, and a floor

**WebGPU only.** No WebGL path survives the rewrite. The floor is what
runs WebGPU on a phone: iOS 26 (iPhone 11 and later) and Chrome on
Android since 2024; every desktop browser has it. A device older than
that cannot carry this game's memory and GPU load whatever the API, so
the fallback would be a path nobody could use.

**What that closes.** Safari's WebGL path balloons its reported memory to
four gigabytes during a bake storm and freezes for seconds at the
collection that follows, on a fresh page with no allocation of ours
behind it; Chrome shows nothing of the kind. The mechanism is on WebKit's
side of the WebGL boundary — the likeliest candidate is the CPU copy
WebKit keeps of every element-array upload until its wrapper is
collected, with JavaScriptCore deferring that collection in proportion
to a live heap it cannot shrink — and it was left unconfirmed on purpose:
a ten-minute experiment would name it, and the name would be worth
having only if a WebGL path were going to stay. It is not. Recorded so
the next reader does not spend a day on it.

## Scheduled 2026-09-13: Rust, piece by piece

The move is the priority and the core is Rust. It is taken from the
smallest separable piece outward rather than as one cut: each piece is
first put behind a boundary that carries typed arrays, in TypeScript, and
only then does what is behind the boundary change language. The chunk bake
in a worker is first — it has the phone's numbers behind it and the
mesher already takes and returns typed arrays. The order is P0 in
[the backlog](../backlog.md).

"Cost it before the cut", above, is not a gate. The decision does not turn
on implementation size, and no estimate would change it. What that line
was right about is that three.js does work for us that would disappear
silently, so the renderer is built against a list of it:

- [x] glTF loading and the model cache (`gltf_cache.ts`, `asset_store`)
- [x] texture arrays, and the texture pack's sheet into one
- [x] the multisampled target and the composer's chain — sized by the
      formula in the update above, not by a default
- [x] the billboard depth pass (`render/scene_depth.ts`,
      `billboard/anchor_occlusion.ts`'s `gl_FragDepth` from an upright twin)
- [x] shadows, the sky and its render order
- [x] frustum culling and bounds, instancing, and buffer lifetimes counted
      rather than trusted

Filled 2026-09-14, when the WebGPU renderer went through (`render/gpu`,
BACKLOG's record of the move).

## Recorded 2026-09-13: the web's ceiling, and the core that is not tied to it

**The ceiling is WebKit's, not the browser's.** On iOS every browser and
every WebView is WebKit, and a page — in Safari or in a WKWebView, which is
what Capacitor ships — runs in a WebContent process with its own memory
limit. Others report it as a 2048 MB jetsam ceiling on iPhone (iPad is
reported higher); the mini's pressure events at 1.6–2.1 GB agree. The
app-level `increased-memory-limit` entitlement applies to an app's own
process and only on some models, per Apple
([forum](https://developer.apple.com/forums/thread/770868)); nothing found
says it lifts the WebContent process's. WebGPU in WKWebView follows the
OS default, which is on from iOS 26
([forum](https://developer.apple.com/forums/thread/770862)). So a
Capacitor wrapper buys distribution and native APIs, not memory.

**What escapes it is a native app, and the core does not have to change to
get there.** WebGPU is a specification before it is a browser API. wgpu
implements it natively over Vulkan, Metal, Direct3D 12 and OpenGL ES, and
in the browser over WebGPU and WebGL2 ([wgpu.rs](https://wgpu.rs/)); the
same Rust renderer and the same WGSL run in both. Rerun's viewer ships one
Rust + wgpu + egui codebase as a native binary and as WebAssembly
([egui](https://github.com/emilk/egui)). Figma, the other precedent, keeps
editing off phones altogether: mobile browsers and its mobile app are
view-only
([system requirements](https://help.figma.com/hc/en-us/articles/360039827194-What-browsers-does-Figma-support),
[mobile app](https://help.figma.com/hc/en-us/articles/1500007537281-Guide-to-the-Figma-mobile-app)).

**The condition is the boundary, not the graphics API.** What does not
port is the shell: React, `fetch`, OPFS, geolocation and sensors, workers.
So the core does not call a platform API. Commands arrive in the shape
[20260913-boundaries-commands-and-what-comes-back-from-april.md](./archive/20260913-boundaries-commands-and-what-comes-back-from-april.md)
settles, so a web UI and a native one call the same ones; work is a Demand
in a slot, executed by a worker on the web and by a thread natively; I/O,
the clock, storage and sensors are handed in, not reached for. A core that
imports `web_sys` for convenience is a web core.

**And inside the ceiling, the budget.** Leaving WebGL removes WebKit's
upload copies; it does not make the working set fit. That is the byte
budget paid for in LOD, in BACKLOG P1.
