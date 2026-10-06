# Architecture — pixelet

[pixelet](./README.md) turns geographic data and player-built blocks into one voxel world and draws it in a browser tab or a device app. It is an application written over [xpute](./xpute/ARCHITECTURE.md): a Rust kernel that runs as xpute's guest, the TypeScript around it — its host, which is xpute's, and its shell, the UI — and Rust servers and bakes that share the kernel's model. The kernel is one WebAssembly module in a tab and a library linked into the app on a device; the host and the shell are the same on both. This document is what pixelet builds; what it builds on — the memory, the rings, the turn — is the other one.

Most of what follows is shaped by where it has to run: a phone, in a browser tab or an app, with a frame to fill. That is why the reservation is halved there, why a chunk's working storage is torn down as soon as it is scanned, and why detail is the first thing spent when memory runs short.

Three properties hold everywhere. The first two are xpute's and everything here lives inside them; the third is pixelet's own.

- The world's state lives in **one linear memory**, fixed at boot.
- All work runs in **turns under one quota**.
- What the client measures about itself goes to a **read-only profiler**. No decision reads it.

This document is in three parts. **The world** is the vocabulary: what a position is, what each level addresses, and what arrives at one. **The client** is the machine that holds it — the crates, the shell and the kernel on either side of it, and how a turn is spent. **The engine** is what is computed with it, from a chunk being built to a frame being drawn.

---

# The world

## Spatial Hierarchy

Every size in the world is a **zoom**. The X lane of a position is one world of 2^24 units, so a thing the world over 2^z wide is at zoom z — z is 24 less the log of its side in units, a bigger zoom is finer, and one zoom finer is half as wide (`pxl-spec space/coord/zoom.rs`). Sizes had been said three ways, a zoom for a tile, a scale for a cell and a fixed tier for the rest, and the conversions between them were where facts went double; one number for all of them leaves nothing to convert ([every size is a zoom](./decision/20261006-every-size-is-a-zoom.md)).

| Zoom | Side   | What it sizes                                            |
| ---- | ------ | -------------------------------------------------------- |
| z27  | `1/8u` | a placed object's finest cell                            |
| z24  | `1u`   | a ground cell, the unit a position names                 |
| z15  | `512u` | a leaf tile: the finest terrain is fetched and stored at |
| z5   |        | the coarsest tile, where the pyramid stops for now       |

The words for what is sized build on one another. A **cell** is the granule at a zoom, named and possibly empty: a **pixel** is a cell carrying a 2D sample, a **voxel** a cell carrying a material. A **chunk** is what is built and drawn as one, 32 cells of the camera grid a side, so its zoom is its cells' less five (`pxl-spec space/camera/grid.rs` `CHUNK_STEPS`) — Minecraft's chunk under another name, the unit a world is meshed in, cached at and rebuilt when edited. It is never seen: one cell of it is one thing drawn, and what lies inside a cell is its layer's to fold. A **tile** is what is fetched and stored as one: a layer's tile holds a count of its cells a side that is the layer's own and the same at every zoom (`pxl-spec space/layer/lattice.rs` `Lattice`), so a tile's zoom is its cells' less a constant — terrain's tile is 512 cells, its z24 cells making z15 tiles. A tile's zoom is a web map's tile zoom, and a z24 cell a web map's z16 pixel, which only the 3857 adapter says. A **block** is a thing placed, never a size.

Whether a layer fills one axis, two or three at a size is its own choice. A **raster** fills two — terrain's tile, a raster of pixels with height, which is why it is 2.5D and not 3D — and a **volume** all three, the shape the objects take. A lattice address goes to its cell at any zoom and back by the same two operations (`pxl-spec space/coord/zoom.rs` `idx`, `from_idx`).

| Layer                       | Size      | Shape  | Concrete type                                          |
| --------------------------- | --------- | ------ | ------------------------------------------------------ |
| terrain, source             | a tile    | Raster | a `.fixed.terrain` record                              |
| object, source              | the world | Volume | one sparse octree, leaves catalog values at their zoom |
| terrain + object, for build | a chunk   | Runs   | a column run set (`pxl-render mesh/run.rs`)            |

Objects are client-local. The store (`pxl-kernel layer/object/store.rs`) keeps only its record: each leaf's catalog value at its node, in one sparse tree a column of the 3D roots (`pxl-spec space/hierarchy/sparse_tree.rs`), whose branches hold nothing but the way down. What a node at any zoom shows — a leaf's value, or its eight folded by the objects' majority — is read from the leaves over and under it, never kept. A write splits a leaf over its node, frees what is under it, and joins eight equal leaves into their parent, so one picture has one tree; a write is `<key, id>` (`space/hierarchy/morton.rs` `key_of`), the message the repository carries as well. It persists to the platform's storage — `localStorage` in a tab, `UserDefaults` on an iPhone, `SharedPreferences` on Android — as five columns per leaf tile — tile-local `x`, `y` and `z`, footprint side, value — under a key that carries the layout's version. One chunk — a slot of `world/chunk/table.rs`, built by `world/chunk/build.rs` — is one cube of its zoom, named by zoom and x, y and z (`world/chunk.rs`); the runs of the column it stands in are written once for every height its content reaches, and each chunk of the column bakes the part in its own heights.

## Coordinate System — PVF3

A position is **PVF3**, a Packed 3-Vector with Fixed-point arithmetic: 96 bits of fixed point, in two words, which unit cell and where inside it. A point and a displacement share the form, but only a displacement adds to a point.

```text
IVec, one 64-bit word                 FVec, one 32-bit word
┌───────────┬───────────┬────────┐    ┌──────┬──────┬──────┬──────┐
│     X     │     Y     │   Z    │    │      │  X   │  Y   │  Z   │
│  24 bits  │  24 bits  │   16   │    │unused│  8   │  8   │  8   │
│   63..40  │   39..16  │  15..0 │    │31..24│23..16│ 15..8│  7..0│
└───────────┴───────────┴────────┘    └──────┴──────┴──────┴──────┘
signed; X is cyclic, so the world     unsigned residues:
wraps at the longitude seam           1/256 of a unit on each axis
```

Each field is a **lane** of its word. The arithmetic is integer — a lane is computed as `i64` and packed back — so a coordinate is the same bits on every machine. There is one implementation of the packing, and it stays one because the shell reads coordinates through the kernel rather than decoding them itself.

The Web Mercator adapter (EPSG:3857) gives the unit its meaning: `1u` is a z24 cell's side, about 2.389 Mercator meters at the equator. Distortion and the tile-constant vertical scale live in the adapter. Placed objects stand on that lattice, an eighth of a unit at their finest, so the `f32` the store saves is exact on it.

## Terrain

Terrain is the baked, canonical geography — what the source says is there, changed only by a rebake — and objects are the world people author and change. That is the line between them, not dimension: terrain holds 3D things too, and an object may never move. Terrain is a **surface**, which is 2.5D, and the **decks** carried above it, which are not; the camera, the observation and the scheduler know neither, and read terrain through its chunks, its index and its query.

Terrain arrives as tiles: a raster of ground and one of elevation, padded so that bicubic sampling can reach past a tile's edge — the surface — and the tile's decks.

A ground cell is one byte, a look from OpenStreetMap Carto, the style openstreetmap.org draws with: `forest`, `residential_road`, `building`, each Carto's color, numbered in the order Carto paints (`spec/spec/cell/cube_type.json`). The numbers are one value space with the objects': the looks are a subset of what a placed block can be, so a block of forest stores forest's number, and what only an object can be — air, the placed colors, decor — lies past the byte. `0` is void and `255` unknown, so memory filled with `0xFF` reads as ground no bake wrote. Each look carries the kind the renderer draws it as — land, water, transport or a structure — and may stand a step off its height: water and a building one below (the catalog's `shift`).

The bake maps Overture's Buildings, Transportation and Base rows to a look as Carto maps the OpenStreetMap tag each class comes from, and paints them in Carto's order — its layers, landcover, water, highway areas, roads, aeroways, but buildings over the roads; within a layer its own sort, a larger area first or a road's `z`; then the source's id, so no row order decides a cell (`pxl-geo-bake overture/policy.rs`, `terrain/pipeline/feature.rs` `Rank`). The coastline comes from Base's `water`, and it alone says where the sea is. A cell is painted by what covers its center, and only what is seen from above: a tunnel, an indoor passage, a way not built or gone and a ferry's route are left off it. Why a cell is a Carto look and not channels of what the land is used for is [the ground is a Carto palette](./decision/20261005-the-ground-is-a-carto-palette.md).

A deck is a way carried over the surface — a bridge, a viaduct — kept as a line with a top along it, a width and the type its surface shows (`pxl-spec bake/deck.rs`): sparse, stacked where an interchange stacks, and nothing on a tile that has none. A chunk draws the decks over its columns as runs above its ground — a slab under each top, a pier down to the ground where one stands — and the index lane and the query count them as they count the ground. Their heights are Arnis's, structure by structure, from a tile and as much of the eight around it as its structures reach (`pxl-geo-bake deck/arnis.rs`, [as Arnis solves them](./decision/20261006-bridges-and-tunnels-as-arnis-solves-them.md)); why decks are lines in the terrain tile and not a lane or objects is [ways off the ground](./decision/20261001-ways-off-the-ground.md).

## The view

**The world is stored in its own shape, and the view keeps only what an observer can tell apart.** Looking farther shows nothing more: what reaches the screen is its area, a direction answers with the first thing it meets, and how far that is chooses how finely it is drawn. So what the view holds and does is bounded by the screen, not by the world under it — a mountain range, a city or two hundred towers stacked behind a ridge cost what the screen they cover costs. The record of how this was arrived at is [what is drawn is what the screen covers](./decision/20260928-what-is-drawn-is-what-the-screen-covers.md).

What the eye sees is quantized into **cells**, each the least the observer promises to tell apart: an angle, the same step of direction about the eye whichever way it lies, on the axes of the eye's own frame. A screen only calibrates it — the cell spans about twelve logical pixels at the optical axis, the reference view's (`pxl-spec space/camera/grid.rs` `SPLIT`, `target_cell_px`), never device pixels, which are how finely the screen is rasterized — and a perspective screen draws the same angle wider toward its edges. Inside a cell nothing is promised: drawn finer, exact, averaged or some other shape altogether, even its own value, as long as the approximation does not leak into a neighbouring cell. That is the one rule of detail: a piece of the world is refined where it contributes to some cell and its error, projected, crosses a cell's edge on either axis of the screen (`pxl-kernel world/view/observer.rs` `Lens::error`). Height is not special: storage may be anisotropic — terrain is a heightfield — but the rule measures what reaches the screen, on both axes alike. Ground seen at a grazing angle refines across the screen and not along its depth; a far pole thinner than a cell is folded into what is around it; what is hidden contributes to no cell and is never refined. τ is one factor on the split distance (`pxl-spec space/camera/grid.rs` `Tau`), and the zoom it chooses for a distance is `Tau::z_at`; τ = 1 is `SPLIT`, the finest cell everything is sized for, and a screen that asks finer is held there.

Between the world and the screen are seven concerns, each with its own bound and its own contract:

| Concern         | Asks                                           | Bounded by                           | Where                                                |
| --------------- | ---------------------------------------------- | ------------------------------------ | ---------------------------------------------------- |
| Storage         | a layer's records, in the layer's own shape    | nothing of the view                  | a layer; the bucket                                  |
| Query           | what is known at a point, or first along a ray | —; it holds nothing                  | `world/query.rs`                                    |
| Demand          | which records an observation requires          | 3 × 3 tiles a zoom (`REACH_TILES`)   | `pxl-spec space/layer/demand.rs`, `world/pyramid.rs` |
| Residency       | which records are held, and why                | its frames, asserted at compile time | `layer/residency.rs`                                 |
| Materialization | what is drawn from what is held                | the arena                            | `world/chunk/domain.rs`, `world/chunk/frontier.rs` |
| Scheduling      | which waiting work runs now                    | the turn's quota and the credits     | `io/sched.rs`, `io/policy.rs`                        |

**Storage** keeps each layer as its data is: terrain a heightfield in a 2D pyramid of tiles, placed objects one sparse octree. A tile carries, in its own lanes, the lowest and highest anything under it stands and which of its children exist — its **index** — so it is read the moment it is resident, with nothing over it required.

**The query** is the only place a layer's shape is known (`world/query.rs`). It answers a value at a point at a zoom, the extent of a region, or the first thing a ray meets, and it says what it lacked: an exact answer, a coarser one with the tile it would have needed, or none. It fetches nothing, holds nothing and keeps no copy; the pointer, the camera's clearance, the map center's footing and the view's own observations all ask it. Whether a ray goes down into a piece of ground is decided by the index, never by the surface a coarse tile draws, which averages away the peaks under it; a resident tile over the one asked lets a ray pass all it bounds at once, and one that is not resident lets it skip nothing. What a node of the world holds is the one question demand and materialization both ask (`layer/query.rs` `Query::bounds`: nothing known, empty, or inside a conservative bound with the finest zoom held under it), a cube or a column, answered by every layer from its own records and joined by the world's (`world/query/world.rs`), so no axis is special above a layer.

**Demand** is what an observation requires — the ground at the resolution its distance chooses — before it is any tile; the layer's storage maps the requirement to its tiles. The exact requirement depends on what the world holds, so it is estimated from what is known. The screen is cut into **patches** coarser than its cells (`pxl-kernel param.rs` `OBSERVED_PATCH`), each casting its corner rays, and what is required is the ground the patches reach, at each zoom whose band of distance some of it lies in, and what their rays read coarser than asked (`world/view/observation.rs` `ByFootprints`). Ground behind what the patches meet is required at no zoom; the fallback answers it. Where the rays met coarse ground they name what they lacked, and once it comes the patches over it are cast again, so the requirement grows to what the exact records make it. A tile of the requirement that is not resident is a miss, asked for at the zoom it wants, never a zoom at a time.

**Residency** holds a tile hard for the fallback, for the demand while it is demanded, or as cache; only the hard set is correctness. Every reader of the ground takes the fallback's answer where the exact tile is not resident. Its capacity is two asserts: one frame for everything held hard and one to land in (safety), and everything demand can name at once besides (convergence). A tile that arrives takes a free record or one nothing holds hard and the view does not read, the farthest first. The fallback is a law — every observation has an answer — kept here by holding the coarsest zoom the source supplies whole, all around the eye (`layer/residency/floor.rs`). The source declares that zoom: the published version in its metadata, the dev tile server at `/tile/source`. A region is unresolved until the fallback lands over it, then covered coarse, then covered as finely as it is observed, and it never goes back.

**Nothing is prefetched.** A view converges to exact on its misses alone — a miss, a coarser answer meanwhile, a fetch, a refinement — so a prefetch would be a policy and part of no meaning, bought only as latency where it is measured to be needed.

**Materialization** makes what is drawn. Its goal is the fallback's cover, all around, refined only where the screen's cells observe: each patch casts its corner rays, the stretch of ground between their hits is what the patch's cells touch, and a piece on it refines while its error crosses a cell (`world/chunk/domain.rs`). What is drawn is a **frontier** moved toward that goal in whole moves (`world/chunk/frontier.rs` `plan`): a piece is replaced by the pieces under or over it only once every one of them is built, and a move starts only when every arena slot it takes is free. Waiting for a tile, a build or a slot is always waiting with a coarser piece on the screen, never a hole. The roots, every chunk all around whose cells are the coarsest the reach draws, are ordinary chunks, priced as any piece. When the arena cannot hold the goal at all, the **quality** is chosen coarser: a pure choice from the view, the knowledge and the arena — the finest step that fits — coarser at once when the view needs it and finer a step at a time once there is room, read by demand and materialization alike so neither is finer than the other (`world/chunk/quality.rs`).

**Scheduling** orders the work the others make — a demand, a decode, a build — by what they gave it and nothing else. It may be any policy that keeps the budgets and never starves a demand still wanted.

Every number a person would tune — τ's target, the patch, the arena, the refinements in flight, the quality's step and wait, the retries, the per-frame caps — is named once in `pxl-kernel param.rs`, grouped by the policy it tunes, with its reason. What is derived from them stays where it is derived.

The map center is only the handle the camera is moved by, and plays no part in any of this.

## Bake

The bake is `pxl-geo-bake`, and it writes the same formats the client reads — XTP, xpute's tree, read where it lands rather than parsed — so nothing is translated on arrival.

```text
Overture rows and the elevation raster, from PostGIS
  → segment (each transportation row cut where a rule it carries changes, a way of one value each; what is not in use is no way)
  → deck solve (every bridge and tunnel reaching the tile, solved whole: its lines, and the ranges of ways they draw)
  → overture/policy (row → feature: its shape, the cell it paints, its rank)
  → ground (features rasterized into one look a cell, the coast's land over the sea)
  → encode_terrain (elevation + ground + lines → XTP)
```

Rasterizing is the bulk of it: polygons are filled even-odd with holes, and lines are drawn with a thickness kernel. Elevation is stored as signed 16-bit cell units under one Mercator Z scale per tile, which is what makes the bake, the terrain and the objects agree on what a unit of height is.

Every land tile is baked once into a versioned prefix. Open ocean is no object: a tile the bucket lacks is drawn from the one over it.

Those tiles are the leaves, z15, 2^15 of them around the world. A view from farther away wants fewer, coarser tiles, so the bake stacks coarser zooms on them: a tile at z covers the 2 × 2 tiles at z + 1 under it at half their resolution and is the same record as a leaf, named by its 2D Morton key written a digit a zoom (`space/hierarchy/morton.rs`), so that a tile's name is one digit shorter than its children's and a prefix of them. It is folded from the zoom below with no database in the loop — the ground by a majority of the four cells under it, the elevation by an average that reads past the tile's edge — and the fold is `pxl-spec`'s (`bake/fold.rs`), one function for the offline bake and the dev tile server. A tile with no object folds as what it stands for: open sea inside the source's coverage, since only a tile with nothing drawn in it is left unbaked, and void past it, where the terrain gives no surface and nothing is drawn (`pxl-geo-bake terrain/absent.rs`); a coarser one is the fold of those under it.

The dev tile server (`geo-bake demand/`) serves every pyramid layer the same way (`demand/pyramid.rs`): a leaf baked from the layer's source when it is asked for, a tile above it folded from the zoom below exactly as the offline bake folds (`pyramid.rs`), and nothing more than `--folds` zooms over the leaves, the one guard every layer shares. With `--baked` it bakes nothing and serves finished bakes from a directory as they are — at the published version, or the version a request names as a production build's does — so a bake is looked at before it is uploaded.

---

# The client

Two things run on the client: a kernel that decides and a shell that carries out. Where a piece of code lives follows from that, and so does which language it is written in.

## Crates and packages

One line decides a thing's language: **what shares `pxl-spec` is Rust**.

```text
                TypeScript                     Rust                              over pxl-spec, never in the browser
              ┌───────────────┐            ┌───────────────┐
generic kit   │ @xpute/core   │  mirrors   │ xpute-core    │  ┐
              └───────┬───────┘            └───────┬───────┘  │ xpute — xpute/ARCHITECTURE.md
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

| Crate / package | What it is                                                                                                                                                                                                                                            |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pxl-spec`      | The model: PVF3, the spatial hierarchy, the cell catalogs, the command numbers, the tile formats, the asset tables. One implementation, in Rust.                                                                                                      |
| `pxl-render`    | The view: the column runs, the mesher, glTF, the LOD criterion's reference, the atlas, the draw list, the shaders, the renderer.                                                                                                                      |
| `pxl-kernel`    | The kernel: the chunk scheduler, the layers and their stores, fetches and loads, the camera and sensors, the turn, the profiler.                                                                                                                      |
| `pxl-wasm`      | The kernel in a tab: one module that fixes the memory's front with linker flags, imports the clock and exports `interrupt`.                                                                                                                           |
| `pxl-native`    | The kernel on a device: a static library that takes its memory from the system, runs every call on a thread of its own, and exports `pxl_interrupt` and the surface's binding through the C ABI, and through JNI on Android (`android.rs`).           |
| `apps/web`      | The kernel's host and its shell, both apps': the host that opens the kernel, rings its turns and does the I/O it asks for — WebGPU in a tab among it — and the shell, React over the kernel's snapshot; and the Cloudflare Worker that serves it all. |
| `apps/native`   | The device app: the same host and shell run by Metro, what only a device does, and the native module that links the kernel.                                                                                                                           |
| `pxl-server`    | What both servers need before answering: the environment variables, the database pool, the gate over internal routes.                                                                                                                                 |
| `pxl-geo-spec`  | What the geo server and the bakes agree on about db-geo: table names, and what Overture's categories and divisions mean.                                                                                                                              |
| `pxl-geo`       | The world's server. Forward search — the one route that queries a database at request time.                                                                                                                                                           |
| `pxl-api`       | The application's server.                                                                                                                                                                                                                             |
| `pxl-geo-bake`  | The three offline bakes — terrain, POI, geocode — and, behind a feature, the dev tile server.                                                                                                                                                         |
| `pxl-tool`      | Hashes `asset/` into the bucket's shape and writes the asset tables `pxl-spec` and the shell carry; bakes the stars, the Moon, the brand.                                                                                                             |

The servers, the bakes and the asset ingest all compile the model, so they are crates. The Overture and elevation ingests put a provider's data into Postgres as the provider wrote it and touch no model, so they are scripts under `infra/geo/`. The host and the shell are TypeScript because neither decides anything about the world; what it needs of the model — catalogs, ABI enums, routes — is generated into `apps/web/src/**/*.spec.ts` from the same JSON the Rust is generated from.

Three rules hold the graph in that shape:

- **The view does not know the kernel.** `pxl-render` sees the model and a `FrameInput`, and returns what a frame cost.
- **The kernel does not know which bracket holds it.** A **bracket** is what gives the kernel a place to run: `pxl-wasm` in a tab, `pxl-native` on a device. It hands in the memory, the clock, the allocator's reading and where to say what stopped the kernel (`door.rs` `Bracket`), and the kernel asks everything else of the host as system calls the host runs after the turn — xpute's for its files — a bake's objects, the assets, what storage keeps (`io/file.rs`) — its own for text (`host.rs`). So the kernel also runs under `cargo test`, with no bracket at all. The one thing the target decides is how the renderer's calls are made: recorded for the page's WebGPU on wasm32, made through wgpu anywhere else (`pxl-render gpu/api.rs`).
- **Nothing is found by name.** Commands, cells, decor kinds (what the world grows on its own — trees, cars, boats, trains), character variants (the people and pets a player can be), assets and messages are numbers fixed in data. The words a message is shown in belong to the shell's locale files.

## Host, shell and kernel

The kernel is the process: it holds the world and decides everything about it, and it owns no device. The TypeScript around it is two things to it, and they are not the same thing. Its **host** is what an operating system is to a process — xpute's host — and its **shell** is the UI, a program that talks to the kernel as one process talks to another.

The host is most of `apps/web/src/kernel/`, the kernel's channel; the shell's end of it is `shell.ts`, `snapshot.ts` and `render/surface.ts` there. The host does five things:

- Opens the kernel: reserves the memory and instantiates the module over it in a tab, asks the app's native module for the linked kernel's memory on a device (`platform/core.ts`, `mem.ts`, `module.ts`).
- Rings the doorbell once per animation frame, when something asked for one.
- Forwards pointer, wheel, sensor and visibility events as commands (`control/input.ts`).
- Does the I/O the kernel asks for — fetches, image decodes, sensor watches, storage, a line of text laid out or drawn — and hands the bytes to blocks the kernel chose.
- Holds the GPU: in a tab it runs the renderer's recorded calls against WebGPU after each turn; on a device it opens the device and binds a view's Metal layer or `SurfaceView` to a surface, and the kernel draws into it directly.

The shell (`apps/web/src/app/`, `feature/`, and its end of the channel) is React over one snapshot the kernel writes, and every control is a command the kernel applies. It hands the kernel the surfaces it draws into, each named by a handle — a canvas transferred to an `OffscreenCanvas` once in a tab, a view on a device. Nothing in it computes a coordinate, a tile or a draw. `spec/client/kernel/abi/cmd.json` groups the commands by which of the two a command is between.

Both apps run this one tree of TypeScript. What differs between a page and a device is behind one import prefix, `@platform/`, which each app resolves to its own directory: `apps/web/src/platform/web/` for Vite, `apps/native/src/platform/` for Metro. Each module there exports what its twin does — a check fails the gate if a device's module lacks a name the page's has (`platform/conform.ts`) — and the device's modules reach the system through one native module of the app's (`apps/native/modules/platform/`). A device's kernel runs every call on a thread of its own, with a stack it sizes, while the caller waits ([a device's kernel on a thread of its own](./decision/20260926-a-device-s-kernel-on-a-thread-of-its-own.md)).

## The kernel's turn

The turn is xpute's, and so is the quota: the host rings once, the guest spends what it was given, and the falling edge says when to ring again ([xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md) is the mechanism). What pixelet decides is what fills it.

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
│   build · fetch decode · glTF · decor · store's save             │
└──────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
                          the falling edge  ──▶  wake_ms
```

The host submits one `WORLD_FRAME` before it rings — the viewport, the gap since the last turn, and how many reads this turn's credits allow to be out — and reads the completion ring after. Everything else here is the kernel's.

**The quota's policy** is the kernel's (`sched/tick.rs`, `QUANTUM`). It reaches the host in the kernel's directory (xpute's, `ipc/directory.rs`), which `boot` writes on the first turn for the host to read once.

What the policy says: the frame is the 25th percentile of recent gaps between frames, ignoring any gap over 50 ms. A turn is interactive when input arrived within `settle_ms`, or when the kernel asked for it on its last falling edge. An interactive turn gets that frame less a margin; every other turn gets `batch_frames` frames.

**The phases**, in the order that is their priority:

| Phase         | What                                                                                                                                                          |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `interaction` | input, camera damping, the environment (sun, moon), the view's frame of reference, the companion (the player's character at the map center); O(1), unbudgeted |
| `visible`     | the chunk scheduler's pass                                                                                                                                    |
| `content`     | what is about to be on screen; guaranteed `content_share` of the budget                                                                                       |
| `cosmetic`    | what only looks better for running (debug gizmos)                                                                                                             |
| `report`      | the profiler, last, so it sees everything                                                                                                                     |

**The tasks** run on `embassy-executor`'s raw executor, which the module installs. The frame is a task rather than a function so that the order above is its own, and no task depends on the order the executor happens to poll them in.

**The credits** bound what is out, and one place spends them, the request scheduler (`io/sched.rs`). A producer says only what it wants — the subjects it wants read, each with a rank, replacing whatever it said last turn — and never that an attempt should start, stop or be retried. What is wanted, with when a failed attempt at it may be tried again, and what is running are two tables of fixed size, so an attempt ending answers nothing about the demand that outlived it, a failure waits rather than asking straight back, and the demand of a camera that has just moved is representable however much old work is still draining. The scheduler is the mechanism and orders nothing itself: a serve policy (`io/policy.rs`) ranks each want from why it is asked for and where it falls in the view's order, says whether a waiting want is worth a running one's credit and how long a failure waits, and the scheduler keeps the credits, a total order and — while nothing moves — every want served, whatever the policy says. A model or a texture is a read like any other, spending the same credits as a tile rather than bidding against it from a second scheduler.

**The profiler** (`sched/profiler`, laid out by `spec/client/kernel/sched/profiler/profiler.json`) records what each frame and each task cost — pacing, per-phase time, GPU ask, heap gauges, LOD pressure, draw census, per-job waits — into rings the shell reads in place (`PROFILER_READ`, `PROFILER_TAKE`) and prints as a report or exports as a Chrome trace. The scheduler never reads them.

## The channel

The host and the kernel share the two rings xpute describes, of four-word messages with 4 KiB payload slots: 2048 for what the host submits — its own commands and the shell's, which go through it — and 4096 for what the kernel completes (`mem.json`). The completion ring holds everything one turn leaves, since the host reads it dry before it rings again; a turn that has too little room left for what one more command might raise leaves the rest of the submissions for the next turn, in order, and asks for it.

The commands are `spec/client/kernel/abi/cmd.json`'s, numbered from `0x10` past xpute's own (`xpute/spec/abi/cmd.json`, the fetches among them): each one's number and the fields of the packet it carries, from which both sides' readers and writers are generated, so no field is found by its position anywhere else. A packet is an XTP tree, and anything larger than a slot crosses in a heap block the kernel hands out.

The kernel also posts messages nobody asked for: a fetch to start or cancel, a model or image to decode, a sensor to watch, a console line, a new snapshot. Those are its signals.

The world's revisions (`world/change.rs`) say what is stored has changed: one for the objects, and one per chunk, which a write to a cube it draws moves (`world/chunk/source.rs` `written`), so a chunk reads one revision rather than summing what it draws. They move on a write and on nothing else: the eraser's previews are a chunk's sources, not a change to the store. A reader compares a revision with the one it last read; nothing is called when one moves.

## The memory map

The client's state is one memory, reserved once and never grown: in a tab a `WebAssembly.Memory`, 1 GiB on the desktop and 512 MiB on a phone, `initial == maximum`; on a device one region of 512 MiB the library takes from the system on the first call (`pxl-native memory.rs`), zeroed and committed tile by tile as it is touched. Since it never moves, a ring keeps its words and a table is read where it lies for the kernel's whole life — the way Figma's editor core holds every object as a range inside one pre-allocated array. A phone's tab gets half because JavaScriptCore counts the whole reservation as the tab's JavaScript memory, committed or not.

Every address the kernel names is an offset from the memory's first byte (xpute's `mem/base.rs`), which is address zero in a module and the region's start on a device. So the map below holds on both, and the shell reads the same numbers either way. On a device the stack and the library's own data are where the system put them, outside the region, so the two ranges the map keeps for them below — `.stack` and the linker's — stand empty there.

```text
address        bytes                    range                  placed by
──────────────────────────────────────────────────────────────────────────────────────
0              1 MiB                    .stack                 rustc (-z stack-size, --stack-first)
1 MiB          36 KiB + 68 KiB          .ring.desc             mem.json (door/layout.rs)
2 MiB          8 MiB + 16 MiB           .ring.slot             mem.json (door/layout.rs)
32 MiB − 4 KiB 4 KiB                    the directory          mem.json (door.rs)
32 MiB         ~0.7 MiB                 .rodata .data .bss     wasm-ld (--global-base)
               ↑ D: __data_end, rounded up to 4 KiB (PAGE_BYTES), about 32.7 MiB
D              FRAMES terrain frames    .fixed.terrain         the kernel (mem/fixed.rs)
               one mesh bake            .fixed.bake            the kernel (mem/fixed.rs)
               the chunk table          .fixed.mounts          the kernel (mem/fixed.rs)
               MAX_LIVE_BLOCKS          .fixed.blocks          the kernel (mem/fixed.rs)
               the GPU stream           .fixed.gpu             the kernel (mem/fixed.rs)
               the host stream          .fixed.host            the kernel (mem/fixed.rs)
               the profiler's rings     .fixed.profiler        the kernel (mem/fixed.rs)
D + BYTES      the rest                 .heap                  the allocator
512 MiB / 1 GiB                         end
──────────────────────────────────────────────────────────────────────────────────────
```

The map is `pxl-kernel mem/section.rs`'s, over xpute's sections. The stack is first so that an overflow wraps below zero and traps — there is no MMU, so the wraparound is the only guard — and the kernel asserts at boot that the linker's `__stack_high` is where `mem.json` says. The rings and the directory's tile sit at fixed addresses, asserted apart in `door/layout.rs`, and `build.rs` emits `--global-base` from the same numbers, so the linker cannot place `.data` over them.

`.fixed` is what the kernel places once, sizes at compile time and never frees: the ranges in address order, each a page-rounded span, the sizes in the table being what each is derived from, never a number kept here (`mem/fixed.rs` `RANGES`, `BYTES`). The heap is the rest of the reservation, most of it on either. The largest range is the resident terrain (`layer/terrain/fixed.rs`), one frame a page, as many as residency's capacity (`layer/residency.rs` `FRAMES`); which page lies in which frame is residency's, not a coordinate's. A page held without a frame — the void a source lacks at its top — holds no frame (`layer/residency.rs`), its body its own and none of the range's. A thing belongs in `.fixed` when it is in the kernel's own format and its size is settled at compile time; the heap is left for what is asked for, refused and given back, and for what another format sizes — a glTF, a PNG — whose bound nothing here can predict. Every range is reached through `mem/fixed.rs`, which checks what is asked for against the range's end, so nothing written through one lands in the next.

`.heap` is everything else, under xpute's slab over buddy. A `String`, a chunk's runs and a fetch body all come from here. A refused request is a wait, not a failure: a chunk's build, its decor and a model keep their place and ask again after the heap's next release (`world/chunk/build.rs` `refuse`), and the client does not die.

The numbers — page size, stack, ring addresses, `--global-base`, the two reservations, the heap a phone is guaranteed (`MIN_HEAP_BYTES`) — are one file, `spec/client/kernel/mem/mem.json`, read by the kernel's map, by `build.rs` and by the shell's reservation, and asserted at compile time and at boot.

What the renderer reads each frame is laid out as **column lanes**, structure-of-arrays — not the packed-word lanes of a coordinate. They are terrain planes, column runs, a mesh per chunk, the chunk table, decor instances, the object store's records, and every registry. A table whose columns move together is one struct of lanes with one length. Slots are dense, and an index is told when its slot moves — except in a handle table, where the slot is what the other side holds, so it stays and is reused under a new generation. A lane set has one owner; a consumer borrows it for one call. The JavaScript heap holds only glue: React, the WebGPU objects behind the kernel's handles on a tile, and a file's bytes until the kernel reads them.

---

# The engine

Everything here serves one meeting. The ground arrives as a dense raster for a whole tile; what the player places is one sparse octree. The two are stored apart and travel apart, and they meet once, when a chunk is built. What comes out of that meeting is one set of spans, one per stretch of a column that holds a single material — the **runs** the rest of this half is about. The pointer never reads them: it asks the records, through the functions the paint reads them with, so it meets what is drawn.

## Chunk pipeline

This is the meeting.

```text
terrain tile ──▶ sample (mesh/paint/terrain) ─────────────┐
                                                          ├─▶ column runs (mesh/paint/column)
object store ──▶ folded to the chunk's cells (fold_at) ───┘          │
                                                                     ▼
                                                           mesher (mesh/chunk_mesher)
                                                           one mesh a chunk
```

In the store, `0` is VOID, `128` is explicit AIR — a leaf that erases what a lower layer would show — and `255` is UNKNOWN. Any other value is at once a placed color, a terrain pick or a decor kind, and its id in the one global palette (`pxl-spec cell/`).

A chunk's terrain is the tile whose cells fold into its own, sampled a column a cell of that tile: its look and its height, its decks standing off the column, a bore taking the air over its floor out of it. Past the leaves the leaf's sample is refined, each column the leaf column it lies in, so nothing finer than the ground is invented (`ChunkSample::refine`). Its objects are the store's, folded to the same columns by majority (`Store::fold_at`), a cell of apron around so the faces at its edge see what is beside them. What is laid later wins a cell, as the layers are painted: the ground, then decks, then what is placed, air included. A chunk records the revisions it was built from — its terrain tile's, and its own object and decor revisions — and is not rebuilt while they stand.

**The column run set lasts as long as its bakes.** A column is written bottom to top, and a run extends while the material holds. A chunk's columns, its own and an apron of one, become their runs in a heap block of exactly the size the writer counted (`pxl-render mesh/run.rs`), two sets at most at once; a column's runs are what stands in it, every height up to the top of the world, so no fixed range is sized for every column. The chunks of one column, one above another, bake their parts of one set, and then the set is let go: the meshes are what lasts, and runs kept for a mesh asked later would be a cache of a cache.

**A mesh is its boxes, and lives on the device.** Each chunk's mesh is held in a slot of the fixed buffers the renderer keeps (`pxl-render gpu/arena.rs`). A slot is room, not an address: any chunk takes any free slot of its pool (`world/chunk/mesh_slot.rs`), keeps it while it is drawn, and gives it back the moment it is not, so a slot never holds what is off the frontier and a drawn mesh outlives its tile's record. The slots are derived, a residency of what is drawn, and apart from the one bake's workspace in `.fixed.bake` that a mesh is made in. A mesh past a slot fills as many as it takes, any free ones chained, all held at once or none: it is never cut, since a box left out is a hole, and never baked coarser to fit, since which chunks are drawn is the quality's to choose and a slot's size is no quality. How many slots a mesh fills is counted by the frontier and the quality policy, which alone decide what the arena has no room for. A mesh holds nothing per vertex — a box is its corner, size, color, texture layers and face mask — and the chunk pipeline binds it as storage for its vertex shader to unfold into a cube (`frame/shader/chunk.wgsl`).

**The scheduler** (`pxl-kernel world/chunk/table.rs`) is one table with a slot per mounted chunk: its role in the frontier, flags, distances, bounding sphere, pending-since, retained bytes. A chunk is a piece drawn, or one built for a move of the frontier and not drawn until the move is whole; each holds a slot of the arena or has one held for it, so the table is never larger than the arena. A tick **refresh**es each against the frustum; **recheck**s whether its layout is of its sources' generation; takes the frontier's step (`materialize`) — the moves whose pieces are all ready made, and new ones started only where the arena has room for all they take; and **commit**s a layout whose mesh is in hand.

**What to build next** is one priority. A chunk build writes its column's runs and bakes its mesh, one bake open at a time; the priority (`world/chunk/table.rs` `priority`) puts the chunk holding the bake first, then what the view sees nearest the eye, then what it does not, so the view fills out from the eye. Builds run in the build task on what the tick left of the quota.

## Rendering

Each tick ends with a **draw list** (`pxl-render frame/draw.rs`). In it: the chunk meshes at the weights the LOD blend chose; decor as instanced draws, one model at many placements; the actors — the companion and other characters, each with a joint palette for skinning; the location puck; the billboards, labels and bubbles drawn flat to the camera; and the debug gizmos.

The kernel writes the frame's uniforms into the stage lane (`frame/stage.rs`) and calls `render_frame` (`gpu/renderer.rs`) with a `FrameInput`: the list, the environment, the models and their skins, the billboard sheet (the one texture every label is drawn into), and the surfaces. The renderer draws into every open surface — the world's and the character picker's display case — and reports what the frame cost and whether it wants another turn. It decides nothing.

Its calls are WebGPU's, and how they are made is the target's (`gpu/api.rs`). A module cannot reach WebGPU itself, so on wasm32 they are records the page makes once the turn has returned (`api/record.rs` ↔ `apps/web/src/platform/web/webgpu.ts`); on a device the same calls are made through wgpu as they are asked for (`api/device.rs`). A call carries numbers and the address and length of what it names, the same words to both, and every GPU object is one of xpute's handles: 2^15 slots for objects, 8 for surfaces.

Shaders and pipelines are the kernel's (`frame/shader/`). A pipeline is a record of words — shaders, vertex lanes, bind group layouts, primitive, depth and blend state — that the page turns into a WebGPU descriptor and a device into wgpu's. `cargo test` compiles every shader with naga, the WGSL compiler, and checks each record against its shader. What a shader shares with the host has no room inside it — 4-byte scalars and `vec4`s, and arrays and structs of them with no gap — so the host's words and every driver's layout agree, and the tests hold every shader to it, since a phone's Vulkan read a padded field and a matrix array in storage its own way ([a matrix the driver reads its own way](./decision/20260927-a-matrix-the-driver-reads-its-own-way.md)). A uniform block is bound as `array<vec4f, N>` and each field read from its word by a function of its own, never by one that builds the struct back, with which Safari's WebGPU lost its GPU process.

Two choices outlive a frame. Depth is reversed float, near at 1, so precision falls where perspective needs it and far coplanar faces do not fight. And on device loss the blocks are still in memory, so everything is re-uploaded from the addresses it already had.

The **view** (`pxl-kernel world/view/frame/pose.rs`) is the observer's pose — the eye, and the turn from the world's axes to the camera's — and the matrices made from it. The world stays in its own axes, in doubles whose X runs on past the seam, a canonical point entering them on the lap nearest the eye (`world/view/frame/coord.rs` `to_world`); what turns is the observer, and in its own frame the eye is the origin. The pose is made from one camera (`control/camera/controller.rs` `Camera`): an eye and a look in spherical coordinates about the world's Z, θ = polar from straight down and φ = azimuth from north toward east, which every controller moves and nothing else keeps a copy of. The free camera (`free.rs`) is first person: it turns about its eye and walks it. The orbit (`orbit.rs`) is third person: it turns about a point r = `dist` ahead of the eye along the look, and its distance is the only state it adds. That point is the map center — the eye itself first person, and the tracked eye while a tracker (`INPUT_POSE`) holds the pose — derived from the camera and never kept. It is the character's place, half a cell over the floor it stands on as a cell's center stands over its ground (`Cell::center`), in either person. A change of person moves the camera about the map center and never the map center; a navigation command puts the camera so the map center is where it says, once the ground there is known exactly (`control/navigation/destination.rs`). Nothing is kept against an origin of its own: every draw uploads its point less the eye, subtracted in doubles before it is narrowed (`pxl-render frame/draw.rs` `translation`), and a double holds the whole lattice with its fraction exactly. The matrices are doubles, written the moment a damping step moves, and read by the scheduler, the renderer, placement and the pointer's ray.

**LOD** is which chunk is drawn. A chunk's cells are the camera grid's, and it splits into its eight children while their error, a cell on every axis, crosses an observed cell (`world/chunk/domain.rs`, from `pxl-spec space/camera/grid.rs` `Tau::cell_at`) and something under it is held finer than its cells (`Bounds::finest`, read off what is stored): the ground stops at a leaf's cells, z24, and objects at their finest held. One cell of the grid is one thing drawn, and what lies inside it is folded by its layer: a cube's records one zoom finer (`pxl-spec space/layer/lattice.rs` `Lattice::fold`), as a coarse cell below says. The store keeps the objects folded a zoom at a time as it is written (`pxl-kernel layer/object/store.rs` `fold_cell`), so an object's cell and the ground's are one zoom wherever both have data. A chunk hands off when one of its cells would project to about twelve pixels (`pxl-spec space/camera/grid.rs` `target_cell_px`), so the world stride is `1/2/4/8/16/32/64` and on. What is folded is a summary, not what is held: nothing in a folded cell is struck, hidden or cut by a preview (`pxl-kernel control/interaction.rs` `folded_at`, `world/chunk/build.rs` `draws_as_held`), and nothing in one is pointed at.

A decor kind is drawn as a model down to its own size on screen and as a sprite below it (`pxl-kernel param.rs` `DECOR_MODEL_PX`, `DECOR_SPRITE_PX`), apart from the grid.

Culling uses the exact union of the mesh's bounds, which survive GPU eviction and are invalidated with their sources. Until a chunk has been measured, its bounds are where in its cube anything stands as far as the world's query says (`layer/query.rs` `Query::bounds`), the ones the rule chose it by, and a chunk the camera stands in is seen whatever the frustum says of its center.

**A coarse cell** (`lod_step > 1`, `mesh/chunk_mesher.rs`) is the `lod_step²` columns under it summarized — a cell of the grid over its records one zoom finer — and the ground and what stands on it are summarized apart. The ground is a height field, so only its footprint coarsens: one run from the lowest floor to the columns' mean top, in units, its color the mean of their top surfaces and up to four of their texture layers, a full run that merges and culls faces like any other. What objects add above it is a volume: `lod_step³` voxels to one box whose fill is coverage over the cube, kept in its children's worth and shrunk about the cell center by the cube root of the fill so its volume is the fill's, on an inset path with all four creases. Heights are units at every zoom, so no ground is ever a cell part full.

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

The orbit is three controls — `dist`, `polar`, `azimuth` — and nothing else writes them. Terrain may ask one question, in `control/camera/orbit/terrain_gate.rs`: would this state put the eye inside the ground. The gate is a function of the three controls and the map center they turn about (`allowed_view`), and it sits in `ask` (`control/camera/orbit.rs`), the one funnel every input path goes through, so a refused move never leaves the controls and the drawn camera disagreeing. `Settings > Camera` picks the answer, `stop` by default: `through` ignores terrain, `stop` takes a control as far as it can, `climb` grants the move and raises the drawn pitch instead where the eye gets there through the air, and is held as `stop` where it would pass through something solid. A pan moves the map center to a new XY for either camera, at the orbit's distance third person and the default distance first, by the lowest way there (`pan_to` in `control/navigation/support.rs`): along the way at its height, and where something solid stands across it, over it, tried again from the top of what it struck (`first_hit`, `open_from` in `world/query.rs`) — a height the center's own column must reach before its ceiling does (`ceiling_over`), or the pan is refused. Then it stands on the floor under that height. Under the open sky it goes over anything; under a roof, a deck or a cave's rock it keeps to its layer, at any zoom. Off a leaf the height stays unknown until the surface gives it. A tap names the surface it struck, and the center stands across that face on the floor under it. What the pointer picks and edits — the tool held, what is under the pointer, what a tap does — is `control/interaction.rs`'s, apart from the camera; the camera's input passes it hover and tap.

Every setting is one entry of one schema (`spec/client/kernel/shell/setting/schema.json`): default, kind, section. The store keeps only the user's overrides, and the settings sheet renders its rows from the schema. The nightly entries — the camera against terrain, the structures — are marked `dev`: a development build offers them, and a published one shows no row for them and holds each at its default (`apps/web/script/check-bundle.ts` holds the rows to that; the kernel's settings store, the values). The debug section — the gizmos, the profiler panel — is in every build, since a release is what it most needs to measure.

## Billboards and POI

Places are a second layer of the same pyramid, folded by another rule. A POI tile has the same name and zoom as the ground tile it lies over, cut in a lattice sixty-four times the ground's cell, a chunk's column (`POI_LATTICE`): a zoom's ground cell covers three to six pixels, so a cell of it is never narrower on the screen than the widest badge, and badges of one zoom cannot cover each other. A tile holds what is there and no choice made of it: a count per cell, whether that count is all of them or only as many as the source gave, and of each kind among the cell's named places the one of least id — a kind the category its search result shows, named as the badge table every source maps its own categories onto names it (`spec/spec/bake/poi_badge.json`), an id the source's own identity for the place as it gives it, and its names by language (`pxl-spec bake/poi_tile.rs`, one XTP tree as every baked tile is). Where the ground folds by majority, places fold by sum and by the least id of each kind; the tree stops where a tile already says all of a child's ground, so an empty stretch is never asked for (the tile's `Occupancy`, `pxl-spec bake/format.rs`). Which POI tiles are fetched and kept is the pyramid's, each zoom asked over the distances where its cells are the grid's (`pxl-kernel world/pyramid.rs`, `layer/poi.rs` `LATTICE`), so there is no POI-specific residency, schedule or key; its observation is cut where the ground is drawn coarser than the coarsest zoom a place is shown at, since past it the layer draws nothing. The layer is a setting, off unless turned on.

Placement (`world/chunk/place.rs`) reads the chunks the terrain draws, as decor does, each place once: each chunk from the tile that says its ground — its own, or a tile over it that marks it none — whose cells under the chunk are its badges, as the pyramid folded them. What a cell is shown as is the client's to choose, so a change of it is no new bake: from z14 a cell of one place is that place and a cell of more its count, a cluster, as a map's markers are; at z12 and z13 a cell is shown only by its most prominent place where that is a station, by OpenMapTiles' class rank (`layer/poi.rs` `rank_of`); coarser, nothing is shown or asked for (`param.rs` `POI_ALL_ZOOM`, `POI_MAJOR_ZOOM`). The zoom is the screen's at the cell — the units a logical pixel spans at its distance from the eye — not the zoom its ground is drawn at, so the quality that coarsens the ground does not move it. A badge stands at the center of a leaf cell — its place's, or its cell's middle's for a count (`Cell::center`, `pxl-spec space/layer/tile.rs`), on what a ray down through that center meets first, ground or a placed object, at the zoom the view draws there (`world/query.rs` `first_hit`), and where nothing is drawn there is no badge. Badges are drawn farthest first; none is hidden for overlapping another, since a badge stands for the places under its cell and the lattice keeps a zoom's badges apart. A tap on a badge is the badge's before the ground's (`world/chunk/place.rs` `tap`), and takes the view to where it stands.

Badges are one instanced quad (`scene/billboard/field.rs`) drawn from one atlas (`pxl-render asset/atlas.rs`, one texture the badges are packed into shelf by shelf): anchor, size and sheet rectangle per instance, with the fit to screen pixels and the badge's anchor pinning in the vertex shader. Occlusion is all or nothing per badge: the pass (`frame/shader/badge.wgsl`) binds the world's depth as a texture, reprojects several points up each badge through it, and discards the badge when most are behind terrain. The same pass writes the fragment's depth (`@builtin(frag_depth)`) from an upright twin of the quad, so billboards sort against the world along world-up. Chat bubbles are drawn by the same pass, with a sprite where a badge has text.

An entry of the sheet is drawn on the GPU as well, once, when what it shows changes (`pxl-render gpu/sheet.rs`, `frame/shader/sheet.wgsl`): its shapes — rounded rectangles, circles, rings, a bubble's tail — as signed distances, and its text as lines the platform drew, into a texture of the entry's size that is then copied into its rectangle. Text is asked of the platform a line at a time, so kerning, joining, bidirectional runs, fallback and emoji are the platform engine's on every target: the kernel asks for a text's lines and widths and then for each line's pixels (`pxl-kernel text.rs`), the tile answering with its canvas, an iPhone with CoreText and Android with its `Canvas` ([the sheet on the GPU, and text by the line](./decision/20260926-the-sheet-on-the-gpu-and-text-by-the-line.md)).

## Catalogs and assets

Everything a build ships is decided at build time, as JSON under `spec/` grouped the way `crates/` is. `pxl-tool` writes a Rust module from each file and, where the shell reads it, a TypeScript one; `deno task spec` writes them and `check:spec` fails when the tree disagrees with the JSON. That covers the commands and their packets, the cell catalogs, the memory map, the GPU API's enums, the profiler's layout, the settings schema, the decor and character catalogs, the routes and the published versions.

A catalog is never sent. Both sides compile it, so a message carrying it would be a program telling itself what it already knows. Where bytes do cross — a published tile, a saved blob — the layout is the model's and the version is in the envelope: the path, or the storage key.

- **Cubes and decor kinds** are numbers in `spec/spec/cell/` (`cube_type.json`, `color.json`, `decor.json`), and the number is what the object store writes.
- **Models** are named by path under `asset/` in the catalog that uses them — each decor kind's variants, a model and a skin each (`pxl-kernel world/chunk/decor/kind/`), each character group's `keys`, `models`, `skin` and `size` (`spec/client/kernel/scene/character/variant.json`). A path resolves to a bucket key and a measured extent through the asset table `pxl-tool`'s ingest writes (`pxl-spec asset/ingested.spec.rs`, `apps/web/src/catalog/asset/ingested.spec.ts`), naming every model, skin and texture by its own path with a hash of its bytes in the stem — `asset/actor/kenney_cube_pets/beaver-20f91809e69c.glb`, the way a bundler names a chunk — and measuring each glb's bounds. Nothing imports a model or an image; the client fetches a key, the same key from `./out/asset` in development and from R2 in production.
- **The texture pack** is one table (`pxl-spec asset.rs`: which image a cube type wears on which face) and one WebP sheet in the bucket, from which the client fills its layer array one layer at a time. A cube's color is `color.json`'s, what chunks coarser than a leaf's cells draw; a pack decorates it.
