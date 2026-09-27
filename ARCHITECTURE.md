# Architecture — pixelet

[pixelet](./README.md) turns geographic data and player-built blocks into one voxel world and draws it in a browser tab or a device app. It is an application written over [xpute](./ARCHITECTURE.xpute.md): a Rust kernel that runs as xpute's guest, a TypeScript shell that is xpute's host, and Rust servers and bakes that share the kernel's model. The kernel is one WebAssembly module in a tab and a library linked into the app on a device; the shell is the same on both. This document is what pixelet builds; what it builds on — the memory, the rings, the turn, the credits — is the other one.

Most of what follows is shaped by where it has to run: a phone, in a browser tab or an app, with a frame to fill. That is why the reservation is halved there, why a chunk's working storage is torn down as soon as it is scanned, and why detail is the first thing spent when memory runs short.

Three properties hold everywhere. The first two are xpute's and everything here lives inside them; the third is pixelet's own.

- The world's state lives in **one linear memory**, fixed at boot.
- All work runs in **turns under one quota**.
- What the client measures about itself goes to a **read-only profiler**. No decision reads it.

This document is in three parts. **The world** is the vocabulary: what a position is, what each level addresses, and what arrives at one. **The client** is the machine that holds it — the crates, the shell and the kernel on either side of it, and how a turn is spent. **The engine** is what is computed with it, from a chunk being built to a frame being drawn.

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

L5 is commented out (`L5`); when it exists it will address X and Y only, since the Z field of a packed position is 16 bits.

An extent is all the table gives. What is stored at a level is a separate question, and the answer is not always a cube: a level is an extent on one axis, not on three. `BASE`, `CENTER`, `LIMIT` and `IDX` (`pxl-spec space/indexer.rs`) are the same operations at every level, and whether a layer fills one axis, two or three is that layer's own choice.

The words for what sits at a level build on one another. A **Cell** is an address and nothing else — one L1 block, named and possibly empty. A **Pixel** is a cell carrying a 2D sample; a **Voxel** is a cell carrying a material. A **Tile** is a level filled on two axes — terrain at L3, a raster of pixels with height, which is why it is 2.5D and not 3D. A **Volume** is a level filled on all three, which is the shape a chunk's objects take and the shape a chunk is built into.

One more word belongs to the renderer rather than to the model. A **Band** is one `64³` volume materialized as voxels, the working storage a chunk is composed into. It is never a level and never persisted.

| Layer                       | Level              | Shape  | Concrete type                                   |
| --------------------------- | ------------------ | ------ | ----------------------------------------------- |
| terrain, source             | L3 Page            | Tile   | a `.fixed.terrain` record                       |
| object, source              | L2 Chunk, per band | Volume | sparse tree per band, leaves catalog values     |
| terrain + object, for build | L2 Chunk           | Volume | a volume of bands (`pxl-render mesh/volume.rs`) |
| any                         | L4 Anchor          | —      | the view's frame of reference                   |

Objects are client-local. The store (`pxl-kernel layer/object/store.rs`) keeps a sparse octree per occupied band, leaves being catalog values, and derives the chunk's entries as columns inside every write, in the trees' walk order, so a cell's entries are one run found by a binary search — the occupancy and hit queries read no index of their own. It persists to the platform's storage — `localStorage` in a tab, `UserDefaults` on an iPhone, `SharedPreferences` on Android — as five columns per page — page-local `x`, `y` and `z`, footprint side, value — under a key that carries the layout's version. One chunk — a slot of `world/chunk/table.rs`, built by `world/chunk/build.rs` — is one L2 cube of its level, named by level and x, y and z (`world/pyramid/chunk.rs`); it composes the column it stands in, as many signed 64-cell Z bands as the column's content touches, and bakes the part in its own heights. A page holds 64 columns, and a column as many chunks as its ground and objects reach through.

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

Sea is a `LAND` value (`sea = 129`, `spec/pxl/spec/cell/cube_type.json`). A page stores each channel as a dense index into that channel's few types rather than the catalog's number, so all four pack into one 16-bit cell (`pxl-spec bake/pack.rs`). The channels are rasterized from Overture's Buildings, Transportation and Base themes, the coastline from Base's `water`.

What the view draws is decided from the eye, in three dimensions. A chunk of level L — a cube 64 of L's cells a side — is split into its eight children while the nearest point of its box is nearer the eye than L's **split distance**, a fixed multiple of its cell (`pxl-kernel world/pyramid/domain.rs`, [what a view costs is the eye's](./issue/20260925-what-a-view-costs-is-the-eye-s.md)). So every cell drawn covers about the same share of the screen wherever the eye stands and however the camera is tilted, as Cesium's selection does, and the rule runs from the pyramid's top down to terrain's atom. A child that holds nothing — no ground crosses it and no object's band is in it (`Domain::occupied`) — is never made, so the air is never split, counted or drawn; ground outside the frustum is not split at all. The map center is only the handle the camera is moved by, and plays no part in this or in anything below.

What a view holds is a budget, not a count of levels. The chunks the rule draws, the pages they need and every ancestor of those are counted before anything is asked for, and where they are more than the device holds (`VIEW_CHUNKS`, `VIEW_PAGES`, `LEVEL_CHUNKS`), every split distance is shortened alike until they fit: the whole view coarser, never a piece of ground left out. Those pages answer three questions at once — fetched, kept decoded, given a record — so a page cannot be requested and then dropped from under its own request, and every layer on the pyramid is held to the same pages.

The rule's chunks are the view's **domain** (`Domain`): every piece of ground within reach, once, at the level the rule wants it. What a layer can draw of it is its **cover** (`world/pyramid.rs`): each chunk drawn from its own page where that has come, and from the nearest resident page over it where it has not (`drawn_from`) — that page's ground at the chunk's own cells, as Cesium upsamples a parent tile — so ground is on the screen the turn its page lands and is a gap only where nothing over it is held. Everything is asked for in one order out from the eye, the same for every layer (`Order`, `io/policy.rs`), and a fetch the view has left is stopped the turn it leaves (`io/sched.rs`).

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

Those pages are the leaves, z16. A view from farther away wants fewer, coarser pages, so the bake stacks levels on them: a level-k page covers the 2 × 2 level-(k − 1) pages under it at half their resolution and is the same record as a leaf, named by the Web Mercator tile it stands on so that a page's name is one digit shorter than its children's and a prefix of them. It is folded from the level below with no database in the loop — each ground channel by a majority of the four cells under it, the elevation by an average that reads past the page's edge — and the fold is `pxl-spec`'s (`bake/fold.rs`), so the client can fold the pages it already holds by the same function instead of fetching. Open water folds to itself, which is what lets the one ocean page serve every level.

The dev tile server (`geo-bake demand/`) serves every pyramid layer the same way (`demand/pyramid.rs`): a leaf baked from the layer's source when it is asked for, a page above it folded from the level below exactly as the offline bake folds (`pyramid.rs`), and nothing past `--top-level`, the one guard every layer shares.

---

# The client

Two things run on the client: a kernel that decides and a shell that carries out. Where a piece of code lives follows from that, and so does which language it is written in.

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
view                  │     │              │ pxl-render    │            │ pxl-tool     │  ingest, bakes
                      │     │              └───────┬───────┘            └──────┬───────┘
                      │     │              ┌───────▼───────┐            pxl-server · pxl-geo-spec
kernel                │     │              │ pxl-kernel    │            beneath them, no model
                      │     │              └───────┬───────┘
                      │     │                      ├──────────────────────────┐
                      │     │              ┌───────▼───────┐          ┌───────▼───────┐
bracket               │     │              │ pxl-wasm      │          │ pxl-native    │
                      │     ▼              └───────┬───────┘          └───────┬───────┘
              ┌───────▼─────────────┐              │ kernel.wasm    ┌─────────▼─────────┐
app           │ apps/web  *.spec.ts │◀─────────────┘                │ apps/native       │
              └──────────┬──────────┘                               └─────────▲─────────┘
                         └──── the same shell, its own @platform/ ────────────┘
```

| Crate / package | What it is                                                                                                                                                                                                                                  |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pxl-spec`      | The model: PVF3, the spatial hierarchy, the cell catalogs, the command numbers, the page formats, the asset tables. One implementation, in Rust.                                                                                            |
| `pxl-render`    | The view: volumes, the column scan, the mesher, glTF, the LOD criterion's reference, the atlas, the draw list, the shaders, the renderer.                                                                                                   |
| `pxl-kernel`    | The kernel: the chunk scheduler, the layers and their stores, fetches and loads, the camera and sensors, the turn, the profiler.                                                                                                            |
| `pxl-wasm`      | The kernel in a tab: one module that fixes the memory's front with linker flags, imports the clock and exports `interrupt`.                                                                                                                 |
| `pxl-native`    | The kernel on a device: a static library that takes its memory from the system, runs every call on a thread of its own, and exports `pxl_interrupt` and the surface's binding through the C ABI, and through JNI on Android (`android.rs`). |
| `apps/web`      | The shell, both apps': React Native over the kernel's snapshot, the I/O the kernel names, and what only a page does — the WebGPU host among it; and the Cloudflare Worker that serves it all.                                               |
| `apps/native`   | The device app: the shell run by Metro, what only a device does, and the native module that links the kernel.                                                                                                                               |
| `pxl-server`    | What both servers need before answering: the environment variables, the database pool, the gate over internal routes.                                                                                                                       |
| `pxl-geo-spec`  | What the geo server and the bakes agree on about db-geo: table names, and what Overture's categories and divisions mean.                                                                                                                    |
| `pxl-geo`       | The world's server. Forward search — the one route that queries a database at request time.                                                                                                                                                 |
| `pxl-api`       | The application's server.                                                                                                                                                                                                                   |
| `pxl-geo-bake`  | The three offline bakes — terrain, POI, geocode — and, behind a feature, the dev tile server.                                                                                                                                               |
| `pxl-tool`      | Hashes `asset/` into the bucket's shape and writes the asset tables `pxl-spec` and the shell carry; bakes the stars, the Moon, the brand.                                                                                                   |

The servers, the bakes and the asset ingest all compile the model, so they are crates. The Overture and elevation ingests move a provider's columns into Postgres columns and touch no model, so they are scripts under `infra/geo/`. The shell is TypeScript because it decides nothing about the world; what it needs of the model — catalogs, ABI enums, routes — is generated into `apps/web/src/**/*.spec.ts` from the same JSON the Rust is generated from.

Three rules hold the graph in that shape:

- **The view does not know the kernel.** `pxl-render` sees the model and a `FrameInput`, and returns what a frame cost.
- **The kernel does not know which bracket holds it.** A **bracket** is what gives the kernel a place to run: `pxl-wasm` in a tab, `pxl-native` on a device. It hands in the memory, the clock, the allocator's reading and where to say what stopped the kernel (`door.rs` `Bracket`), and the kernel asks everything else of the host in records the host runs after the turn — storage, text, a body's bytes (`host.rs`). So the kernel also runs under `cargo test`, with no bracket at all. The one thing the target decides is how the renderer's calls are made: recorded for the page's WebGPU on wasm32, made through wgpu anywhere else (`pxl-render gpu/api.rs`).
- **Nothing is found by name.** Commands, cells, decor kinds (what the world grows on its own — trees, cars, boats, trains), character variants (the people and pets a player can be), assets and messages are numbers fixed in data. The words a message is shown in belong to the shell's locale files.

## Shell and kernel

The shell is a device driver (`apps/web/src/kernel/`). It does five things:

- Opens the kernel: reserves the memory and instantiates the module over it in a tab, asks the app's native module for the linked kernel's memory on a device (`platform/core.ts`, `mem.ts`, `module.ts`).
- Rings the doorbell once per animation frame, when something asked for one.
- Forwards pointer, wheel, sensor and visibility events as commands (`control/input.ts`).
- Does the I/O the kernel names — fetches, image decodes, sensor watches, storage, a line of text laid out or drawn — and hands the bytes to blocks the kernel chose.
- Hands over surfaces, each named by a handle like any GPU object. In a tab a canvas is transferred to an `OffscreenCanvas` once; on a device a view binds its own surface to the handle while it has one — a Metal layer on an iPhone, a `SurfaceView`'s on Android — and the kernel draws into it directly.

Both apps run this one tree of TypeScript. What differs between a page and a device is behind one import prefix, `@platform/`, which each app resolves to its own directory: `apps/web/src/platform/web/` for Vite, `apps/native/src/platform/` for Metro. Each module there exports what its twin does — a check fails the gate if a device's module lacks a name the page's has (`platform/conform.ts`) — and the device's modules reach the system through one native module of the app's (`apps/native/modules/platform/`). A device's kernel runs every call on a thread of its own, with a stack it sizes, while the caller waits ([a device's kernel on a thread of its own](./issue/20260926-a-device-s-kernel-on-a-thread-of-its-own.md)).

The UI is React over one snapshot, a packet the kernel writes after a turn in which something changed. Every control is a command of its own that the kernel applies. Nothing in the shell computes a coordinate, a tile or a draw.

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
│ seven tasks beside it, polled on what the tick left              │
│                                                                  │
│   build · fetch decode · glTF · two folds · decor · store's save │
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
| `interaction` | input, camera damping, the environment (sun, moon, theme), the view's frame of reference, the companion (the player's character at the map center); O(1), unbudgeted |
| `visible`     | the chunk scheduler's pass                                                                                                                                           |
| `content`     | what is about to be on screen; guaranteed `content_share` of the budget                                                                                              |
| `cosmetic`    | what only looks better for running (debug gizmos)                                                                                                                    |
| `report`      | the profiler, last, so it sees everything                                                                                                                            |

**The tasks** run on `embassy-executor`'s raw executor, which the module installs. The frame is a task rather than a function so that the order above is its own, and no task depends on the order the executor happens to poll them in.

**The credits** bound what is out, and one place spends them (`io/sched.rs`). A producer says only what it wants — the subjects it wants read, each with a rank, replacing whatever it said last turn — and never that an attempt should start, stop or be retried. What is wanted, what is running and what has just failed are three tables, so an attempt ending answers nothing about the demand that outlived it, a failure waits rather than asking straight back, and the demand of a camera that has just moved is representable however much old work is still draining. A rank is a class and a distance; the numbers behind both are in one module (`io/policy.rs`) rather than one per layer, and a model or a texture is a read like any other, spending the same credits as a page rather than bidding against it from a second scheduler.

**The profiler** (`sched/profiler`, laid out by `spec/pxl/web/kernel/sched/profiler/profiler.json`) records what each frame and each task cost — pacing, per-phase time, GPU ask, heap gauges, LOD pressure, draw census, per-job waits — into rings the shell reads in place (`PROFILER_READ`, `PROFILER_TAKE`) and prints as a report or exports as a Chrome trace. The scheduler never reads them.

## The channel

The shell and the kernel share three rings, each 2048 messages of four words with 4 KiB payload slots. Two of them are the pair xpute describes: one for what the shell submits, one for what the kernel completes. The third is a spill, which the completion ring overflows into when it is full, so that nothing is dropped and completions stay in order.

The commands are `spec/pxl/web/kernel/abi/cmd.json`'s: each one's number and the fields of the packet it carries, from which both sides' readers and writers are generated, so no field is found by its position anywhere else. A packet is an XTP tree, and anything larger than a slot crosses in a heap block the kernel hands out.

The kernel also posts messages nobody asked for: a fetch to start or cancel, a model or image to decode, a sensor to watch, a console line, a new snapshot. Those are its signals.

The world's change log (`world/change.rs`) keeps what is stored: a revision per column, and per chunk one for its tiers and one for its decor, which a write to a band it draws moves (`world/chunk/source.rs` `band_written`), so a chunk reads one revision rather than summing its columns'. They move on a write and on nothing else: the eraser's previews are a chunk's sources, not a change to the store. Listeners run when the tick pumps, never inside the write.

## The memory map

The client's state is one memory, reserved once and never grown: in a tab a `WebAssembly.Memory`, 1 GiB on the desktop and 512 MiB on a phone, `initial == maximum`; on a device one region of 512 MiB the library takes from the system on the first call (`pxl-native memory.rs`), zeroed and committed page by page as it is touched. Since it never moves, a ring keeps its words and a table is read where it lies for the kernel's whole life — the way Figma's editor core holds every object as a range inside one pre-allocated array. A phone's tab gets half because JavaScriptCore counts the whole reservation as the tab's JavaScript memory, committed or not.

Every address the kernel names is an offset from the memory's first byte (xpute's `mem/base.rs`), which is address zero in a module and the region's start on a device. So the map below holds on both, and the shell reads the same numbers either way. On a device the stack and the library's own data are where the system put them, outside the region, so the two ranges the map keeps for them below — `.stack` and the linker's — stand empty there.

```text
address        bytes                    range                  placed by
──────────────────────────────────────────────────────────────────────────────────────
0              1 MiB                    .stack                 rustc (-z stack-size, --stack-first)
1 MiB          36 KiB × 3               .ring.desc             build.rs
2 MiB          8 MiB × 3                .ring.slot             build.rs
32 MiB         ~0.7 MiB                 .rodata .data .bss     wasm-ld (--global-base)
               ↑ __data_end, rounded up to 4 KiB (PAGE_BYTES)
~32.7 MiB      25.12 MiB                .fixed.terrain         the kernel (mem/fixed.rs)
~57.8 MiB      1 MiB × 2                .fixed.runs            the kernel (mem/fixed.rs)
~59.8 MiB      1 MiB                    .fixed.bake            the kernel (mem/fixed.rs)
~60.8 MiB      11.60 MiB                .fixed.surface         the kernel (mem/fixed.rs)
~72.4 MiB      2.12 MiB                 .fixed.decor           the kernel (mem/fixed.rs)
~74.5 MiB      0.88 MiB                 .fixed.blocks          the kernel (mem/fixed.rs)
~75.4 MiB      4 MiB                    .fixed.gpu             the kernel (mem/fixed.rs)
~79.4 MiB      64 KiB                   .fixed.host            the kernel (mem/fixed.rs)
~79.5 MiB      4.72 MiB                 .fixed.profiler        the kernel (mem/fixed.rs)
~84.2 MiB      428 MiB  (phone)         .heap                  the allocator
               940 MiB  (desktop)
512 MiB / 1 GiB                         end
──────────────────────────────────────────────────────────────────────────────────────
```

The map is `pxl-kernel mem/section.rs`'s, over xpute's sections. The stack is first so that an overflow wraps below zero and traps — there is no MMU, so the wraparound is the only guard — and the kernel asserts at boot that the linker's `__stack_high` is where `mem.json` says. The rings sit at fixed addresses and `build.rs` emits `--global-base` from the same numbers, so the linker cannot place `.data` over them.

`.fixed` is what the kernel places once, sizes at compile time and never frees; the table above lists it. The largest range is the resident terrain (`layer/terrain/fixed.rs`), one record a page: a record for every page a view can want (`VIEW_PAGES`, 48, the budget the view's criterion is loosened to fit) and one for the pinned page, so every page the view wants has room at once. Which page lies in which record is the pyramid's (`world/pyramid.rs`), not a coordinate's. Every layer's records begin with `ABSENT`, which is no page's own and which every absent page shares — in the terrain's range, the open ocean. A page that arrives takes a free record or the one worth least, one the view has left before one it wants, and the pinned page never gives its record up. A thing belongs in `.fixed` when it is in the kernel's own format and its size is settled at compile time; the heap is left for what is asked for, refused and given back, and for what another format sizes — a glTF, a PNG — whose bound nothing here can predict. Every range is reached through `mem/fixed.rs`, which checks what is asked for against the range's end, so nothing written through one lands in the next.

`.heap` is everything else, under xpute's slab over buddy. A `String`, a band's voxels and a fetch body all come from here. A refused request is a wait, not a failure: the build keeps its place and asks again after a release, and the view's criterion is loosened until what it wants fits (`world/pyramid/pressure.rs`).

The numbers — page size, stack, ring addresses, `--global-base`, the two reservations, the heap a phone is guaranteed (`MIN_HEAP_BYTES`) — are one file, `spec/pxl/web/kernel/mem/mem.json`, read by the kernel's map, by `build.rs` and by the shell's reservation, and asserted at compile time and at boot.

What the renderer reads each frame is laid out as **column lanes**, structure-of-arrays — not the packed-word lanes of a coordinate. They are terrain planes, voxel bands, column runs, a mesh per LOD tier, the chunk table, decor instances, the object store's records, and every registry. A table whose columns move together is one struct of lanes with one length. Slots are dense, and an index is told when its slot moves — except in a handle table, where the slot is what the other side holds, so it stays and is reused under a new generation. A lane set has one owner; a consumer borrows it for one call. The JavaScript heap holds only glue: React, the WebGPU objects behind the kernel's handles on a page, and response bodies until the kernel takes them.

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

In a band, `0` is VOID, `128` is explicit AIR — a leaf that erases what a lower layer would show — and `255` is UNKNOWN. Any other value is at once a placed color, a terrain pick or a decor kind, and its id in the one global palette (`pxl-spec cell/`).

Objects paint after terrain, so a placed value wins the voxel. A chunk records the revisions it was built from — its terrain page's, and its own object and decor revisions — and is not rebuilt while they stand.

**Bands are temporary** (`pxl-render mesh/volume.rs`). A band is `64³` voxels, five bytes each — a `u32` `material` `0xCC_RRGGBB` and a `u8` `layer_idx`, plus a sub-cell payload where fine objects need it — about 1.25 MiB, allocated whole when one peak touches it. That is too much to keep for every visible chunk on a phone, so bands live only while a chunk is composed and scanned; sixteen band stores keep their lanes between composites, because reallocating them was the heap's worst churn.

**The column run set lasts as long as its bakes.** A column is read bottom to top, and a run extends while the material holds. A chunk's `66²` columns, its own and an apron, become their runs in one of `.fixed.runs`'s two sets; a set past its room is refused rather than cut, since a run left out is a hole. Every tier the chunk has is baked from the set while it is there, and then the set is let go: the tiers are what lasts, and runs kept for a tier asked later would be a cache of a cache.

**A mesh is its boxes, and lives on the device.** Each tier's mesh is held in a slot of the fixed buffers the renderer keeps (`pxl-render gpu/arena.rs`). A slot is room, not an address: any tier takes any free slot of its pool (`world/chunk/mesh_slot.rs`), and a slot is taken over only once the chunk holding it is no longer drawn, so a mesh outlives its page's record and nothing is freed. What a slot holds is a bound that closes: a mesh past it is baked a step coarser until it fits, never cut, since a box left out is a hole. A mesh holds nothing per vertex — a box is its corner, size, color, texture layers and face mask — and the chunk pipeline binds it as storage for its vertex shader to unfold into a cube (`frame/shader/chunk.wgsl`).

The raycast keeps its own exact composites near the pointer, made from the leaf page wherever one is resident, so the ground is hit at the finest level whichever level it is drawn at.

**The scheduler** (`pxl-kernel world/chunk/table.rs`) is one table with a slot per chunk: flags, distances, bounding sphere, pending-since, retained bytes. It runs four passes a tick. **mount** decides which chunks exist: the terrain's cover of the view, each chunk at the level the rule wants its ground at, or a coarser or finer one standing in where that page has not come. A chunk no longer wanted is stale rather than gone: it is drawn until every chunk replacing it has a layout, and the replacements wait under it, so a split or a merge is never a hole. **refresh** measures each against the frustum; **recheck** asks the view's rule which tier distance wants (`wanted_cube`); **commit** applies a layout whose tiers are all in hand, or releases one that has gone off screen.

**What to build next** is one priority. A chunk build runs in two stages, a scan and a mesh bake, one of each open at a time; the priority (`world/chunk/table.rs` `priority`) puts the two chunks holding those slots first, then what the view sees nearest the eye, then what it does not, so the view fills out from the eye. Builds run in the build task on what the tick left of the quota.

**Memory is paid for in LOD.** A refused block ends the tick's builds and the chunk waits for a release; reclaim gives back off-screen work, farthest first. If a refusal finds nothing to reclaim, the view's one criterion is pulled a level coarser (`world/pyramid/pressure.rs`) — at most every half second, up to six — and its split, its pages and its tiers coarsen together; it is let out again after five seconds without a refusal and with the heap under 90%. The view gets coarser; the client does not die.

## Rendering

Each tick ends with a **draw list** (`pxl-render frame/draw.rs`). In it: the chunk meshes at the weights the LOD blend chose; decor as instanced draws, one model at many placements; the actors — the companion and other characters, each with a joint palette for skinning; the location puck; the billboards, labels and bubbles drawn flat to the camera; and the debug gizmos.

The kernel writes the frame's uniforms into the stage lane (`frame/stage.rs`) and calls `render_frame` (`gpu/renderer.rs`) with a `FrameInput`: the list, the environment, the models and their skins, the billboard sheet (the one texture every label is drawn into), and the surfaces. The renderer draws into every open surface — the world's and the character picker's display case — and reports what the frame cost and whether it wants another turn. It decides nothing.

Its calls are WebGPU's, and how they are made is the target's (`gpu/api.rs`). A module cannot reach WebGPU itself, so on wasm32 they are records the page makes once the turn has returned (`api/record.rs` ↔ `apps/web/src/platform/web/webgpu.ts`); on a device the same calls are made through wgpu as they are asked for (`api/device.rs`). A call carries numbers and the address and length of what it names, the same words to both, and every GPU object is one of xpute's handles: 2^15 slots for objects, 8 for surfaces.

Shaders and pipelines are the kernel's (`frame/shader/`). A pipeline is a record of words — shaders, vertex lanes, bind group layouts, primitive, depth and blend state — that the page turns into a WebGPU descriptor and a device into wgpu's. `cargo test` compiles every shader with naga, the WGSL compiler, and checks each record against its shader. What a shader shares with the host has no room inside it — 4-byte scalars and `vec4`s, and arrays and structs of them with no gap — so the host's words and every driver's layout agree, and the tests hold every shader to it; a uniform block is bound as `array<vec4f, N>` and read back into its struct by one function, since a phone's Vulkan read a padded field and a matrix array in storage its own way ([a matrix the driver reads its own way](./issue/20260927-a-matrix-the-driver-reads-its-own-way.md)).

Two choices outlive a frame. Depth is reversed float, near at 1, so precision falls where perspective needs it and far coplanar faces do not fight. And on device loss the blocks are still in memory, so everything is re-uploaded from the addresses it already had.

The **view** (`pxl-kernel world/frame/view.rs`) is an orbit camera in a root frame of reference that the world stands in — a frame in the coordinate sense, not a drawn one. The eye never turns: the world rotates under it by the map's azimuth and is translated so the map center is the origin. The local origin is the map center's L4 base, which keeps the world in `f32` range near the camera; the translation rebases in the same step that it snaps. The matrices are doubles, written the moment a damping step moves, and read by the scheduler, the renderer, placement and the pointer's ray.

**LOD** is a chunk's level and its tier together, and a tier is a cube: the side the chunk is drawn in, which the shared rule chooses by the chunk's nearest distance from the eye (`world/chunk/table.rs` `wanted_cube`, `build.rs` `Cube`). A chunk is 64 cells of its level, 2^k units each, drawn from its level's page and from the objects folded to its cube by the same majority as the terrain, one dimension up (`pxl-render mesh/paint/object/fold.rs`), so an object's cube and the ground's are one size wherever both have data. Above z16 a chunk draws its cells averaged 2×2; at z16, near the eye, objects draw cubes finer than a cell down to their finest tier while the ground stays at z16, since it has nothing finer. A tier hands off when one of its cells would project to about six pixels (`pxl-render look/lod.rs`), so the world stride is `1/2/4/8/16/32/64` and on. A change of what a chunk draws dissolves rather than switches (a setting, on by default): the old and the new take complementary halves of one screen-space pattern for 150 ms (`world/chunk/table.rs` `Fade`), so a pixel shows one of them and never both or neither.

The first handoff is the unit decor's range keeps step with: a thing covering fewer pixels than two of the tier's cells is not drawn.

Culling uses the exact union of the terrain, object, primary and secondary mesh bounds, which survive GPU eviction and are invalidated with their sources. Until a chunk has been measured, its bounds are its column's heights cut to its cube, the ones the rule chose it by, and a chunk the camera stands in is seen whatever the frustum says of its center.

**A coarse cell** (`lod_step > 1`, `mesh/chunk_mesher.rs`) is the `lod_step²` columns under it summarized, and the ground and what stands on it are summarized apart. The ground is a height field, so only its footprint coarsens: one run from the lowest floor to the columns' mean top, in units, its color the mean of their top surfaces and up to four of their texture layers, a full run that merges and culls faces like any other. What objects add above it is a volume: `lod_step³` voxels to one box whose fill is coverage over the cube, shrunk about the cell center by the cube root of the fill so its volume is the fill's, on an inset path with all four creases. Heights are units at every level, so no ground is ever a cell part full.

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

**Face culling and merging** happen in the run domain. A run that is not the top of its column and whose four horizontal neighbors are solid across its exact `z0..z1` span emits nothing; chunk-edge columns are exempt, since the neighbor across the edge cannot be checked. Adjacent columns holding the same run — same span, same material — merge into one box, so a plateau is one box, and a merge may only cross a border that carries nothing to draw or shade.

## Camera and settings

The camera is three controls — `dist`, `polar`, `azimuth` — and nothing else writes them. Terrain may ask one question, in `control/camera/terrain_gate.rs`: would this state put the eye inside the ground. The gate is a function of the three controls only, and it sits in `set_view_state`, the one funnel every input path goes through, so a refused move never leaves the controls and the drawn camera disagreeing. `Settings > Camera` picks the answer: `through` ignores terrain, `stop` takes a control as far as it can, `climb` grants the move and raises the drawn pitch instead, so the pitch relaxes back on its own.

Every setting is one entry of one schema (`spec/pxl/web/kernel/shell/setting/schema.json`): default, kind, section. The store keeps only the user's overrides, and the settings sheet renders its rows from the schema. The nightly entries — the camera against terrain, the ground snap, terrain carving, the shadows — are marked `dev`: a development build offers them, and a published one carries neither their rows nor the kernel's code for them (`apps/web/script/check-bundle.ts` holds it to that). The debug section — the gizmos, the profiler panel — is in every build, since a release is what it most needs to measure.

## Billboards and POI

Places are a second layer of the same pyramid, folded by another rule. A POI page has the same name and level as the ground page it lies over, cut in a lattice sixteen times the ground's cell (`POI_LATTICE`), since a badge is many times a ground cell on the screen. A page holds a count per cell and, where a cell holds one place, which place it is: its name and its kind, the category its search result shows, numbered by the badge table both read (`spec/pxl/spec/bake/poi_badge.json`; `pxl-spec bake/poi_page.rs`, `PXP5`). Where the ground folds by majority, places fold by sum, so a coarse page counts everything under it and the client folds a page from the ones it holds exactly as the bake would; the tree stops where a page already says all of a child's ground, so an empty stretch is never asked for (`Folds::settles`). Which POI pages are fetched, kept and folded is the pyramid's, from the same pages the ground uses (`pxl-kernel world/pyramid.rs`, `layer/poi.rs`), so there is no POI-specific residency, schedule or key. The layer is a setting, off unless turned on.

Placement (`layer/poi/placement.rs`) counts what the layer's own cover draws, each place once: a cell of one place is drawn as the place, a cell of more as its count. A badge stands at the center of the z16 ground cell its places' centroid falls in, above the surface drawn there (`Cell::center`, `pxl-spec space/pyramid.rs`; `surface_at`), and where no ground is drawn there is no badge. Badges fade out over one sphere about the eye that grows with the zoom, and are drawn farthest first; none is hidden for overlapping another, since a badge stands for the places under its cell. A tap on a badge is the badge's before the ground's (`layer/poi/placement.rs` `tap`), and takes the view to where it stands.

Badges are one instanced quad (`layer/billboard/field.rs`) drawn from one atlas (`pxl-render asset/atlas.rs`, one texture the badges are packed into shelf by shelf): anchor, size and sheet rectangle per instance, with the fit to screen pixels, the distance fade and the badge's anchor pinning in the vertex shader. Occlusion is all or nothing per badge: the pass (`frame/shader/badge.wgsl`) binds the world's depth as a texture, reprojects several points up each badge through it, and discards the badge when most are behind terrain. The same pass writes `gl_FragDepth` from an upright twin of the quad, so billboards sort against the world along world-up. Chat bubbles are drawn by the same pass, with a sprite where a badge has text.

An entry of the sheet is drawn on the GPU as well, once, when what it shows changes (`pxl-render gpu/sheet.rs`, `frame/shader/sheet.wgsl`): its shapes — rounded rectangles, circles, rings, a bubble's tail — as signed distances, and its text as lines the platform drew, into a texture of the entry's size that is then copied into its rectangle. Text is asked of the platform a line at a time, so kerning, joining, bidirectional runs, fallback and emoji are the platform engine's on every target: the kernel asks for a text's lines and widths and then for each line's pixels (`pxl-kernel text.rs`), the page answering with its canvas, an iPhone with CoreText and Android with its `Canvas` ([the sheet on the GPU, and text by the line](./issue/20260926-the-sheet-on-the-gpu-and-text-by-the-line.md)).

## Catalogs and assets

Everything a build ships is decided at build time, as JSON under `spec/` grouped the way `crates/` is. `pxl-tool` writes a Rust module from each file and, where the shell reads it, a TypeScript one; `deno task spec` writes them and `check:spec` fails when the tree disagrees with the JSON. That covers the commands and their packets, the cell catalogs, the memory map, the GPU API's enums, the profiler's layout, the settings schema, the decor and character catalogs, the routes and the published versions.

A catalog is never sent. Both sides compile it, so a message carrying it would be a program telling itself what it already knows. Where bytes do cross — a published page, a saved blob — the layout is the model's and the version is in the envelope: the path, or the storage key.

- **Cubes and decor kinds** are numbers in `spec/pxl/spec/cell/` (`cube_type.json`, `color.json`, `decor.json`), and the number is what the object store writes.
- **Models** are named by path under `asset/` in the catalog that uses them — each decor kind's variants, a model and a skin each (`pxl-kernel layer/decor/kind/`), each character group's `keys`, `models`, `skin` and `size` (`spec/pxl/web/kernel/layer/character/variant.json`). A path resolves to a bucket key and a measured extent through the asset table `pxl-tool`'s ingest writes (`pxl-spec asset/ingested.spec.rs`, `apps/web/src/catalog/asset/ingested.spec.ts`), naming every model, skin and texture by its own path with a hash of its bytes in the stem — `asset/actor/kenney_cube_pets/beaver-20f91809e69c.glb`, the way a bundler names a chunk — and measuring each glb's bounds. Nothing imports a model or an image; the client fetches a key, the same key from `./out/asset` in development and from R2 in production.
- **The texture pack** is one table (`pxl-spec asset/texture.rs`: which image a cube type wears on which face) and one WebP sheet in the bucket, from which the client fills its layer array one layer at a time. A cube's color is `color.json`'s, what the coarse tiers draw; a pack decorates it.
