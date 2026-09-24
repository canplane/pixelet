# Architecture — pixelet

[pixelet](./README.md) turns geographic data and player-built blocks into one voxel world and draws it in a browser. It is an application written over [xpute](./ARCHITECTURE.xpute.md): a Rust kernel that runs as xpute's guest in one WebAssembly module, a TypeScript shell that is xpute's host, and Rust servers and bakes that share the kernel's model. This document is what pixelet builds; what it builds on — the memory, the rings, the turn, the credits — is the other one.

Most of what follows is shaped by where it has to run: a phone, in a browser tab, with a frame to fill. That is why the reservation is halved there, why a chunk's working storage is torn down as soon as it is scanned, and why detail is the first thing spent when memory runs short.

Three properties hold everywhere. The first two are xpute's and everything here lives inside them; the third is pixelet's own.

- The world's state lives in **one linear memory**, fixed at boot.
- All work runs in **turns under one quota**.
- What the client measures about itself goes to a **read-only ledger**. No decision reads it.

This document is in three parts. **The world** is the vocabulary: what a position is, what each level addresses, and what arrives at one. **The client** is the machine that holds it — the crates, the two sides of the browser tab, and how a turn is spent. **The engine** is what is computed with it, from a chunk being built to a frame being drawn.

---

# The world

## Spatial Hierarchy

| Level | Name      | Extent per axis | Used for                                  |
| ----- | --------- | --------------- | ----------------------------------------- |
| L0    | Sub-block | within one L1   | fine placement, below one block           |
| L1    | Block     | `1u`            | the unit a position names                 |
| L2    | Chunk     | `64` blocks     | meshing, LOD, raycasting, frustum culling |
| L3    | Page      | `512` blocks    | terrain streaming and storage             |
| L4    | Anchor    | `4096` blocks   | the floating origin                       |
| L5    | Shard     | `65536` blocks  | reserved                                  |

L2 is Minecraft's chunk under another name — the unit a world is meshed in, cached at, and rebuilt when edited — at 64 blocks rather than 16.

L5 is commented out (`L5_SHAMT`); when it exists it will address X and Y only, since the Z field of a packed position is 16 bits.

An extent is all the table gives. What is stored at a level is a separate question, and the answer is not always a cube: a level is an extent on one axis, not on three. `BASE`, `CENTER`, `LIMIT` and `IDX` (`pxl-spec space/indexer.rs`) are the same operations at every level, and whether a layer fills one axis, two or three is that layer's own choice.

The words for what sits at a level build on one another. A **Cell** is an address and nothing else — one L1 block, named and possibly empty. A **Pixel** is a cell carrying a 2D sample; a **Voxel** is a cell carrying a material. A **Tile** is a level filled on two axes — terrain at L3, a raster of pixels with height, which is why it is 2.5D and not 3D. A **Volume** is a level filled on all three, which is the shape a chunk's objects take and the shape a chunk is built into.

One more word belongs to the renderer rather than to the model. A **Band** is one `64³` volume materialized as voxels, the working storage a chunk is composed into. It is never a level and never persisted.

| Layer                       | Level              | Shape  | Concrete type                                   |
| --------------------------- | ------------------ | ------ | ----------------------------------------------- |
| terrain, source             | L3 Page            | Tile   | a `.fixed.terrain` record                       |
| object, source              | L2 Chunk, per band | Volume | palette per chunk, sparse tree per band         |
| terrain + object, for build | L2 Chunk           | Volume | a volume of bands (`pxl-render mesh/volume.rs`) |
| any                         | L4 Anchor          | —      | the view's frame of reference                   |

Objects are client-local. The store (`pxl-kernel layer/object/store.rs`) keeps a palette per chunk and a sparse octree per occupied band, leaves being palette indices, and derives the chunk's entries as columns with a cell index for occupancy and hits. It persists to `localStorage` as five columns per page — page-local `x`, `y` and `z`, footprint side, value — under a key that carries the layout's version. One chunk node (`world/chunk/node.rs`) owns one L2 column and composes as many signed 64-cell Z bands as its content touches; a page holds 64 columns.

## Coordinate System — PVF3

A position inside that hierarchy is 96 bits of fixed point, in two words: which block, and where inside that block.

```text
IVec, one 64-bit word                 FVec, one 32-bit word
┌───────────┬───────────┬────────┐    ┌──────┬──────┬──────┬──────┐
│     X     │     Y     │   Z    │    │      │  X   │  Y   │  Z   │
│  24 bits  │  24 bits  │   16   │    │unused│  8   │  8   │  8   │
│   63..40  │   39..16  │  15..0 │    │31..24│23..16│ 15..8│  7..0│
└───────────┴───────────┴────────┘    └──────┴──────┴──────┴──────┘
signed; X is cyclic, so the world     unsigned residues:
wraps at the longitude seam           1/256 of a block on each axis
```

Each field is a **lane** of its word. The arithmetic is integer — a lane is computed as `i64` and packed back — so a coordinate is the same bits on every machine. There is one implementation of the packing, and it stays one because the shell reads coordinates through the kernel rather than decoding them itself.

The Web Mercator adapter (EPSG:3857) gives the unit its meaning: `1u` is one L1 block, about 2.389 Mercator meters at the equator. Distortion and the page-constant vertical scale live in the adapter. Placed objects quantize their sub-cell offset and size to the same `1/256` (`SUB_CELL_SCALE`, `pxl-render mesh/volume.rs`), so a position round-trips through the mesh without rounding, and the `f32` the store saves is exact on that lattice.

## Terrain

Terrain arrives as L3 pages: four rasters of ground type, and one of elevation, padded so that bicubic sampling can reach past a page's edge.

| Channel     | Contents                                  |
| ----------- | ----------------------------------------- |
| `LAND`      | ground surface: bare, green, sand, rock … |
| `WATER`     | inland water: river, lake, wetland …      |
| `TRANSPORT` | roads, paths, railways                    |
| `STRUCTURE` | buildings and constructed surfaces        |

The rows are in the one order the channels have (`GROUND_CH_KEYS`, `pxl-spec bake/format.rs`): the order a page stores them in and the order a cell is painted in, a later channel's non-void type winning, so a building that overlaps a road keeps its cells.

Sea is a `LAND` value with the high bit set (`sea = 129`, `spec/pxl/spec/cell/cube_type.json`), so a channel stays one byte per cell. The channels are rasterized from Overture's Buildings, Transportation and Base themes, the coastline from Base's `water`.

Which pages are held is decided from one point and the page pyramid. The point is the **range anchor** (`world/chunk/range.rs`): the view centre on the ground, snapped to a chunk — an anchor in the sense of a fixed point, not the L4 level of the same name. It is not the camera, so rotating does not sweep pages in and out.

What a screen shows spans a few octaves of distance, and every octave costs the same, so the client holds a fixed number of pages a level rather than a radius (`pxl-kernel world/pyramid/window.rs`, `spec/pxl/web/kernel/world/pyramid/window.json`). The window is the pyramid's, not the ground's: every layer on the pyramid is held in the same one. At every level of a **window** — the levels the ground in view wants, from the nearest ground's to the farthest's, at most five, each one level finer than the step it is drawn at — it holds a 5 × 5 ring of that level's pages around the anchor's page, and draws from the inner 3 × 3 — the middle the ring below covers too, since it is what is drawn where the finer page has not come. Looking down at a street, the window is the leaves alone. It follows the camera rather than growing, so memory is constant in zoom. The ring decides all three questions at once: its pages are the pages fetched, the pages kept decoded, and the pages the resident terrain range has records for, so a page cannot be requested and then dropped from under its own request. Coarse pages are fetched first — a coarse page is the fallback a fine one refines — nearest first within a level; a page the ring leaves is let go once the page over it or the four under it are resident; and a page is copied into its record and its body released in the same command.

## Bake

The bake is `pxl-geo-bake`, and it writes the same formats the client reads — XTP, xpute's tree, read where it lands rather than parsed — so nothing is translated on arrival.

```text
Overture PostGIS rows
  → XTP decode
  → overture/adapter, coast/adapter (row → feature)
  → normalizer (classify, rank, tag — the rules live in overture/policy, coast/policy)
  → ground (rasterize geometry into ground channels)
  → encode_terrain (elevation + ground → XTP)
  → compressed HTTP page response
```

Rasterizing is the bulk of it: polygons are filled even-odd with holes, lines are drawn with a thickness kernel, and the scratch geometry for both comes from one arena per page. Elevation is stored as signed 16-bit cell units under one Mercator Z scale per page, which is what makes the bake, the terrain and the objects agree on what a unit of height is.

Every land page is baked once into a versioned prefix. Open ocean is one shared page, since every one of them would be identical.

Those pages are the leaves, level 0. A view from farther away wants fewer, coarser pages, so the bake stacks levels on them: a level-k page covers the 2 × 2 level-(k − 1) pages under it at half their resolution and is the same record as a leaf, named by the Web Mercator tile it stands on so that a page's name is one digit shorter than its children's and a prefix of them. It is folded from the level below with no database in the loop — each ground channel by a majority of the four cells under it, the elevation by an average that reads past the page's edge — and the fold is `pxl-spec`'s (`bake/fold.rs`), so the client can fold the pages it already holds by the same function instead of fetching. Open water folds to itself, which is what lets the one ocean page serve every level.

---

# The client

Two things run in the tab: a kernel that decides and a shell that carries out. Where a piece of code lives follows from that, and so does which language it is written in.

## Crates and packages

One line decides a thing's language: **what shares `pxl-spec` is Rust**.

```text
                TypeScript                     Rust                              over pxl-spec, never in the browser
              ┌───────────────┐            ┌───────────────┐
generic kit   │ @xpute/core   │  mirrors   │ xpute-core    │  ┐
              └───────┬───────┘            └───────┬───────┘  │ xpute — ARCHITECTURE.xpute.md
              ┌───────▼───────┐            ┌───────▼───────┐  │
runtime       │ @xpute/runtime│ two ends   │ xpute-runtime │  ┘
              └───────┬───────┘ of one     └───────┬───────┘
                      │                    ┌───────▼───────┐            ┌──────────────┐
model                 │   spec/ ─writes─▶  │ pxl-spec      │ ◀──────────┤ pxl-geo      │  forward search
                      │     │              └───────┬───────┘            │ pxl-api      │  the app's root
                      │     │              ┌───────▼───────┐            │ pxl-geo-bake │  the three bakes
view                  │     │              │ pxl-render    │            │ ingest-asset │  asset/script
                      │     │              └───────┬───────┘            └──────┬───────┘
                      │     │              ┌───────▼───────┐            pxl-server · pxl-geo-spec
kernel                │     │              │ pxl-kernel    │            beneath them, no model
                      │     │              └───────┬───────┘
                      │     │              ┌───────▼───────┐
module                │     │              │ pxl-wasm      │
                      │     ▼              └───────┬───────┘
              ┌───────▼─────────────┐              │ kernel.wasm
app           │ apps/web  *.spec.ts │◀─────────────┘
              └─────────────────────┘
```

| Crate / package | What it is                                                                                                                                       |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `pxl-spec`      | The model: PVF3, the spatial hierarchy, the cell catalogs, the command numbers, the page formats, the asset tables. One implementation, in Rust. |
| `pxl-render`    | The view: volumes, the column scan, the mesher, glTF, the LOD ladder, the atlas, the draw list, the shaders, the renderer.                       |
| `pxl-kernel`    | The kernel: the chunk scheduler, the layers and their stores, fetches and loads, the camera and sensors, the turn, the ledger.                   |
| `pxl-wasm`      | The module: installs the allocator and the executor, fixes the memory's front with linker flags, exports `boot` and `interrupt`.                 |
| `apps/web`      | The shell: React over the kernel's snapshot, the WebGPU host, the I/O the kernel names; and the Cloudflare Worker that serves it all.            |
| `pxl-server`    | What both servers need before answering: the environment variables, the database pool, the gate over internal routes.                            |
| `pxl-geo-spec`  | What the geo server and the bakes agree on about db-geo: table names, and what Overture's categories and divisions mean.                         |
| `pxl-geo`       | The world's server. Forward search — the one route that queries a database at request time.                                                      |
| `pxl-api`       | The application's server.                                                                                                                        |
| `pxl-geo-bake`  | The three offline bakes — terrain, POI, geocode — and, behind a feature, the dev tile server.                                                    |
| `ingest-asset`  | Hashes `asset/` into the bucket's shape and writes the asset tables that `pxl-spec` and the shell carry. Its own workspace.                      |

The servers, the bakes and the asset ingest all compile the model, so they are crates. The Overture and elevation ingests move a provider's columns into Postgres columns and touch no model, so they are scripts under `infra/geo/`. The shell is TypeScript because it decides nothing about the world; what it needs of the model — catalogs, ABI enums, routes — is generated into `apps/web/src/**/*.spec.ts` from the same JSON the Rust is generated from.

Three rules hold the graph in that shape:

- **The view does not know the kernel.** `pxl-render` sees the model and a `FrameInput`, and returns what a frame cost.
- **The kernel does not know it is WebAssembly.** The host's clock, storage and text measurement arrive as hooks `pxl-wasm` installs, so the kernel runs natively under `cargo test`.
- **Nothing is found by name.** Commands, cells, decor kinds (what the world grows on its own — trees, cars, boats, trains), character variants (the people and pets a player can be), assets and messages are numbers fixed in data. The words a message is shown in belong to the shell's locale files.

## Shell and kernel

The shell is a device driver (`apps/web/src/kernel/`). It does five things:

- Reserves the memory and instantiates the module (`mem.ts`, `module.ts`).
- Rings the doorbell once per animation frame, when something asked for one.
- Forwards pointer, wheel, sensor and visibility events as commands.
- Does the I/O the kernel names — fetches, image decodes, sensor watches — and hands the bytes to blocks the kernel chose.
- Hands over canvases. A canvas is transferred to an `OffscreenCanvas` once and drawn by the kernel from then on, named by a handle like any GPU object.

The UI is React over one snapshot, a JSON document the kernel writes after a turn in which something changed. Every control is an action the kernel applies. Nothing in the shell computes a coordinate, a tile or a draw.

## The kernel's turn

The turn is xpute's, and so are the quota and the credits: the host rings once, the guest spends what it was given, and the falling edge says when to ring again ([ARCHITECTURE.xpute.md](./ARCHITECTURE.xpute.md) is the mechanism). What pixelet decides is what fills it.

```text
                           the rising edge
                                  │
                                  ▼
┌──────────────────────────────────────────────────────────────────┐
│ the frame task, in an order of its own                           │
│                                                                  │
│   effects   the fetchers, the followers a store's change wakes,  │
│             the markers — each compares the world with what it   │
│             last saw                                             │
│   tick      interaction → visible → content → cosmetic → report  │
│   pump      start the I/O this turn's credits bought             │
│   draw      the frame's draw list                                │
│                                                                  │
│ five tasks beside it, polled on what the tick left               │
│                                                                  │
│   build · fetch decode · glTF · decor · the object store's save  │
└──────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
                          the falling edge  ──▶  wake_ms
```

The host submits one `WORLD_UPDATE` before it rings — the viewport, the gap since the last turn, and how many reads this turn's credits allow to be out — and reads the completion ring after. Everything else here is the kernel's.

**The quota's policy** is the kernel's (`sched/tick.rs`, `QUANTUM`). It reaches the host in the boot block, the words `boot` writes at the front of the memory for the shell to read once.

What the policy says: the frame is the 25th percentile of recent gaps between frames, ignoring any gap over 50 ms. A turn is interactive when input arrived within `settle_ms`, or when the kernel asked for it on its last falling edge. An interactive turn gets that frame less a margin; every other turn gets `batch_frames` frames.

**The phases**, in the order that is their priority:

| Phase         | What                                                                                                                                                                 |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `interaction` | input, camera damping, the environment (sun, moon, theme), the view's frame of reference, the companion (the player's character at the map centre); O(1), unbudgeted |
| `visible`     | the chunk scheduler's pass                                                                                                                                           |
| `content`     | what is about to be on screen; guaranteed `content_share` of the budget                                                                                              |
| `cosmetic`    | what only looks better for running (debug gizmos)                                                                                                                    |
| `report`      | the ledger, last, so it sees everything                                                                                                                              |

**The tasks** run on `embassy-executor`'s raw executor, which the module installs. The frame is a task rather than a function so that the order above is its own, and no task depends on the order the executor happens to poll them in.

**The credits** bound what is out, and one place spends them (`io/sched.rs`). A producer says only what it wants — the subjects it wants read, each with a rank, replacing whatever it said last turn — and never that an attempt should start, stop or be retried. What is wanted, what is running and what has just failed are three tables, so an attempt ending answers nothing about the demand that outlived it, a failure waits rather than asking straight back, and the demand of a camera that has just moved is representable however much old work is still draining. A rank is a class and a distance; the numbers behind both are in one module (`io/policy.rs`) rather than one per layer, and a model or a texture is a read like any other, spending the same credits as a page rather than bidding against it from a second scheduler.

**The ledger** (`sched/ledger`, laid out by `spec/pxl/web/kernel/sched/ledger/ledger.json`) records what each frame and each task cost — pacing, per-phase time, GPU ask, heap gauges, LOD pressure, draw census, per-job waits — into rings the shell reads in place (`ledger_read`, `ledger_log`) and prints as a report or exports as a Chrome trace. The scheduler never reads them.

## The channel

The shell and the kernel share three rings, each 2048 messages of four words with 4 KiB payload slots. Two of them are the pair xpute describes: one for what the shell submits, one for what the kernel completes. The third is a spill, which the completion ring overflows into when it is full, so that nothing is dropped and completions stay in order.

The command numbers are `spec/pxl/spec/abi/cmd.json`'s. A packet is an XTP tree, and anything larger than a slot crosses in a heap block the kernel hands out.

The kernel also posts messages nobody asked for: a fetch to start or cancel, a model or image to decode, a sensor to watch, a console line, a new snapshot. Those are its signals.

The world's change log (`world/change.rs`) has five topics: object column, decor column, object cell, terrain page, decor asset. Listeners run when the tick pumps, never inside the write.

## The memory map

The client's state is one `WebAssembly.Memory`, 1 GiB on the desktop and 512 MiB on a phone, `initial == maximum`, never grown. Since it never moves, a ring keeps its words and a table is read where it lies for the module's whole life — the way Figma's editor core holds every object as a range inside one pre-allocated array. A phone gets half because JavaScriptCore counts the whole reservation as the tab's JavaScript memory, committed or not.

```text
address        bytes                    range                  placed by
──────────────────────────────────────────────────────────────────────────────────────
0              1 MiB                    .stack                 rustc (-z stack-size, --stack-first)
1 MiB          36 KiB × 3               .ring.desc             build.rs
2 MiB          8 MiB × 3                .ring.slot             build.rs
32 MiB         ~0.7 MiB                 .rodata .data .bss     wasm-ld (--global-base)
               ↑ __data_end, rounded up to 64 KiB
~32.7 MiB      62.81 MiB                .fixed.terrain         the kernel (mem/fixed.rs)
~95.5 MiB      1 MiB                    .fixed.bake            the kernel (mem/fixed.rs)
~96.5 MiB      1 MiB × 2                .fixed.runs            the kernel (mem/fixed.rs)
~98.5 MiB      413 MiB  (phone)         .heap                  the allocator
               925 MiB  (desktop)
512 MiB / 1 GiB                         end
──────────────────────────────────────────────────────────────────────────────────────
```

The map is `pxl-kernel mem/section.rs`'s, over xpute's sections. The stack is first so that an overflow wraps below zero and traps — there is no MMU, so the wraparound is the only guard — and the kernel asserts at boot that the linker's `__stack_high` is where `mem.json` says. The rings sit at fixed addresses and `build.rs` emits `--global-base` from the same numbers, so the linker cannot place `.data` over them.

`.fixed` is what the kernel places once, sizes at compile time and never frees. The largest of it is the resident terrain (`layer/terrain/fixed.rs`): a lane of ground, one packed word a cell, and an elevation plane past it, indexed by a page's coordinate at its level modulo the ring's axis, like a clipmap — a ring over a coordinate — within a group of records for its level modulo the window's width. Five groups of 5 × 5 are 125 records. A page that arrives overwrites the one the view has left, and a level that enters the window overwrites the one that left it, so there is no eviction list and no page is refused. Beside it are the one mesh a chunk bake has open (`.fixed.bake`, 1 MiB), sent to the device the moment it is done, and two run sets (`.fixed.runs`), one filled while the other is baked from.

`.heap` is everything else, under xpute's slab over buddy. A `String`, a band's voxels and a fetch body all come from here. A refused request is a wait, not a failure: the build keeps its place and asks again after a release, and the LOD ladder gets coarser.

The numbers — page size, stack, ring addresses, `--global-base`, the two reservations, the heap a phone is guaranteed (`MIN_HEAP_BYTES`) — are one file, `spec/pxl/web/kernel/mem/mem.json`, read by the kernel's map, by `build.rs` and by the shell's reservation, and asserted at compile time and at boot.

What the renderer reads each frame is laid out as **column lanes**, structure-of-arrays — not the packed-word lanes of a coordinate. They are terrain planes, voxel bands, column runs, a mesh per LOD tier, the chunk table, decor instances, the object store's records, and every registry. A table whose columns move together is one struct of lanes with one length. Slots are dense, and an index is told when its slot moves — except in a handle table, where the slot is what the other side holds, so it stays and is reused under a new generation. A lane set has one owner; a consumer borrows it for one call. The JavaScript heap holds only glue: React, the WebGPU objects behind the kernel's handles, and response bodies until the kernel takes them.

---

# The engine

Everything here serves one meeting. The ground arrives as a dense raster for a whole page; what the player places is a sparse tree per band. The two are stored apart and travel apart, and they meet once, when a chunk is built. What comes out of that meeting is one set of spans, one per stretch of a column that holds a single material — the **runs** the rest of this half is about. The same runs are drawn and hit, so the picture and the pointer can never disagree.

## Chunk pipeline

This is the meeting.

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

In a band, `0` is VOID, `128` is explicit AIR — a leaf that erases what a lower layer would show — and `255` is UNKNOWN. Any other value is at once a placed colour, a terrain pick or a decor kind, and its id in the one global palette (`pxl-spec cell/`).

Objects paint after terrain, so a placed value wins the voxel. A chunk records the revisions it was built from — its terrain page's, its column's object and decor revisions — and is not rebuilt while they stand.

**Bands are temporary** (`pxl-render mesh/volume.rs`). A band is `64³` voxels, five bytes each — a `u32` `material` `0xCC_RRGGBB` and a `u8` `layer_idx`, plus a sub-cell payload where fine objects need it — about 1.25 MiB, allocated whole when one peak touches it. That is too much to keep for every visible chunk on a phone, so bands live only while a chunk is composed and scanned; sixteen band stores keep their lanes between composites, because reallocating them was the heap's worst churn.

**The column run set lasts as long as its bakes.** A column is read bottom to top; a run opens at its first voxel and extends while the material holds. A chunk's `66²` columns, its own and an apron, become their runs, 22 bytes a run, in one of `.fixed.runs`'s two sets of 1 MiB; a set past that is refused rather than cut, since a run left out is a hole. Every tier the chunk has is baked from the set while it is there — a leaf's two, one above — and then the set is let go: the tiers are what lasts, so runs kept for a tier asked later would be a cache of a cache. Each tier's mesh lives on the device alone, in a slot of fixed buffers the renderer keeps a level group (`pxl-render gpu/arena.rs`): the slot is the chunk's coordinate modulo its drawn ring's 24 chunks, within a group for its level modulo five (`world/chunk/mesh_slot.rs`), the terrain records' addressing one level down. A slot is taken over only once the chunk holding it is no longer drawn, so a mesh outlives its page's record and nothing is freed. A mesh is its boxes and nothing per vertex: 36 bytes a box — its least corner and size, its colour, four texture layers, its face mask — which the chunk pipeline binds as storage and its vertex shader unfolds into a cube, a face the mask leaves out collapsing to nothing (`frame/shader/chunk.wgsl`). As vertices a box of three faces was over a kilobyte, every number in it the box's own repeated. The raycast keeps its own exact composites near the pointer — four chunks, eight bands, about 10 MiB — made from the leaf page wherever one is resident, so the ground is hit at the finest level whichever level it is drawn at.

**The scheduler** (`pxl-kernel world/chunk/table.rs`) is one table with a slot per chunk: flags, distances, bounding sphere, pending-since, retained bytes. It runs four passes a tick. **mount** decides which chunks exist: every chunk of the window's top level that its drawn ring holds, each drawn whole or as its four children a level down where it lies whole inside the ring that level draws and the page under it has come, down to the window's base. The rings nest as a clipmap's do, so a level's chunks never leave its own ring and each has a mesh slot to itself. A chunk no longer wanted is stale rather than gone: it is drawn until every chunk replacing it has a layout, and the replacements wait under it, so a split or a merge is never a hole. **refresh** measures each against the frustum; **recheck** asks the LOD ladder which tiers distance wants; **commit** applies a layout whose tiers are all in hand, or releases one that has gone off screen.

**What to build next** is one priority. A chunk build runs in two stages, a scan and a mesh bake, and one of each is open at a time; the priority (`sched/priority.rs`) puts the two chunks holding those slots first, then chunks in the frustum by distance from the lead point, aged so that nothing waits forever. The lead point is where the camera is going, so a pan fills in ahead of the view. Builds run in the build task on what the tick left of the quota.

**Memory is paid for in LOD.** A refused block ends the tick's builds and the chunk waits for a release; reclaim gives back off-screen work, farthest first. If a refusal finds nothing to reclaim, the ladder steps in one tier — at most every half second, up to three — and steps back out after five seconds without a refusal. The view gets coarser; the tab does not die.

## Rendering

Each tick ends with a **draw list** (`pxl-render frame/draw.rs`). In it: the chunk meshes at the weights the LOD blend chose; decor as instanced draws, one model at many placements; the actors — the companion and other characters, each with a joint palette for skinning; the location puck; the billboards, labels and bubbles drawn flat to the camera; and the dev gizmos.

The kernel writes the frame's uniforms into the stage lane (`frame/stage.rs`) and calls `render_frame` (`gpu/renderer.rs`) with a `FrameInput`: the list, the environment, the models and their skins, the billboard sheet (the one texture every label is drawn into), and the surfaces. The renderer draws into every open surface — the world's canvas and the character picker's display case — and reports what the frame cost and whether it wants another turn. It decides nothing.

It reaches WebGPU through calls the host answers (`gpu/api.rs` ↔ `apps/web/src/kernel/render/gpu.ts`). A call carries numbers and the address and length of what it names, and every GPU object is one of xpute's handles: 2^15 slots for objects, 8 for surfaces.

Shaders and pipelines are the kernel's (`frame/shader/`). A pipeline is a record of words — shaders, vertex lanes, bind group layouts, primitive, depth and blend state — that the host turns into a WebGPU descriptor, and that a native host could turn into wgpu's. `cargo test` compiles every shader with naga, the WGSL compiler, and checks each record against its shader.

Two choices outlive a frame. Depth is reversed float, near at 1, so precision falls where perspective needs it and far coplanar faces do not fight. And on device loss the blocks are still in memory, so everything is re-uploaded from the addresses it already had.

The **view** (`pxl-kernel world/frame/view.rs`) is an orbit camera in a root frame of reference that the world stands in — a frame in the coordinate sense, not a drawn one. The eye never turns: the world rotates under it by the map's azimuth and is translated so the map centre is the origin. The local origin is the map centre's L4 base, which keeps the world in `f32` range near the camera; the translation rebases in the same step that it snaps. The matrices are doubles, written the moment a damping step moves, and read by the scheduler, the renderer, placement and the pointer's ray.

**LOD** is a chunk's level and its tier together. A chunk is 64 cells of its level, 2^k units each, drawn from its level's page and from the objects folded to that level by the same majority as the terrain, one dimension up (`pxl-render mesh/paint/object/fold.rs`); decor is scattered from the leaves, whatever level draws the ground over them. A chunk at level 0 builds its own cell's stride and the next; above it, a chunk builds only the next, its cells averaged 2×2, so every stride past 1 is drawn one way — the mean of the level below it — and never as the fold's vote. Past that a chunk merges into its parent or splits into its children at the edge of the ring the level below draws, so the world stride is `1/2/4/8/16/32/64` and on. A tier hands off when one of its cells would project to about six pixels, a screen-space error from viewport height and field of view (`pxl-render look/lod.rs`). At the reference viewport height that puts the handoffs at `192/384/768/1536/3072/6144` units, proportional to the stride. The leaves' two tiers dither with complementary screen-space masks; above them a handoff is a split or a merge at a ring's edge, at least `256·2^k` units from the anchor for level k, past the ladder's `192·2^k`. A chunk coming into view starts at its distance's tier.

The first handoff is the unit everything else keeps step with: the fade of billboards and decor's range. Decor is drawn by the same rule on a kind's extent — a thing covering fewer pixels than two of the tier's cells is not drawn.

Culling uses the exact union of the terrain, object, primary and secondary mesh bounds, which survive GPU eviction and are invalidated with their sources. Until a chunk has been measured, a conservative guard keeps a low camera from culling the ground under it.

**A coarse cell** (`lod_step > 1`, `mesh/chunk_mesher.rs`) is the `lod_step²` columns under it summarised, and the ground and what stands on it are summarised apart. The ground is a height field, so only its footprint coarsens: one run from the lowest floor to the columns' mean top, in units, its colour the mean of their top surfaces and up to four of their texture layers, a full run that merges and culls faces like any other. What objects add above it is a volume: `lod_step³` voxels to one box whose fill is coverage over the cube, shrunk about the cell centre by the cube root of the fill so its volume is the fill's, on an inset path with all four creases. Heights are units at every level, so no ground is ever a cell part full.

```text
   eight voxels, one solid            the box that replaces them

        ┌───┬───┐                          ┌───────┐
        │   │   │                          │       │
        ├───┼───┤   fill = 1/8             │  ┌─┐  │
        │▓▓▓│   │   ∛(1/8) = 1/2  ──────▶  │  │▓│  │
        └───┴───┘                          │  └─┘  │
                                           └───────┘
                                            half the side, an eighth the volume
```

Flat ground at a distance therefore reads as separated cubes rather than a sheet — the cost of a rule that treats every axis alike, taken over rules where orientation would decide whether a thing exists.

**Face culling and merging** happen in the run domain. A run that is not the top of its column and whose four horizontal neighbours are solid across its exact `z0..z1` span emits nothing; chunk-edge columns are exempt, since the neighbour across the edge cannot be checked. Adjacent columns holding the same run — same span, same material — merge into one box, so a plateau is one box, and a merge may only cross a border that carries nothing to draw or shade.

## Camera and settings

The camera is three controls — `dist`, `polar`, `azimuth` — and nothing else writes them. Terrain may ask one question, in `control/camera/terrain_gate.rs`: would this state put the eye inside the ground. The gate is a function of the three controls only, and it sits in `set_view_state`, the one funnel every input path goes through, so a refused move never leaves the controls and the drawn camera disagreeing. `Settings > Camera` picks the answer: `through` ignores terrain, `stop` takes a control as far as it can, `climb` grants the move and raises the drawn pitch instead, so the pitch relaxes back on its own.

Every setting is one entry of one schema (`spec/pxl/web/kernel/shell/setting/schema.json`): default, kind, section. The store keeps only the user's overrides, and the settings sheet renders its rows from the schema. Dev-only entries — gizmos, the ledger panel, a render-path bisect — are marked `dev`.

## Billboards and POI

Places are a second layer of the same pyramid. A POI page has the same name, level and cells as the ground page it lies over, and holds only a count per cell (`pxl-spec bake/poi_page.rs`, `PXP1`); a page with no places is no object, so a 404 is an empty page rather than a failure. Where the ground folds by majority, places fold by sum, so a coarse page counts everything under it and the client folds a coarse page from the ones it holds exactly as the bake would. Which POI pages are fetched, kept and folded is decided by the pyramid in the window the ground uses (`pxl-kernel world/pyramid.rs`, `layer/poi.rs`), so there is no POI-specific residency, schedule or key. The layer is off in production until the nationwide pages are published.

Placement (`layer/poi/placement.rs`) reads the ground the scheduler is drawing: at each drawn chunk's level and step, the counts are summed over the step's block of cells, and the badge stands at that block's centre (`cell_centre`, `pxl-spec space/pyramid.rs`) on the drawn top face there. A badge is therefore where its ground is drawn, at the same resolution, and moves between levels with it. Candidates outside the viewport are skipped, the rest are ordered by distance to what the view looks at, and each fades by its own cell size, so a coarse count stays visible as far as its level reaches. Screen collision decides which are shown and never changes a count.

The local server (`geo-bake demand/`) holds every pyramid layer the same way (`demand/pyramid.rs`). A name is a page; a leaf is baked from the layer's source, and a page above it is folded from the sixteen pages of the level below, faulted in one at a time so a coarse request cannot fan out, exactly as the offline bake folds (`pyramid.rs`). A layer supplies its leaf and its encoding and nothing else. Past `--top-level` nothing is answered, which is the one guard every layer shares; geocode's dictionary and groups are the only other kind. All kinds share admission and execution limits. Bake work runs in child processes with an execution deadline, because timing out a blocking thread would leave the bake running, and one absolute deadline follows a job through dependency waits, slot waits and execution. Over-budget work returns 503 without publishing. A server holds its cache directory for as long as it runs and clears what a stopped server left there, because a generation deletes itself only when its server shuts down cleanly. A job's key is the path it publishes, so finished work is found on disk rather than remembered; only running work and source failures are held. Invalidation replaces a kind's whole generation, so older in-flight jobs cannot repopulate the current cache.

Badges are one instanced quad (`layer/billboard/field.rs`) drawn from one atlas (`pxl-render asset/atlas.rs`, one texture the badges are packed into shelf by shelf): anchor, size and sheet rectangle per instance, with the fit to screen pixels, the distance fade and the badge's anchor pinning in the vertex shader. Occlusion is all or nothing per badge: the pass (`frame/shader/badge.wgsl`) binds the world's depth as a texture, reprojects several points up each badge through it, and discards the badge when most are behind terrain. The same pass writes `gl_FragDepth` from an upright twin of the quad, so billboards sort against the world along world-up. Chat bubbles are drawn by the same pass, with a sprite where a badge has text.

## Catalogs and assets

Everything a build ships is decided at build time, as JSON under `spec/` grouped the way `crates/` is. `script/spec.ts` writes a Rust module from each file and, where the shell reads it, a TypeScript one; `deno task spec` writes them and `check:spec` fails when the tree disagrees with the JSON. That covers the command numbers, the cell catalogs, the memory map, the GPU API's enums, the ledger's layout, the settings schema, the decor and character catalogs, the routes and the published versions.

A catalog is never sent. Both sides compile it, so a message carrying it would be a program telling itself what it already knows. Where bytes do cross — a published page, a saved blob — the layout is the model's and the version is in the envelope: the path, or the storage key.

- **Cubes and decor kinds** are numbers in `spec/pxl/spec/cell/` (`cube_type.json`, `color.json`, `decor.json`), and the number is what the object store writes.
- **Models** are named by path under `asset/` in the catalog that uses them — each decor kind's `models` and `skins` (`spec/pxl/web/kernel/layer/decor/kind/kind.json`), each character group's `keys`, `models`, `skin` and `size` (`spec/pxl/web/kernel/layer/character/variant.json`). A path resolves to a bucket key and a measured extent through the asset table `ingest-asset` writes (`pxl-spec asset/ingested.spec.rs`, `apps/web/src/catalog/asset/ingested.spec.ts`), naming every model, skin and texture by its own path with a hash of its bytes in the stem — `asset/actor/kenney_cube_pets/beaver-20f91809e69c.glb`, the way a bundler names a chunk — and measuring each glb's bounds. Nothing imports a model or an image; the client fetches a key, the same key from `./out/asset` in development and from R2 in production.
- **The texture pack** is one table (`pxl-spec asset/texture.rs`: which image a cube type wears on which face) and one WebP sheet in the bucket, from which the client fills its layer array one layer at a time. A cube's colour is `color.json`'s, what the minimap and the coarse tiers draw; a pack decorates it.
