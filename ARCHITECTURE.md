# Architecture

A spatial engine that turns real-world geographic data and player-built objects into one voxel-style 3D world.

The document has two halves. **Runtime** describes how the client runs: the crates and packages, the turn, the rings, and the memory. **Engine** describes what the client runs: coordinates, the spatial hierarchy, terrain, the chunk pipeline, LOD and rendering, the camera, catalogs, and wire formats.

Three properties hold throughout:

- The world's state lives in **one linear memory**.
- The work runs in **turns under one quota**.
- What the client measures about itself goes to a **read-only ledger** that never feeds back into a decision.

The destination is WebAssembly and WebGPU. The kernel decides and draws, and the TypeScript that remains is the shell.

Earlier states of the project are preserved on `checkpoint/20260107-float`, `checkpoint/20260405-xpute` and `checkpoint/20260409-wasm-aborted`.

---

# Runtime

## Crates and packages

```text
xpute-core ◀── pxl-spec ◀── pxl-render ◀── pxl-kernel ◀── pxl-wasm ──▶ kernel.wasm
     ▲                            │              │            │                │
     └── xpute-runtime ◀──────────┴──────────────┴────────────┘                ▼
                                                                            apps/web
@xpute/core ◀── @xpute/runtime ◀───────────────────────────────────────────────┘
```

| Crate / package  | Role                                                                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `xpute-core`     | The generic kit: collections and lanes, the codecs and XTP, the errno table both languages are written from, the libm that agrees with V8. |
| `xpute-runtime`  | What runs over it: the rings and the change log, the scheduler's three levels, and the memory a module is given — the slab over a buddy.   |
| `pxl-spec`       | The model: PVF3 and the spatial hierarchy, the cell catalogs, the command and message numbers, the bake's formats, the asset tables.       |
| `pxl-render`     | The view: volumes, the column scan and the mesher, glTF, the LOD ladder, the billboards' atlas, the draw list, the shaders, the renderer.  |
| `pxl-kernel`     | The kernel: the chunk scheduler and nodes, the layers and their stores, fetches and loads, the camera and sensors, the turn, the ledger.   |
| `pxl-wasm`       | The module: the allocator and executor it installs, the linker flags that fix the memory's front, the rings, and the exports.              |
| `@xpute/core`    | The same kit in TypeScript: XTP, the codecs, the errno table, the words.                                                                   |
| `@xpute/runtime` | The same runtime's host end: the rings' doorbell (`ipc/doorbell.ts`), the strobe and quantum (`sched/`), the reservation (`sys/mem.ts`).   |

Those are what the client is built from. Three more crates sit over `pxl-spec`
and never reach the browser: `pxl-geo` and `pxl-api` are the two servers, and
`pxl-geo-bake` holds the three page bakes and the dev tile server. A fourth,
`pxl-geo-spec`, is what Overture's own values mean, read by the server and the
bake alike.

What fills the databases those read is not a crate. The Overture ingests and
the elevation pipeline import nothing of the model — they move a provider's
columns into Postgres columns — so they are scripts under `infra/geo/`, beside
the schema each one writes. The asset tree's own tooling is the other way
round: it generates `pxl-spec`'s asset table and the shell's, so it is scripts
under `asset/script/` that depend on the crates rather than the reverse.

Two boundaries are kept on purpose:

- **xpute has no domain.** It holds no coordinates, terrain or tiles, so it stays portable to other projects. A mechanism belongs to xpute, and the policy over it belongs to the kernel. The tick's phases are xpute's, for example, while their shares and the steps registered in them are the kernel's.
- **The view never names the kernel.** `pxl-render` knows the model and nothing of `pxl-kernel`. The kernel writes the frame's uniforms into the stage lane, builds a `FrameInput` from the draw list and the stores, and calls `render_frame`, which returns what the frame cost. This is the same split as Godot's servers or Bevy's extract step.

`pxl-kernel` knows nothing about WebAssembly or a browser. The host's clock, storage, text measurement and entropy reach it as function hooks that `pxl-wasm` installs (`sys/host.rs`), so the same code runs natively under `cargo test`.

**The kernel finds nothing by a string.** An identity is a number decided in data: a catalog JSON, the asset ingest, or the bake. That covers a command, a decor kind, a character variant, a sky theme, an asset and a message. The kernel indexes by that number. The words a message is shown in are the shell's locale files.

## Shell and kernel

The shell (`apps/web`) is a driver. It does five things:

- It reserves the memory and instantiates the module (`kernel/core_mem.ts`, `kernel/module.ts`).
- It runs the turns (`@xpute/runtime/sched/strobe.ts`) and submits what a frame carries: the viewport, the gap since the last frame, the measured frame interval and the I/O credits.
- It forwards pointer, wheel, sensor and visibility events as commands.
- It answers the kernel's signals: it starts and cancels the fetches the kernel names, reads a response body into a block the kernel gives it, decodes images, and runs the sensors' watches.
- It hands a canvas over as a surface. The canvas is transferred to an `OffscreenCanvas` and from then on is drawn by the kernel alone, through the calls in `device/gpu.ts`.

The UI renders one snapshot, a JSON document the kernel writes after a turn when it has changed. Every control on the UI is an action the kernel applies. What draws on the canvas is the kernel's, and what is only UI state is the shell's.

## The turn

The host and the guest take turns like a clock's two edges.

```text
rising edge    host → guest   interrupt(quota_ms)
               the guest applies what waits on its submission ring,
               polls its tasks on the quota, posts to its completion ring
falling edge   guest → host   the call returns wake_ms:
               0 = the next frame · n = in n ms · NO_WAKE = none
```

A turn runs on an animation frame, and only when something asked for one: the shell (an input, an arrival) or the kernel (its last falling edge). The host runs no heartbeat, so an idle world costs nothing. A doorbell rung with no quota and no frame only applies what waits, which is how a UI action takes effect inside the click that caused it.

Scheduling has three levels, from outside in:

1. **Quantum** (`xpute sched/quantum`, the host's). The quota is a share of the frame, and the frame interval is measured rather than assumed. It is the 25th percentile of recent gaps between interactive turns, ignoring gaps longer than 50 ms. A turn is interactive when there was input within `settle_ms` or the guest's last falling edge asked for the very next frame. An interactive turn gets the frame less a margin, and any other turn gets `batch_frames` frames. An overrun comes off the guest's own next quota, down to nothing: a constant bandwidth server's rule. The policy numbers (margin share, batch frames, settle time) are the kernel's (`sched/tick.rs`'s `QUANTUM`), and the boot block carries them out to the host.
2. **Tick** (`xpute sched/tick`, the guest's). One tick per frame runs its phases in a fixed order against the quota, counted from the rising edge. The order is the priority:

   | Phase         | What                                                                                   |
   | ------------- | -------------------------------------------------------------------------------------- |
   | `interaction` | input, camera damping, environment, the view frame, the companion; unbudgeted and O(1) |
   | `visible`     | the chunk scheduler's pass                                                             |
   | `content`     | what is about to be on screen; guaranteed `content_share` of the budget                |
   | `cosmetic`    | what only looks better for running (debug gizmos)                                      |
   | `report`      | the ledger, last, so it sees everything                                                |

3. **Pass** (`xpute sched/frame_budget`). A list of steps runs until the budget or a step cap says stop. **Forward progress is the invariant**: the first step that does work always runs, so one expensive item cannot block everything behind it. `hard_slack_ms` bounds a step that costs time but reports no work.

Outside the tick, the kernel's work runs as **tasks** on an executor the module installs, the way a global allocator is installed: `embassy-executor`'s raw executor behind `xpute sched/task`. The kernel spawns six tasks: the frame, fetch decoding, glTF parsing, chunk builds, decor assets and the object store's save. A task yields with `yield_if_spent` or `next_turn` (`xpute sched/edge`) and is woken on the next rising edge, never within the same poll. The **frame task** is one task because a frame has an internal order: the effects (fetchers, followers, markers), then the tick's phases, the fetch pump, and the draw. Nothing depends on the order in which the executor polls tasks.

**I/O is granted by credits** (`xpute sched/io`). The kernel does no I/O itself. Each category syncs its entire wishlist once a turn. Anything no longer wished for is dropped if queued or aborted if in flight, and one pump per turn starts at most as many requests as the host granted credits, in priority order across categories.

**The ledger** (`pxl-kernel sched/ledger`) records frames (pacing, per-phase time, GPU ask, heap gauges, LOD pressure, draw census) and tasks (per-job waits) into rings whose layout is `spec/kernel/sched/ledger/ledger.json`. The shell reads them in place (`ledger_read`, `ledger_log`) without allocating, and prints a report or exports a Chrome trace. Nothing the ledger holds is read back into a scheduling decision.

## Rings and the channel

Everything after boot crosses in three rings in the shared memory: submission (shell → kernel), completion (kernel → shell), and a spill that completions overflow into so that order is kept (`xpute ipc/ring.rs`).

```text
descriptors                                   payloads, elsewhere in memory
┌──────────────────────────────────────────┐  ┌────────┬────────┬───┬───────────┐
│ header: capacity · head · tail ·         │  │ slot 0 │ slot 1 │ … │ slot 2047 │
│         slot_bytes · slot_base · 3 rsv   │  └────▲───┴────────┴───┴───────────┘
├──────────────────────────────────────────┤       │ 4 KiB each
│ message n: tag                           │       │
│            cmd (low 16) | flags (high 16)│       │
│            result (value or -errno)      │       │
│            packet address ───────────────┼───────┘  (slot n & (capacity-1))
└──────────────────────────────────────────┘
```

- **Single producer, single consumer.** The producer writes only the tail and its payloads, and the consumer writes only the head, so no lock is needed. Positions only grow and are taken modulo the capacity, so `tail - head` is how many messages wait.
- **One payload slot per message.** A slot is free exactly when its message is consumed, so no second allocator is needed. A push fails for exactly two reasons: `EAGAIN` when the ring is full (the producer waits, and nothing is dropped) and `EMSGSIZE` when the packet is larger than a slot, whatever the ring's state. Anything larger than a page crosses by address: the kernel hands out a heap block and the shell fills it.
- **Descriptors apart from payloads,** as virtio's descriptor table and AF_XDP's UMEM are: the descriptors are kilobytes and the payloads megabytes.
- **Fixed addresses.** `build.rs` places the rings between the stack and the linker's data and emits `--global-base` from the same numbers, so the linker cannot place `.data` over a ring.

A message's packet is an XTP tree that the kernel reads in place. A **command** is a number, `major << 8 | minor` (`xpute ipc/cmd.rs`), and the numbers are `spec/cmd.json`'s. On the completion ring, `RES` marks a reply to a submission that asked for one (`ACKREQ`) or that failed. Everything else the kernel posts is a **signal** it raised itself: a fetch to start or cancel, a load, a sensor watch, a console line, a new snapshot.

The read half of the channel is a **change log** (`xpute ipc/change.rs`). A source bumps `(topic, key)`, and the topic's tick becomes the key's revision, written to a revision table and one fixed journal ring. A reader compares a revision or walks the journal from its cursor. A cursor that the journal has overwritten re-reads its topics. A change carries no value: the value is read where it lives. The world's log (`pxl-kernel world/change.rs`) has six topics: object column, decor column, object cell, terrain page, POI page and decor asset. Listeners (`ipc/change_waker.rs`) run when the tick pumps, never inside the write.

`xpute ipc/frame.rs` and `ipc/port.rs` are the same command shape for a transferred buffer rather than shared memory: what a worker would use. No worker exists yet.

## Memory

The client's state is one `WebAssembly.Memory`: 1 GiB on the desktop and 512 MiB on a phone, `initial == maximum`, never grown. Growing would detach every view over it. Because the memory never moves, a view over it stays valid for the module's life, which is what lets a ring hold its words and a lane be read where it lies. A reservation is committed lazily, but JavaScriptCore reports all of it as the page's JavaScript memory, which is why a phone gets half.

```text
address        bytes                    range                  placed by
──────────────────────────────────────────────────────────────────────────────────────
0              1 MiB                    .stack                 rustc (-z stack-size, --stack-first)
1 MiB          36 KiB × 3               .ring.desc             build.rs
2 MiB          8 MiB × 3                .ring.slot             build.rs
32 MiB         ~0.5 MiB                 .rodata .data .bss     wasm-ld (--global-base)
               ↑ __data_end, rounded up to a page
~32.5 MiB      196.43 MiB               .fixed.terrain         the kernel (sys/mem/fixed.rs)
~229 MiB       283 MiB  (phone)         .heap                  the allocator
               795 MiB  (desktop)
512 MiB / 1 GiB                         end
──────────────────────────────────────────────────────────────────────────────────────
```

The layout is `pxl-kernel sys/mem/section.rs`'s, over `xpute sys/mem.rs`, which lays ranges end to end the way a linker script does.

- **The stack comes first.** An overflow wraps below zero and lands outside the memory, which traps. There is no MMU and so no guard page, so the wraparound is the whole protection. The kernel asserts at boot that the linker's `__stack_high` is where `mem.json` says.
- **`.fixed`** is what the kernel places once, never frees, and sizes at compile time, laid from the first page past `__data_end`. It is not `.bss`: a 196 MiB static would land in the same place, but the size is a decision about how much world is resident and should not read as compiler output. Its one range today is the resident terrain (`layer/terrain/fixed.rs`). The terrain is laid out as one ground plane per channel and an elevation plane, addressed by a page's own L3 coordinate modulo the axis, like a clipmap. The axis is `2 × KEEP_PAGES + 1` = 7 pages, so 49 records. A page arriving overwrites one the view has already left, so eviction needs no list and no quarantine, and a page cannot be refused.
- **`.heap`** is everything left, served by one allocator: a slab of size classes (`xpute ds/slab.rs`) over a buddy of 4 KiB pages (`ds/buddy_tree.rs`, evanw/buddy-malloc over a given range). A request under a page takes a slot of its class. A class with no free slot borrows a run of pages and gives it back when its last slot is freed. A request of a page or more goes straight to the buddy. The bookkeeping lives in the free blocks themselves, SLUB-style, so no descriptor array grows with the range. A `String`, a band's voxel lanes, a chunk's mesh and a fetch body all come from and return to this one range. **A refused request is a wait, not a failure:** the build keeps its place and asks again after something is released, and the LOD ladder's pressure makes the view coarser.
- **One file for the numbers.** `spec/kernel/sys/mem/mem.json` holds the page, the stack, the ring addresses, `--global-base`, the phone and desktop reservations and the heap a phone is guaranteed (`MIN_HEAP_BYTES`). The kernel's map, `build.rs` and the shell's reservation all read it. Whether the front leaves that guarantee is asserted at compile time, against the lowest the heap could begin, and again at boot, against where it did.

Everything the canvas reads each frame lives here as **lanes**, structure-of-arrays views: terrain planes, voxel bands, column run sets, tier meshes, the chunk table, decor instance matrices, the object store's records and index, and every registry (`IndexedMap`, `PageTable`, handle tables). A table whose columns move together is one `Lanes` (`xpute ds/lanes.rs`). A slot is dense, and its handle is told when it moves. A lane set has one owner, and a consumer borrows it for one call. What stays in the JavaScript heap is glue: React, the WebGPU objects behind the kernel's handles, and response bodies until the kernel reads them.

---

# Engine

## Coordinate System — PVF3

PVF3 (Packed Vector Fixed-point 3D) is a 96-bit fixed-point coordinate:

```text
PVF3 = { i: IVec (64-bit), f: FVec (32-bit) }
```

- **IVec** packs three signed integer lanes in a `24/24/16` X/Y/Z layout. X is cyclic so the canonical world wraps at the longitude seam.
- **FVec** packs three unsigned 8-bit fractional residues, giving `1/256` resolution within one canonical unit, the same lattice the render payload below uses.
- Arithmetic is integer and deterministic across platforms: 64-bit words throughout. There is one implementation — the shell reads a coordinate through the kernel rather than computing one.
- The bit fields contain no guard lanes: IVec uses its full signed `24/24/16` ranges, while FVec uses all 8 bits per residue lane (the top byte of its 32-bit word remains unused).
- The EPSG:3857 adapter assigns physical meaning: currently `1u = 1 L1 block ≈ 2.389` planar Mercator meters at the equator. Geographic distortion and page-constant vertical correction live in the adapter, not in PVF3 itself.

Placed-object render payloads (`SUB_CELL_SCALE` in `crates/pxl/web/render/src/mesh/volume.rs`) quantize their sub-cell offset and size to the same `1/256` of an L1 block, so a placed position round-trips between the coordinate and the mesh without rounding. A page-local `f32`, which is how the object store saves a position, is exact on that lattice too.

## Spatial Hierarchy

The short level names are deliberate: the same `BASE`, `CENTER`, `LIMIT`, and `IDX` operations stay algebraically symmetric. Each level names a spatial extent; storage and transport roles reuse that extent instead of introducing aliases with identical indexing.

| Level | Semantic name | Extent (per axis)     | Responsibility                                                  |
| ----- | ------------- | --------------------- | --------------------------------------------------------------- |
| L0    | Sub-block     | Fractional, within L1 | Fine object placement; PVF3 residue / packed sub-cell payload   |
| L1    | Block         | `1` block (`1u`)      | Canonical cell (address unit; a Voxel once it carries material) |
| L2    | Chunk         | `64` blocks           | Meshing, LOD, raycasting, and frustum-culling band              |
| L3    | Page          | `1024` blocks         | Terrain page streaming/storage unit                             |
| L4    | Anchor        | `4096` blocks         | Floating-origin frame                                           |
| L5    | Shard         | `65536` blocks        | Global sharding domain (reserved, not yet implemented)          |

Extent is a linear range on one packed lane (`RANGE` in `crates/pxl/spec/src/space/indexer.rs`), not an inherent cube — L0–L5 are a pure scale/addressing tier, dimension-agnostic by construction. Whether a level ends up populated on one axis, two axes (a Tile), or all three (a Volume) is a shape-axis decision made by whichever layer uses that level, never a property of the level itself; see [Layer, space, and shape](#layer-space-and-shape) below. `N³`/`N²` notation elsewhere in this document describes a specific layer's _usage_ of a level (e.g. the renderer treating L2 as a `64³` Volume), not the level's own definition.

L5 is reserved, not active code today (`L5_SHAMT` stays commented out in `crates/pxl/spec/src/space/indexer.rs`). Once implemented, its Z lane collapses — Z is only 16 bits wide, narrower than `L5_SHAMT` — so L5 can only ever address X/Y, regardless of what any layer does with it.

Objects have no server persistence yet: the store is client-local. Its records (`crates/pxl/web/kernel/src/layer/object/store.rs`) are a palette per L2 chunk and one sparse octree per occupied `64³` band, whose leaves are palette indices. The chunk's entries are derived from those as lanes, with a cell index over them for occupancy and hit tests. The store also keeps the leaf ids, the change-log bumps and the eraser's virtual edits. It persists to `localStorage` as five lanes per L3 page (a page-local position, the footprint's side and the value) under a storage key that carries its layout's version.

One chunk node (`crates/pxl/web/kernel/src/world/chunk/node.rs`) owns one L2 XY column and composes however many signed 64-cell L2 Z bands its content touches. One L3 page therefore contains 256 possible L2 columns.

### Layer, space, and shape

Three independent axes describe any piece of world data, and none of them implies the others:

- **Layer** — which domain owns the data: `terrain` or `object`. See [Unified World-Layer Pipeline](#unified-world-layer-pipeline).
- **Space** — which scale tier addresses it: the L0–L5 hierarchy above. Purely a size/addressing concept; a level's own name (Page, Chunk, Anchor, ...) says nothing about what shape of data lives at that tier.
- **Shape** — how the data itself is structured: a **Tile** is a 2D/2.5D raster (terrain's own storage form, currently sized at L3/Page granularity); a **Volume** is a full 3D structure (the object octree's storage form, and the renderer's own temporary composited field at L2/Chunk granularity). A **Cell** is just a position/address — no size or dimensionality of its own, no payload; any space level's `addr`/`idx` is a Cell. A **Pixel** is a Cell plus its 2D payload — the atomic sample inside a Tile. A **Voxel** is a Cell plus its 3D material payload — the atomic sample inside a Volume. A **Band** is one materialized `64³` Volume: the renderer's concrete in-memory instance holding a single occupied Z-band of an L2 column (see [Temporary dense bands](#temporary-dense-bands)) — renderer-only working storage, not a hierarchy level or a persisted format.

Terrain today is Tile-shaped data addressed at Page granularity; objects are Volume-shaped (sparse tree) data kept per Chunk band, with the renderer composing an L2/Chunk-sized Volume as their shared transport/render envelope (see [Temporary dense bands](#temporary-dense-bands)).

| Layer                               | Space level           | Shape  | Concrete type                                   | Atomic unit |
| ----------------------------------- | --------------------- | ------ | ----------------------------------------------- | ----------- |
| terrain (source)                    | L3 Page               | Tile   | a `.fixed.terrain` record (`layer/terrain/`)    | Pixel       |
| object (source)                     | L2 Chunk / `64³` Band | Volume | palette per chunk, sparse tree per band         | Voxel       |
| terrain+object (composed for build) | L2 Chunk              | Volume | a volume of bands (`pxl-render mesh/volume.rs`) | Voxel       |
| — (any)                             | L4 Anchor             | none   | the view's root frame (`world/frame/`)          | —           |

## Terrain Data and Streaming

L3 pages are the terrain's unit of transfer and storage. The client keeps terrain in its source-appropriate 2.5D form: dense categorical ground rasters plus a padded elevation raster reconstructed with bicubic sampling.

One radius decides which pages: every page within `KEEP_DIST` of the range anchor (`spec/kernel/layer/terrain/config.json`). That one set is what gets fetched, what stays decoded, and what `.fixed.terrain` has a record for, so a page can never be requested and then dropped from under its own request. The range anchor is the view centre on the ground, snapped to a chunk, not the camera, so rotating the view does not sweep pages in and out. A fetched page is copied into its record, and the body is released in the same command.

Each page carries four ground channels rasterized from Overture Maps geometry (Buildings/Transportation/Base, plus coastline derived from Overture's Base `water` theme):

| Channel     | Contents                                            |
| ----------- | --------------------------------------------------- |
| `LAND`      | Ground surface type (bare, green, sand, rock, etc.) |
| `WATER`     | Inland water body type (river, lake, wetland, etc.) |
| `STRUCTURE` | Buildings and constructed surfaces                  |
| `TRANSPORT` | Roads, paths, railways                              |

Sea is a `LAND` value with the high bit set (`sea = 129`, `spec/cell/cube_type.json`), partitioned by that bit so a channel stays one byte a cell.

## Bake Pipeline

```text
Overture PostGIS rows
  → XTP decode
  → overture_adapter / coast_adapter (row → feature)
  → normalizer (classify, rank, assign tags — policy-parameterized: overture_policy / coast_policy)
  → ground_baker (rasterize geometry into ground channels)
  → encode_terrain (elevation + ground → XTP)
  → compressed HTTP page response
```

The bake is Rust (`crates/pxl/server/geo-bake/`), over `pxl-spec`'s formats, ported from the TypeScript it replaced and compared against it page by page before that was deleted. Polygon rasterization uses even-odd fill with hole support. Linear features use a thickness kernel. Scratch geometry comes from an arena rather than one allocation per feature — it was what kept the TypeScript's collector out of the inner loop, and in Rust it is still one bump per page against a few hundred thousand frees. Elevation is stored as signed 16-bit cell units, using one page-constant Mercator Z scale so the bake, the terrain and the objects do not drift by applying different pointwise scale rules.

## Chunk Pipeline

Terrain and objects keep different source storage:

- **Terrain** is dense and varied, a Tile-shaped layer, so L3 pages keep dense 2D channels and elevation.
- **Objects** are sparse and fully 3D, a Volume-shaped layer, so each occupied `64³` band of an L2 chunk owns a sparse tree of palette indices. A value of `0` is VOID, meaning absence or fallback. `128` is explicit AIR, which keeps the leaf and erases whatever a lower layer would show. `255` is UNKNOWN. Every other value is a placed cube colour, a terrain-styled pick or a decor kind, and that value is also its id in the one global palette (`crates/pxl/spec/src/cell/`).

They meet only when a chunk is built:

```text
terrain record ──▶ terrain paint (mesh/paint/terrain) ──┐
                                                        ├─▶ volume: signed-Z 64³ bands
object store ────▶ object paint  (mesh/paint/object)  ──┘        │
                                                                 ▼
terrain record ──▶ direct runs, one a column ────────────▶ column run set (mesh/voxel_scan)
  (no object in the chunk)                                       │
                                                  ┌──────────────┴──────────────┐
                                                  ▼                             ▼
                                        mesher (mesh/chunk_mesher)       raycast (world/chunk/raycast)
                                        a mesh per LOD tier               the same column walk
```

Objects paint after terrain, so a placed value replaces the terrain's at the same voxel. What a chunk was built from is a tuple of revisions: the terrain page's, and the column's object and decor revisions from the change log. A chunk whose tuple has not moved is not rebuilt. Because the mesher and the raycast read the same runs, rendered and clickable geometry cannot drift apart.

A chunk with nothing placed in it skips the volume altogether: the terrain writes one run a column straight into the run set (the direct path). Objects are painted a band's worth of voxels at a time, and a store written between two steps restarts the paint.

### Temporary dense bands

A composed chunk is a set of sorted, signed 64-cell Z bands (`crates/pxl/web/render/src/mesh/volume.rs`). A band is allocated whole even when one peak touches it; sparsity exists only between bands. Within a band the lanes are flat:

- `material`, u32: material class and RGB (`0xCC_RRGGBB`);
- `layer_idx`, u8: the texture-array layer;
- a sub-cell payload only for bands holding fine objects, and an overflow list for objects sharing a base voxel.

The common payload is five bytes a voxel, so one `64³` band is about 1.25 MiB. Keeping one for every visible chunk would exceed a phone's budget, so bands exist only while a chunk is composed and scanned. Up to 16 band stores keep their lanes for the next composite, because building them fresh was measured as the heap's worst churn.

What survives is the chunk's **column run set**: every one of its `64²` columns' merged material runs, packed as lanes in one heap block (22 bytes a run) rather than as a view into the bands. It is scanned once and reused by every LOD tier and every tier transition until the chunk leaves the retained set or its sources change. Each LOD tier a chunk has built is kept as one heap block holding the mesh's lanes, under a retain budget of half the heap for what is off screen (`RETAIN_BUDGET_BYTES`), because rebuilding a tier from the runs is not free either.

The raycast needs exact composed voxels, but only near the pointer, so it keeps its own composites: at most four chunks and eight bands (about 10 MiB). A chunk taller than that is used once and not cached.

### The chunk scheduler

Every decision about a chunk is `crates/pxl/web/kernel/src/world/chunk/table.rs`'s, one slot a chunk (flags, distances, bounding sphere, pending-since, retained bytes); the nodes (`node.rs`) hold what the decisions act on. A tick is four passes: **mount** decides which chunks exist, by range from the anchor, with hysteresis so a chunk at the edge is not traded back and forth; **refresh** measures each against the frustum; **recheck** asks the LOD ladder which tiers distance wants and marks the slot pending; **commit** applies a layout whose tiers are all in hand, or releases one that has gone off screen.

What to build next is one priority: slot holders first, then chunks in the frustum by distance from the **lead point** — where the camera is going rather than where it is drawn, so a pan fills in ahead of the view (`crates/pxl/web/kernel/src/sched/priority.rs`) — with ageing, so a chunk the camera keeps almost reaching cannot wait forever. Builds run in the build task on what the quota leaves after the tick, one scan and one bake open at a time, a step at a time.

**Memory is paid for in LOD.** A block the heap refuses ends the tick's builds, the chunk gives up its slot and is not tried again until the heap releases something, and reclaim gives back off-screen work farthest first. When a refusal finds nothing to reclaim, the ladder steps in one tier (at most every half second, up to three), and steps back out after five seconds without one — the view gets coarser rather than the page dying.

## Rendering and LOD

The world is drawn from a draw list (`crates/pxl/web/render/src/frame/draw.rs`). Each tick, the scheduler lists what to upload, free and draw: the chunks' mesh blocks at the weights its blend decided, decor as instanced draws of a group's model over the block a chunk shows, the actors with their joint palettes, the puck, the billboards and the dev gizmos. The renderer (`crates/pxl/web/render/src/gpu/renderer.rs` and the passes in `gpu/pass/`) executes the list with no decision of its own.

The renderer reaches WebGPU through the host's calls (`gpu/api.rs`, answered by `apps/web/src/device/gpu.ts`). Every call passes numbers and the address and length of what it names, and every GPU object is named by a handle the kernel gave it. The shaders and pipelines are the kernel's too (`crates/pxl/web/render/src/frame/shader/`). Each pipeline is a record of words (its shaders, its vertex lanes, its bind group layouts, its primitive, depth and blend state) that the host turns into a WebGPU descriptor, and that a native host could turn into wgpu's. `cargo test` compiles every shader with naga and holds each record to what its shader declares. Depth is reversed float throughout, with near at 1 and far at 0, so a float's precision falls where a perspective depth needs it and far coplanar terrain faces do not fight. When the device is lost, the blocks stay in memory and are put back on the next device from their addresses.

The view (`crates/pxl/web/kernel/src/world/frame/view.rs`) is the orbit camera and the root frame the world stands in. The eye never turns: the world turns under it, by the map's azimuth, and is translated to put the map centre at the origin. The local origin is the map centre's L4 base, so the world stays in `f32` range near the camera, and the translation rebases in the same step that it snaps. The matrices are doubles, composed the moment a damping step writes them, and read by the chunk scheduler, the renderer, placement and the pointer's ray.

A drawn frame is handed over whole. After the tick, the kernel writes the frame's uniforms into the stage lane (`pxl-render frame/stage.rs`), then calls the renderer with a `FrameInput`: the draw list, the environment, the models and skins as they stand, the billboard sheet and the surfaces. The renderer returns what the frame cost and whether another turn is wanted. It draws into every open surface: the world's canvas and the character picker's case.

LOD samples the 64² chunk at strides `1/2/4/8/16/32/64`. A tier hands off to the next when one of its cells would project to about six pixels — screen-space error, derived from the viewport height and field of view (`crates/pxl/web/render/src/look/lod.rs`), which at the reference viewport puts the handoffs at `192/384/768/1536/3072/6144` world units, proportional to the stride so projected coarse-cell size changes smoothly. Everything that keeps step with the tiers — the POI badge lattice, the badge fade, decor's range — is a multiple of that first handoff. Decor is drawn by the same criterion applied to a kind's extent: a thing that would cover fewer pixels than two of the tier's cells is not drawn, at full density up to that tier and not past it (`crates/pxl/web/kernel/src/layer/decor/build.rs`).

The visible handoffs dither between adjacent LOD meshes with complementary screen-space masks instead of alpha-overlapping them. A newly visible chunk starts from its distance-derived tier instead of flashing through the coarsest one.

Frustum culling uses the exact union of the terrain, object, primary and secondary mesh bounds. Measured bounds survive GPU eviction and are invalidated with their source revisions. Until a chunk has measured geometry, a conservative guard keeps a low camera from culling the ground directly beneath the view.

### Coarse LOD

A coarse cell (`lod_step > 1`, `crates/pxl/web/render/src/mesh/chunk_mesher.rs`)
merges `lod_step³` voxels into one box, and says three things about them.

- **How much is there.** Fill is coverage over `lod_step³` — one scalar, no
  axis singled out — and the box shrinks by its cube root about the cell's
  own centre, so its volume is the fill fraction of the cell's. There is no
  occupancy threshold: seven VOID and one red block is one quarter-size red
  cube, never a dropped one.
- **What it looks like.** Colour is a _surface_ average: only the topmost run
  of each sub-column contributes, because that is the voxel visible from
  above. Runs arrive ascending, so a later one supersedes the one before it
  by subtracting what that column last contributed, keeping the pass O(1)
  per run.
- **What it is made of.** Up to four texture-array layers blend per cell, so
  a coarse cell spanning grass, road and roof is not whichever it met first.

Sub-cell (fractional, precisely placed) runs contribute their true
volumetric fraction. Full cells stay ordinary runs and keep greedy merging
and face culling; only shrunken ones take the inset path, where they get all
four creases because nothing touches their walls.

Density-as-size costs something and the cost is accepted: a surface has low
volumetric fill in every cell it crosses, so flat ground shrinks too and
distant terrain reads as separated cubes rather than a sheet. The
alternatives — dimming a full-size box by fill, or fitting a box per axis to
the voxels present — each make XY the axis the renderer is built around with
Z as the exception, and orientation then decides existence: thirty-two
voxels laid flat drew, the same thirty-two standing up vanished.

### Face culling and greedy merging

At every tier, a run whose faces could never be seen emits nothing: not the
topmost run in its own column, and all four horizontal neighbours solid
across its exact `z0..z1` span. Chunk-edge columns are exempt, since a
neighbouring chunk's data is not available to check and assuming it solid
would risk a visible hole.

Surviving runs merge in the run domain rather than the face domain: adjacent
columns holding the same run — identical z span, identical material — become
one box several cells wide, so a flat plateau emits one box instead of one
per cell. A merge consumes the border between two cells, so it may only
cross a border that carries nothing to draw or shade, and each pass tests
the border it is about to swallow rather than the one the box started at.

## Camera

The orbit camera is driven by three controls — `dist`, `polar`, `azimuth` —
and nothing else writes them. Terrain is allowed one question, in
`crates/pxl/web/kernel/src/control/camera/terrain_gate.rs`: would this view state put the eye inside
it. The gate is a pure function of the three controls rather than of the
live scene graph, and it sits in `set_view_state`, the one funnel every
input path goes through, so a refused move never leaves the controls and the
drawn camera disagreeing.

`Settings > Camera` picks what the answer does: `through` ignores terrain,
`stop` takes a control as far as it asked or as far as it can, and `climb`
grants every control and raises the _drawn_ pitch instead, leaving
`view_state` untouched so the pitch relaxes back on its own.

Every setting is one entry of one schema (`spec/kernel/shell/setting/schema.json`): its
default, its kind and its section. The store reads the defaults with the
user's overrides on top and stores only the overrides, and the settings
sheet renders its rows from the schema. Dev-only settings — the debug
gizmos, the ledger panel, a render-path bisect — are entries marked `dev`.

## Billboards and POI

**POI is disconnected** until its per-page index is rewritten. What follows is the design the code holds.

A POI page is an XTP record the bake emits — positions, category, rank and
id hash as lanes, the strings as leaves read one at a time — copied into
the core memory on arrival. A spatial index over the resident disc
(`crates/pxl/web/kernel/src/layer/poi/index.rs`) is rebuilt a range of records per unit
in the content phase while the previous build keeps answering; placement
(`crates/pxl/web/kernel/src/layer/poi/placement.rs`) is a job of the same phase — query, file the hits into an
implicit lattice by tier, offer them with estimated widths, greedy
placement on a screen grid in distance order, build the badges — committed
whole. Badges are drawn from one atlas (`crates/pxl/web/render/src/asset/atlas.rs`,
shelf-packed, uploaded by rectangle) as **one instanced quad**
(`crates/pxl/web/kernel/src/layer/billboard/field.rs`): anchor, size and sheet rectangle are per-instance
attributes, and the fit to screen pixels, the distance fade and the anchor
pinning are the vertex shader's.

Occlusion is all or nothing per badge, on purpose: the badge pass
attaches the world's depth read-only and binds it as a texture, and its shader
(`crates/pxl/web/render/src/frame/shader/badge.wgsl`) reprojects several points up each badge
through it and discards the badge when most are behind terrain.
The same pass writes `gl_FragDepth` from an upright twin of the quad, so
billboards sort against the world along world-up rather than camera-up.
Chat bubbles still use the sprite form of the same patch.

## XTP — Xpute Tree Packet

XTP is the binary wire format the bake writes, the bucket holds and the kernel reads. It is also what the rings carry:

- **Tree-shaped**: scalar leaves, typed-array sequences, and branch nodes;
- **C-layout-compatible**: 8-byte word alignment, little-endian, relative offsets;
- **Relocatable**: packets can be moved or spliced without rewriting absolute pointers;
- **Zero-copy reads**: typed-array sequences are views into the packet buffer;
- **GRAFT**: a pre-encoded subtree packet splices with the same wire layout as an inline node;
- **Type-preserving nulls**: optional nodes retain type metadata while physically absent.

Every packet on the rings is XTP too. TLV is used separately for flat, self-delimiting stream arguments. XTP lives in xpute (`crates/xpute/core/src/io/marshal/tree/`, `@xpute/core/io/marshal/tree/`), not in the model, because the codec contains no pixelet domain knowledge.

## Catalogs and assets

What a build ships is decided at build time and written once as JSON under `spec/`: pxl-spec's at the top, and `kernel/`, `render/` and `xpute/` below, each mirroring the crate path it generates into. `script/spec.ts` generates a Rust module from each file (through rustfmt) and, where the shell reads it, a TypeScript one (through `deno fmt`). `deno task spec` writes them, and `deno task check:spec` fails when what is on disk is not what the JSON writes. The generated files include the command numbers, the cell catalogs, the memory map's numbers, the GPU API's enums, the ledger's layout, the settings schema, and the decor and character catalogs.

Nothing in a catalog is sent between shell and kernel or between client and server. Both sides compile the same catalog, so a message carrying it would be a program telling itself what it already has. Where bytes do cross, as a published bake or a saved blob, their layout is the model's and their version is in the envelope: the path or the storage key.

- **Cubes and decor kinds** are `spec/cell/`'s (`cube_type.json`, `color.json`, `decor.json`); a value's number is what the object store writes.
- **Models** are named by their path under the top-level `asset/` tree, in the catalog that uses them: each decor kind's `models` and `skins` in `spec/kernel/layer/decor/kind/kind.json`, each character group's `keys`, `models`, `skin` and `size` in `spec/kernel/layer/character/variant.json`. A path resolves to a bucket key and a measured extent through the asset table (`crates/pxl/spec/src/asset/ingested.rs`, `apps/web/src/catalog/asset/ingested_spec.ts`) that `asset/script/ingest_asset.rs` generates, which hashes every model, skin and texture into the bucket (`asset/<sha256>`) and measures each glb's bounds. Nothing imports a model or an image: the client fetches a key, the same key from the dev API's `./out/asset` and from R2.
- **The texture pack** is one table (`crates/pxl/spec/src/asset/texture.rs`, which image a cube type wears on which face) and one object in the bucket: the ingest composes its images into a single WebP sheet, and the client fills its layer array from that one download, a layer at a time. A cube's colour is `spec/cell/color.json`'s, what the minimap and the coarse tiers draw, and a pack decorates it.
