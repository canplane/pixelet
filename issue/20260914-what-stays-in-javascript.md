# What stays in JavaScript

**2026-09-14** — the evening the renderer, the shaders and the scheduler
were all the module's, and the question came up of where the line finally
lies. Decided: JavaScript keeps only what the browser gives it. Everything
that holds state or makes a decision about the world is the Rust core's,
and the crates are cut along the layers that leaves.

## The line

The precedent is Figma: the editor is C++ in WebAssembly, and the
JavaScript around it is the UI, the browser's input and I/O, and glue
that carries values across the boundary without deciding anything
([Wallace, "WebAssembly cut Figma's load time by 3x"](https://www.figma.com/blog/webassembly-cut-figmas-load-time-by-3x/);
the same shape a decade on). pixelet takes the same line, and states it
as a rule rather than a tendency:

**JavaScript keeps what the browser provides, and only that.**

- **Interrupts**: pointer, wheel, pinch, keyboard; geolocation and the
  device pose; resize and tab visibility; a setting toggled; a network
  response arriving.
- **I/O**: `fetch`, `localStorage`, rasterising text on a canvas (the
  badge sheet), the WebGPU calls themselves, the `requestAnimationFrame`
  loop that drives the tick.
- **UI**: the React tree and the DOM overlays.

Each of those is a line of glue in one of two directions: an event becomes
a command on the submission ring, or a value the module holds is read
back for a control to show. What JavaScript does not do is compute a
state of the world from them — damp a camera, decide which page to fetch,
place a badge, compose a coordinate. If a piece of TypeScript holds a
number the module would have to ask for, it is on the wrong side.

**Restated 2026-09-14, later the same day, because the audit had begun to
read it the other way round.** The module is not an accelerator that
JavaScript hands its hot paths to. It is the program: one WebAssembly
module, complete and portable, that a browser page, a worker, or a native
shell can host without the host knowing what a chunk is. In one line:
TypeScript is the shell, WebAssembly is the kernel, and the tick is the
interrupt — the shell hands the kernel time, the kernel handles what
arrived and hands the time back. What that decides about the boundary:

- **No bindgen.** A table of a hundred exported functions, each a typed
  call into the module, is bindgen by hand — a host written against it
  has to know the module's API by name, and a second host has to be
  written again. The boundary is three things and only three: **the
  rings** (a submission ring and a completion ring in the module's linear
  memory, every command an entry, every result an entry), **shared
  memory** (lanes the host lands bytes in and reads records from, at
  addresses the module hands out through the rings), and **a cooperative
  scheduler** (the host gives the module time by ringing its doorbell —
  `pxl_tick` under the frame loop, `pxl_drain` outside it — and the
  module yields when the budget is spent). What is exported is what
  cannot be a ring entry: the memory, the rings' addresses, the
  doorbells, the panic hook. Everything named `pxl_*` today that is not
  one of those is on its way into the rings.
- **The terms of folding the exports into the rings (2026-09-14).**
  1. The Rust does not bend to JavaScript. Assembling and taking apart
     what crosses is the shell's job, in XTP; the kernel reads a packet
     where it lies and writes one where it is asked to. serde for the
     RPC is avoided — tolerated only as a stopgap that says so.
  2. The RPC is most likely XTP-shaped, and the fold is tried in that
     shape first: a ring entry names an op and a packet in shared memory
     (its offset and length), not a dozen scalar arguments laid out per
     op.
  3. The exchange is interrupts over shared memory. An export that
     remains is a primitive that hands over the turn — the tick, the
     drain — never a typed call per operation.
  4. What goes over the network, and in what format, stays the shell's
     to lead for now. It is a grey zone, not decided.

  **The message, as built (2026-09-14).** What a frame
  (`@xpute/core/ipc/frame.ts`) needs once the transport is shared memory:
  four words — `tag`, `cmd | flags`, `result`, `packet` — and an XTP
  packet in the ring's own arena, its length in its header. The frame's
  args/body split, its spill and its worker id are gone (the packet is a
  tree, the arena holds any size, one ring joins two sides); `RES` tells a
  reply from a signal on the completion ring, and `ACKREQ` asks for a
  reply. The producer rewinds its arena when it finds the ring empty, so
  nothing is allocated per message; a full ring or arena is EAGAIN, a
  packet larger than the arena EMSGSIZE. A command is a number, `major <<
  8 | minor` (`@xpute/core/ipc/cmd.ts`), the minors below 0x10 being
  CREATE/READ/UPDATE/DELETE; what the majors are is pxl's
  (`@pxl/shared/spec/cmd.ts`, one table the glue and the module both
  read, each command's packet listed beside its number). The chunk
  commands and every signal the scheduler raises travel this way now; the
  rest of the `pxl_*` table is still bindgen, and moves family by family.
  The frame itself stays as it was, for the worker boundary.

  A query is a submission with `ACKREQ` that the glue rings for until its
  reply comes (`CoreBuild.call`): the ray and the decor hit went first, as
  the cost check. On V8 with an empty world, the export was 91 ns a ray;
  the ring is 733 ns. The ring itself is not the cost — a call with no
  packet is 92 ns, the export's price — but the TypeScript XTP encoder
  (510 ns for one nine-double packet, a node object per leaf) and reader
  (314 ns for a fourteen-double reply) are. At a pointer's rate that is
  under 50 µs a second, and what it points at is BACKLOG P1's "XTP encoder
  without per-leaf objects", not the ring.

  The writes followed: an object leaf, a chunk's reindex and clearing the
  store (`OBJECT_*`), what a leaf's state paints as (`STATE_UPDATE`), and
  the tables taught once — the terrain's materials as one packet
  (`TERRAIN_UPDATE`, where there were 1,024 calls) and every decor kind
  with its rules as one (`DECOR_KIND_UPDATE`, where there was a call per
  kind, rule and allowed type). A drain now applies every submission
  waiting and holds back only what they make, in the module and in order,
  until the completion ring has room; so the read exports still standing
  (the object store's lanes, the reference paints) apply what waits first
  and see every write sent before them, and the glue's `settle` does the
  same without handing out a signal, for the reindex whose change log it
  reads next.

  Then the object store's reads became queries too (`OBJECT_READ`, `_LANES`,
  `_CELL`, `_COLUMNS`, `_STAT`), answered in the result word where one
  number is the answer and in a reply packet where it is not. A lane
  still crosses as its address: `OBJECT_LANES` replies with where the
  chunk's band, position and state lanes are, and the glue views them in
  place until the next write. What still applies the waiting submissions
  before it reads is only the reference paints (`pxl_terrain_read`,
  `pxl_terrain_paint`, `pxl_object_paint`), which the tests call.

  Then the scene beside the chunks: the hover ghost, the location puck,
  the billboard sets, the actors, the dev gizmos and the environment's
  theme and observer (`GHOST_*`, `PUCK_*`, `BILLBOARD_*`, `ACTOR_*`,
  `DEBUG_UPDATE`, `ENV_*`). Two things this family decided. A lane of
  floats — a billboard's instances, the puck's zone, the gizmos' vertices,
  a theme — crosses inside the packet, not as an address: the glue's
  scratch went back to its allocator the moment the call returned, and a
  command is applied later, so an address would name memory already
  reused. And the renderer drains before it takes the lists, so what a
  phase after the module's tick sends (the chat bubble, the gizmos, a POI
  placement) is still drawn in the frame that sent it, as the direct calls
  were.

  Then a model's loading (`MODEL_*`): the glue asks for a model and is
  answered with its id and where its mesh block is, writes the vertices
  and indices there itself — a block in the module's memory the module
  handed out, which stays valid, so it crosses as an address — and sends
  the joints, the clips and their channels (keys copied into the packet),
  then the seal. A query's tag is its call's, so a query about a subject
  (a model's handle, a clip of a model) carries the subject in its packet.
- **The kits are the same kit twice.** `@xpute/core` and `@pxl/shared`'s
  space are what a host and the module both stand on, and the module
  cannot import TypeScript. So each is kept in both languages, file for
  file and export for export, in the same tree, with a harness
  (`script/pair.ts`, in `deno task check`) that fails when a file, an
  export or a test on one side has no pair on the other, and vectors the
  TypeScript writes and the Rust must reproduce bit for bit. A piece both
  runtimes need is not a duplicate to move to one side. The check runs one
  way: what the TypeScript has, the Rust has, and a TypeScript export left
  out is listed with its reason. The Rust may carry more — the kernel is
  where the work is going, and a TypeScript twin of a kernel-only piece is
  written the day something in the shell needs it, not before. The tree is mirrored as it is, however it might
  be better arranged — arranging comes later, on both sides at once.
- **Bit for bit, except where the TypeScript cannot be.** Two exceptions
  came out of the first vectors. `Math.sin`, `exp`, `log` and the rest are
  no longer fdlibm in V8 but LLVM libc's correctly rounded functions, and
  the kernel does not follow the browser's libm: it carries FreeBSD msun's
  (`xpute_core::kit::fdlibm`) so a result is one set of bits on every
  toolchain, and the vectors hold the functions to within one ulp and
  what is computed from them to a few, while a lattice address, which a rounding tie must never move, is
  held exact. And a vector that catches the TypeScript wrong
  (`unpack96` losing its low limb through `Number()`) is not copied into
  the Rust; the test states the divergence and fails once the TypeScript
  is fixed.

**The destination, whole (2026-09-14).** The client is one Rust kernel
in WebAssembly drawing through WebGPU, with TypeScript as the shell —
the UI, the browser's interrupts and I/O, and the calls that execute
what the kernel lists. The server is Rust too, in time; it is not urgent,
and nothing about the client waits on it. When it comes, `pxl-shared` —
PVF3, the XTP reader and the bake wire format — is the one crate the
server and the client kernel both build on, and `@pxl/shared` shrinks to
what the shell still reads. The mirror the harness holds today runs
TypeScript → Rust because the TypeScript is the finished side; the day
the kernel is the original, the direction turns.

Two things are explicitly on both sides:

- **Marshalling** (XTP). The wire format is a contract, and each side
  reads it: the server writes packets in TypeScript, the core reads them in
  Rust when they land in its memory. A reader in Rust is what lets a packet
  be dropped into the page section and read where it lies, with no decode
  and no copy in between, and it is what a native shell needs regardless.
- **The renderer's contract** (the draw list, the block layouts, the
  shaders and pipeline records). The core writes it; a renderer executes
  it — the TypeScript one through WebGPU today, a wgpu one later. Neither
  decides anything; the contract is data.

## What the line moves, in order

The audit of 2026-09-14 (after the reference split) found the pieces of
world state still in TypeScript. Each is a move of the same shape the
earlier ones had — cut it behind a boundary that carries typed arrays,
then make what sits behind the boundary Rust — and the order is by what
each unlocks:

1. **The view, with PVF3.** The orbit camera, the root frame, the damping
   steps, `map_center` and the view state, the range anchor
   (`module/world/frame/`). Pointer events become commands — pan, rotate,
   zoom, locate, jump — the module damps, and it writes the frame it
   reads today from the glue; the renderer reads `view_proj` from the
   module as it reads the environment. PVF3 enters the crate here, as the
   coordinate type (`space/pvf3.rs`, the pair of
   `@pxl/shared/spec/space/pvf3`): the glue stops composing coordinates
   and reads doubles for the controls that show one. "The pair of" is
   the rule for the kits: what both runtimes need at all — `@xpute/core`
   and `xpute-core`, PVF3 and `space/pvf3.rs`, the XTP codec — is kept
   twice, one copy per runtime, symmetric; a move adds the Rust copy and
   never takes the TypeScript one away while anything there still reads
   it. The three hand copies
   of the lattice word's packing in the crate today (sched.rs, decor.rs,
   the wasm crate) are the first thing it replaces.
2. **Page demand and residency, with an XTP reader.** Which pages the
   world wants is already what the module mounts by; it says so
   (`Event::Want`), the glue fetches and lands the bytes, and the module
   reads them where they lie. The reader is `xpute-core/io/marshal/`,
   held to the TypeScript cursor packet for packet — the test that would
   also settle the one truncated-packet read on record.
3. **The POI index and placement.** With 2, the module reads a POI page
   itself and placement is a phase of the tick; the glue keeps the sheet
   the badges' text is rasterised on.
4. **The object store's persistence.** The record is the module's
   (`Placement`); writing it to `localStorage` is I/O and stays.

After these, `core_tick.tsx` is what its name says — commands in, one
tick, the lists out — and the "input decisions" left (the hover cursor's
placement, follow mode's heading) are commands too.

## The crates, by layer

The same audit put the crates' layers in view. Dependencies run one way
through them, which is what makes each a crate rather than a directory:

```
xpute-core   the generic kit: pools, the fixed heap, the rings, the change log,
             the tables that agree with V8 — no domain knowledge; the pair of
             packages/xpute/core, tree for tree
pxl-render   the renderer's contract: the draw list, the mesh block's layout,
             the WGSL and the pipeline records, the frame uniform — data a
             renderer executes; the pair of apps/web/src/render/{gpu,mesh}
pxl-core     the world: the bake (voxel, volume, runs, scan, mesher, terrain)
             and what is where (the scheduler, objects, decor, environment,
             models, markers, billboards, the raycast) — depends on both above
pxl-wasm     the binding: numeric exports over the three, a fixed heap over
             the imported memory; pxl-native beside it when a shell exists
```

Not cut yet, and why:

- **`pxl-shared`** — the pair of `packages/pxl/shared`: PVF3, the
  cell/terrain model, the wire formats, the records. Today it would hold
  the sparse tree and the voxel encoding and little else; it is cut when
  PVF3 and the XTP reader arrive (moves 1 and 2), which is when it has a
  contract to be.
- **bake and world apart** — the scheduler drives the builds and the bake
  reads the object store; they are one layer, "what is inside a chunk",
  and a crate boundary between them would be crossed in both directions.
- **`pxl-render` today** — `render/draw.rs` imports nothing of the crate
  and `render/shader.rs` only it; six world modules fill the list. The cut
  is a `git mv` and a `Cargo.toml`. The one judgement in it: the mesher's
  lane order and strides are the renderer's fact (the block layout is
  what the pipeline record declares, and naga holds the record to the
  shader), so the mesher takes them from `pxl-render` and three places
  that know the layout become one.

## What this is not

Not concealment, and not a rewrite for its own sake. The WGSL is text the
GPU process compiles either way; the core's memory is the page's memory.
What the line buys is the thing the destination issue asked for
([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md)):
one implementation of the world, deterministic across toolchains, that a
native shell runs unchanged — and, on the way, a TypeScript side small
enough to read in an afternoon.
