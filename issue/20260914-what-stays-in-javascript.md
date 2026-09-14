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
   and reads doubles for the controls that show one. The three hand copies
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
