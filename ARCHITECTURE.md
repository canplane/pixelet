# Architecture

A spatial engine for turning real-world geographic data and player-built objects into one voxel-style 3D world.

The engine is split into a pixelet-specific domain layer (**`@pxl/shared`**: coordinate system, cell/terrain model, spatial hierarchy, bake wire format, the record layer) and a project-agnostic utility layer (**`@xpute/core`**: collections, allocators, codecs, and the binary tree protocol those formats use) — see [Repository Structure](./README.md#repository-structure).

Three things run through every section below, and are the shape the client has had since September 2026: the world's state lives in **one linear memory** ([Memory](#memory)), the frame's work runs as **one cooperative tick** with one budget ([Scheduling](#scheduling)), and what the client measures about itself is a **read-only ledger** that never feeds back ([Scheduling](#scheduling)). The destination those were built for — a Rust core in WASM drawing through WebGPU, with three.js gone — is decided and under way piece by piece — the chunk mesher is the first Rust, in `crates/`; see [issue/20260912-why-the-destination-is-wasm.md](./issue/20260912-why-the-destination-is-wasm.md).

> Earlier iterations explored a decentralized P2P/WebRTC runtime (`xpute` as a peer-symmetric IPC substrate) and a Rust/WebGPU rewrite of the spatial/bake stack. Both were parked in favor of a plain client-server MVP; the WASM/WebGPU direction is decided again as of 2026-09-12 (above), to be built on the MVP's state model rather than resumed from the April checkpoint. Their last states are preserved on `checkpoint/20260107-float`, `checkpoint/20260405-xpute`, and `checkpoint/20260409-wasm-aborted`.

---

## Coordinate System — PVF3

PVF3 (Packed Vector Fixed-point 3D) is a 96-bit fixed-point coordinate:

```text
PVF3 = { i: IVec (64-bit), f: FVec (32-bit) }
```

- **IVec** packs three signed integer lanes in a `24/24/16` X/Y/Z layout. X is cyclic so the canonical world wraps at the longitude seam.
- **FVec** packs three unsigned 8-bit fractional residues, giving `1/256` resolution within one canonical unit (10 bits until 2026-09-03; narrowed so the coordinate and the render payload below share one lattice).
- Arithmetic is deterministic across platforms through BigInt.
- The bit fields contain no guard lanes: IVec uses its full signed `24/24/16` ranges, while FVec uses all 8 bits per residue lane (the top byte of its 32-bit word remains unused).
- The EPSG:3857 adapter assigns physical meaning: currently `1u = 1 L1 block ≈ 2.389` planar Mercator meters at the equator. Geographic distortion and page-constant vertical correction live in the adapter, not in PVF3 itself.

Placed-object render payloads (`SUB_CELL_SCALE` in `render/layer/pipeline/volume.ts`) quantize their sub-cell offset and size to the same `1/256` of an L1 block, so a placed position round-trips between the coordinate and the mesh without rounding. A `Placement` row's page-local `f32` ([Records](#records)) is exact on that lattice too.

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

Extent is a linear range on one packed lane (`RANGE` in `indexer.ts`), not an inherent cube — L0–L5 are a pure scale/addressing tier, dimension-agnostic by construction. Whether a level ends up populated on one axis, two axes (a Tile), or all three (a Volume) is a shape-axis decision made by whichever layer uses that level, never a property of the level itself; see [Layer, space, and shape](#layer-space-and-shape) below. `N³`/`N²` notation elsewhere in this document describes a specific layer's _usage_ of a level (e.g. the renderer treating L2 as a `64³` Volume), not the level's own definition.

L5 is reserved, not active code today (`L5_SHAMT` stays commented out in `indexer.ts`). Once implemented, its Z lane collapses — Z is only 16 bits wide, narrower than `L5_SHAMT` — so L5 can only ever address X/Y, regardless of what any layer does with it.

Objects have no server persistence yet — the store is client-local. It keeps a palette per L2 chunk and one sparse tree per occupied `64³` band whose leaves are palette indices, all as lanes in the core memory (`module/world/object/store.ts`), with a derived cell index for occupancy and hit tests. It persists as one `Placement` record per L3 page ([Records](#records)), which is the form a server would receive too.

One chunk node (`module/world/chunk_node.ts`) owns one L2 XY column and composes however many signed 64-cell L2 Z bands its content touches. One L3 page therefore contains 256 possible L2 columns.

### Layer, space, and shape

Three independent axes describe any piece of world data, and none of them implies the others:

- **Layer** — which domain owns the data: `terrain` or `object`. See [Unified World-Layer Pipeline](#unified-world-layer-pipeline).
- **Space** — which scale tier addresses it: the L0–L5 hierarchy above. Purely a size/addressing concept; a level's own name (Page, Chunk, Anchor, ...) says nothing about what shape of data lives at that tier.
- **Shape** — how the data itself is structured: a **Tile** is a 2D/2.5D raster (terrain's own storage form, currently sized at L3/Page granularity); a **Volume** is a full 3D structure (the object octree's storage form, and the renderer's own temporary composited field at L2/Chunk granularity). A **Cell** is just a position/address — no size or dimensionality of its own, no payload; any space level's `addr`/`idx` is a Cell. A **Pixel** is a Cell plus its 2D payload — the atomic sample inside a Tile. A **Voxel** is a Cell plus its 3D material payload — the atomic sample inside a Volume. A **Band** is one materialized `64³` Volume: the renderer's concrete in-memory instance holding a single occupied Z-band of an L2 column (see [Temporary dense bands](#temporary-dense-bands)) — renderer-only working storage, not a hierarchy level or a persisted format.

Terrain today is Tile-shaped data addressed at Page granularity; objects are Volume-shaped (sparse tree) data kept per Chunk band, with the renderer composing an L2/Chunk-sized Volume as their shared transport/render envelope (see [Temporary dense bands](#temporary-dense-bands)).

| Layer                               | Space level           | Shape  | Concrete type                                    | Atomic unit |
| ----------------------------------- | --------------------- | ------ | ------------------------------------------------ | ----------- |
| terrain (source)                    | L3 Page               | Tile   | `Tile` / `TileStore`                             | Pixel       |
| object (source)                     | L2 Chunk / `64³` Band | Volume | palette per chunk, sparse tree per band          | Voxel       |
| terrain+object (renderer, composed) | L2 Chunk              | Volume | `ChunkVolume`, sliced into `VoxelBand`s ("Band") | Voxel       |
| — (any)                             | L4 Anchor             | none   | `AnchorRegistry`                                 | —           |

## Memory

The client's state is one fixed `WebAssembly.Memory` — 512 MB on the desktop, 256 on a phone, `initial == maximum`, never grown, since growing would detach every view and a resizable buffer is slower to read and refused by WebGL — cut into **sections** with fixed starts and sizes (`render/layer/pipeline/core_pool.ts`, [issue/20260913-sections-of-the-core-memory-and-what-full-means.md](./issue/20260913-sections-of-the-core-memory-and-what-full-means.md)), none borrowing from another: **module**, the first 16 MB, for the Rust module's own stack, data and heap (where a wasm32 linker puts them; its allocator is `crates/pxl-wasm/src/arena.rs`, and the module states its size so the map is checked at instantiation), then each its own buddy allocator (`@xpute/core/ds/buddy.ts`) over its range: **fixed** for what is allocated once and never freed (the change log, the scheduler's lanes, the mesher's scratch; it refuses rather than spills, so a shortfall fails at startup), **page** for resident terrain and POI pages, **mesh** for what churns as the view moves. Blocks are what typed-array views are taken over. Everything the canvas reads each frame lives there as **lanes** (structure-of-arrays typed views): terrain pages, voxel bands, column run sets, baked meshes, the chunk table, decor instance matrices, the POI index, the badge atlas's slots, the object store's records and its derived index, and every registry that used to be a `Map` or a `Set` of objects (`IndexedMap`, `PageTable`); a table whose columns move together is one `Lanes` (`@xpute/core/ds/lanes.ts`), and a block with several spans is cut by `carver`. The allocator itself allocates nothing: its free lists are threaded through the free blocks. What stays in the JS heap is glue — the `Mesh`, `BufferAttribute` and `Group` three.js requires, React, and the handles that name a slot — measured at 30–40 MB of objects on a loaded page.

The rules that came out of that move ([issue/20260912-linear-memory-what-is-left.md](./issue/20260912-linear-memory-what-is-left.md)): a lane set has one owner and a consumer borrows it for one call, keeping only what it wrote; a slot is dense and its handle is told when it moves; two owners means a refcount. Past its range the page and mesh sections still hand out plain regions the collector can reclaim, counted per section; a bake the mesh section refuses keeps its place and asks again next tick, and the mesh section's fill is a second trigger for reclaiming off-screen chunks. Beside the big memory sits the **little memory** (`module/world/sched/slab.ts`): fixed-size transferable slabs, made on demand and released when idle, for work that will leave the thread — a `WebAssembly.Memory` cannot be transferred, so a worker job travels in a slab and comes back the same way.

## Terrain Data and Streaming

L3 pages are the primary terrain streaming unit, storing terrain's Tile-shaped data. The client requests pages around the current center and keeps terrain in its source-appropriate 2.5D form: dense categorical ground rasters plus a padded elevation raster reconstructed with bicubic sampling.

Each page carries four ground channels rasterized from Overture Maps geometry (Buildings/Transportation/Base, plus coastline derived from Overture's Base `water` theme):

| Channel     | Contents                                            |
| ----------- | --------------------------------------------------- |
| `LAND`      | Ground surface type (bare, green, sand, rock, etc.) |
| `WATER`     | Inland water body type (river, lake, wetland, etc.) |
| `STRUCTURE` | Buildings and constructed surfaces                  |
| `TRANSPORT` | Roads, paths, railways                              |

Sea cells use `SEA_FLAG = 0x80` to distinguish sea from ordinary land while retaining the byte-sized channel representation.

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

Polygon rasterization uses even-odd fill with hole support. Linear features use a thickness kernel. Arena-backed scratch allocation reduces garbage-collection pressure during geometry work. Elevation is stored as signed 16-bit cell units, using one page-constant Mercator Z scale so the API, terrain, and objects do not drift by applying different pointwise scale rules.

## Unified World-Layer Pipeline

Terrain and objects deliberately keep different source storage:

- **Terrain** is dense and varied — a Tile-shaped layer — so L3 pages retain dense 2D channels and elevation data.
- **Objects** are sparse and fully 3D — a Volume-shaped layer — so each occupied `64³` band of an L2 chunk owns a sparse tree of palette indices. A catalog value of `0` is `OBJECT_UNDEF`, meaning absence/fallback. `0x80` is explicit AIR (retains the leaf, erasing whatever a lower layer would otherwise show), `0xff` is UNKNOWN, and every other real value is a placed cube color, terrain-styled pick, or decor kind (`cell/cube.ts`, `cell/decor.ts`, `cell/mod.ts`) — which is also its id in the global palette ([Records](#records)).

They unify only at the renderer boundary through `WorldLayer`:

```text
terrain page ──→ TerrainLayer ──┐
                               ├─→ temporary signed-Z voxel bands
object store   → ObjectLayer ──┘       ├─→ shared mesher → baked per-chunk geometry
                                       └─→ shared raycaster → world hit
```

Each layer provides:

- `revision(key)`: a cheap layer-owned cache identity;
- `paint(key, target)`: full voxel composition.

Registration order is override priority, so later layers replace earlier material at the same voxel. Cache invalidation does not know terrain- or object-specific stores; it compares the revision tuples owned by the registered layers.

### Temporary dense bands

One chunk composes into its L2/Chunk-sized Volume as sorted, signed 64-cell Z bands. Within a band, arrays are flat and direct:

- `Uint32Array material`: material class + RGB (`0xCC_RRGGBB`);
- `Uint8Array layer_idx`: texture-array layer;
- optional sub-cell payload only for bands containing fine objects.

The common payload is five bytes per voxel. One `64³` band is therefore about `1.25 MiB`; retaining one for hundreds of visible chunks would exceed a mobile memory budget. Bands are pooled scratch storage and released immediately after meshing — `ChunkVolume` never survives past one synchronous composite pass.

What _does_ survive that pass, per in-frustum chunk, is the chunk's **column run set** — every one of its `64²` columns' already-merged material runs (`voxel_scan.ts`), packed as parallel lanes over one pooled block (17 bytes a run) rather than a view into the released bands. It is scanned once and reused across every LOD tier and every LOD transition after that, until the chunk leaves the retained set or its source revision changes — the expensive part (walking bands) happens once per chunk's lifetime instead of once per rebuild, and the same run set feeds LOD1 and every coarse tier. Coarse tiers of pure terrain skip the volume altogether and are built from the page heightmap directly. A `BuiltChunkBucket` (`render/layer/pipeline/chunk_bucket.ts`) is kept per LOD tier a chunk has actually built, retained under a memory budget rather than evicted on every tier change, since rebuilding one from the run set isn't free either. Its buffers are one pooled block per mesh, refcounted where the layout and the mounted geometry both hold it, so a mesh's several typed-array spans are one allocation whose lifetime ends together.

Raycasting needs exact composed voxels but only near the pointer ray, so it uses a separate LRU capped at four chunks and eight total bands (about 10 MiB of common payload). An oversized vertical chunk is consumed once instead of entering the cache.

## Rendering and LOD

The calculation side — layer composition, column scanning, meshing, bounds, and voxel ray tests — returns plain typed-array data and is renderer-independent. Three.js / React Three Fiber begins at the submission boundary, where it owns meshes, materials, texture arrays, camera/frustum math, scene lifecycle, and GPU resources. In the repo: `apps/web/src/render/` (including `render/layer/pipeline/`) stays Three.js-free; the R3F-owning consumers (`ChunkStage`, `Cube`, the anchor/frame components, ...) live under `apps/web/src/module/world/` instead. Chunk ownership itself is imperative: `chunk_node.ts` drives meshes and buckets directly rather than through a component tree, since one component per chunk per tier made React's reconciler the thing deciding when GPU buffers live and die.

LOD samples the 64² chunk at strides `1/2/4/8/16/32/64`. A tier hands off to the next when one of its cells would project to about six pixels — screen-space error, derived from the viewport height and field of view (`render/lod.ts`), which at the reference viewport puts the handoffs at `192/384/768/1536/3072/6144` world units, proportional to the stride so projected coarse-cell size changes smoothly. Everything that keeps step with the tiers — the POI badge lattice, the badge fade, decor's range — is a multiple of that first handoff. Decor is drawn by the same criterion applied to a kind's extent: a thing that would cover fewer pixels than two of the tier's cells is not drawn, at full density up to that tier and not past it (`module/world/decor/decor_layer.ts`).

The visible handoffs use complementary stochastic visibility between adjacent LOD meshes rather than alpha-overlapping them. LOD work is frame-budgeted, while a newly visible chunk starts from its distance-derived tier instead of always flashing through the coarsest proxy.

Frustum culling uses the exact union of terrain/object and primary/secondary mesh bounds. Measured bounds survive GPU/surface eviction and are invalidated with their source revisions. Until a chunk has measured geometry, a conservative camera-projection guard prevents the low-camera-polar false negative that previously removed the ground directly beneath the view.

### Coarse LOD

A coarse cell (`lod_step > 1`, `chunk_mesher.ts`'s `bake_chunk_mesh_coarse`)
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

### Scheduling

Every per-frame subscriber in the world is a named step of **one tick**
(`module/world/sched/tick.ts`), the single `useFrame` in the canvas. The
tick runs its phases in priority order — `interaction` (camera, frame,
puck, companion: unbudgeted and O(1) by contract), `visible` (the chunks on
screen: LOD recheck, scan, bake, commit), `content` (what fills the world
but is not this frame's geometry: the POI index sweep and placement),
`cosmetic` (fades, environment, the badge field), `report` (the ledger) —
on one budget sized from the display's measured frame interval, with a
fifth of it held back from `visible` so `content` is never left the
forward-progress floor alone. A step runs its work as units under
`budget.run`, which guarantees that the first unit that does work always
runs and stops after the deadline; a budgeted phase is only as cooperative
as its units are small, which the ledger's step table exists to check.

The scheduler's view of every chunk is one slot in `sched/chunk_table.ts`:
flags, distances, bounding sphere, pending-since, retained bytes. What to
build next is one unified priority (`sched/priority.ts`) — slot holders,
then in-frustum by distance from the lead point, with ageing so a chunk the
camera keeps almost reaching cannot wait forever — and what to keep is a
memory budget over the retained meshes.

The renderer follows a single-threaded cooperative-yield model, closer to
Figma's than to a preemptive one: everything that can be cut into units
under the budget stays on the main thread, and that is nearly everything.
The chunk scan and bake are stepped too — a row of columns, a few rows of
masks or a few hundred boxes a call — and run through the Rust scan and
mesher (`crates/pxl-core`, compiled by
`crates/pxl-wasm` into one module with numeric exports and no bindgen, which
imports the core memory: a run set is baked in the block it lives in and the
mesh written into the block acquired for it, nothing copied). It is held byte
for byte to the TypeScript mesher, which stays behind the dev setting
`bake_path` as the reference; on a phone the Rust bake cost a third to a
half as much per run and its longest step was 1–4 ms against 12–17
([issue/experiment/20260913-mesher-in-rust/](./issue/experiment/20260913-mesher-in-rust/)).
A worker for the bake was built behind the same setting and measured not to
help
([issue/experiment/20260913-bake-worker-on-the-phone.md](./issue/experiment/20260913-bake-worker-on-the-phone.md));
it stays as a measurement knob over the frame, port and link in
`@xpute/core/ipc/`, with the little memory above as its transport. `module/world/sched/` also holds the fetch scheduler (a priority
queue over a concurrency ceiling, aborting what falls out of a category's
wishlist) and the **ledger**: a read-only ring of frames and tasks with a
step table, per-job waits, a draw census and a GPU timer where one exists,
printed as a report and exported as a Chrome trace. The ledger observes; it
never feeds back into the schedulers' own decisions
([issue/20260909-cooperative-scheduler-and-task-ledger.md](./issue/20260909-cooperative-scheduler-and-task-ledger.md)).

### The channel: commands and changes

What crosses a boundary — between the UI and the world, the main thread and
a worker, later the glue and a WASM core — has two halves, both data
([issue/20260913-boundaries-commands-and-what-comes-back-from-april.md](./issue/20260913-boundaries-commands-and-what-comes-back-from-april.md),
[issue/20260913-the-read-direction-a-change-log.md](./issue/20260913-the-read-direction-a-change-log.md)).
The write half is a **command**: a numeric `cmd`, args, a body, an `errno`
reply, laid out as a frame in the buffer that carries it (`@xpute/core/ipc/frame.ts`),
dispatched by a port that is codec and dispatch only, carried by a link
that transfers. The read half is a **change log** (`@xpute/core/ipc/change.ts`):
a source bumps `(topic, key)` and the topic's tick becomes the key's
revision, written to a revision table and one fixed ring; a reader compares
a revision or walks the ring from its cursor, and a cursor the ring has
overwritten re-reads its topics. A change carries no value — the value is
read where it lives — so the ring is small and a span of it crosses a
boundary unchanged. The world's log (`module/world/change.ts`) numbers six
topics — object column, decor column, object cell, terrain page, POI page,
decor asset — and every way of reading that existed before (store
revisions, per-key listeners, page interests, `useSyncExternalStore`, the
per-tick pull) is an adapter over it; listeners run when the tick pumps,
never inside the write. Inside a page nothing on these boundaries is lost,
so there is no acknowledgement, deadline or retry; reliability is decided at
the network boundary alone, when a server exists. A command travels on a
ring in the receiver's memory with a doorbell that names only a count — the
submission ring is not built yet: the Rust mesher is still called export by
export.

## Camera

The orbit camera is driven by three controls — `dist`, `polar`, `azimuth` —
and nothing else writes them. Terrain is allowed one question, in
`module/camera/terrain_gate.ts`: would this view state put the eye inside
it. The gate is a pure function of the three controls rather than of the
live scene graph, and it sits in `set_view_state`, the one funnel every
input path goes through, so a refused move never leaves the controls and the
drawn camera disagreeing.

`Settings > Camera` picks what the answer does: `through` ignores terrain,
`stop` takes a control as far as it asked or as far as it can, and `climb`
grants every control and raises the _drawn_ pitch instead, leaving
`view_state` untouched so the pitch relaxes back on its own.

Every setting is one entry of one schema (`app/setting/schema.ts`): its
default, its kind and its section. The store reads the defaults with the
user's overrides on top and stores only the overrides, and the settings
sheet renders its rows from the schema. Dev-only settings — the debug
gizmos, the ledger panel, a render-path bisect — are entries marked `dev`.

## Billboards and POI

A POI page is an XTP record the API emits — positions, category, rank and
id hash as lanes, the strings as leaves read one at a time — copied into
the core memory on arrival. A spatial index over the resident disc
(`module/world/poi/poi_index.ts`) is rebuilt a range of records per unit
in the content phase while the previous build keeps answering; placement
(`poi_layer.tsx`) is a job of the same phase — query, file the hits into an
implicit lattice by tier, offer them with estimated widths, greedy
placement on a screen grid in distance order, build the badges — committed
whole. Badges are drawn from one atlas (`module/world/billboard/atlas.ts`,
shelf-packed, uploaded by rectangle) as **one instanced quad**
(`billboard/field.ts`): anchor, size and sheet rectangle are per-instance
attributes, and the fit to screen pixels, the distance fade and the anchor
pinning are the vertex shader's.

Occlusion is all or nothing per badge, on purpose: `render/scene_depth.ts`
copies world depth after the scene render, and
`billboard/anchor_occlusion.ts` reprojects several points up each badge
through that copy and discards the badge when most are behind terrain.
The same pass writes `gl_FragDepth` from an upright twin of the quad, so
billboards sort against the world along world-up rather than camera-up.
Chat bubbles still use the sprite form of the same patch.

## XTP — Xpute Tree Packet

XTP is the binary wire format used by the bake pipeline and terrain-page API:

- **Tree-shaped**: scalar leaves, typed-array sequences, and branch nodes;
- **C-layout-compatible**: 8-byte word alignment, little-endian, relative offsets;
- **Relocatable**: packets can be moved or spliced without rewriting absolute pointers;
- **Zero-copy reads**: typed-array sequences are views into the packet buffer;
- **GRAFT**: a pre-encoded subtree packet splices with the same wire layout as an inline node;
- **Type-preserving nulls**: optional nodes retain type metadata while physically absent.

TLV is used separately for flat, self-delimiting stream arguments. XTP lives in `@xpute/core`, not `@pxl/shared`, because the codec itself contains no pixelet domain knowledge.

## Records

On top of the tree packet, `@pxl/shared/record` defines the **column layer**: a record is `[kind, version, keys, required flags, count, columns...]`, so a reader finds a column by name, skips one it does not know and refuses one marked required — glTF's `extensionsRequired` and PNG's critical bit under one rule. Two records are shared by everything ([issue/20260913-everything-is-a-file-the-placement-and-the-palette.md](./issue/20260913-everything-is-a-file-the-placement-and-the-palette.md)):

- **`Placement`** — one row per placed thing, whatever it is: a `u64` id, a page-local `f32` position, the footprint's side in world units, an ENU heading (radians about up, counter-clockwise from +x), and a palette id; ancillary scale, rotation and tags. A tree, a placed cube and a bare ground cell are the same row; what differs is the palette entry.
- **`Palette`** — one global record naming every kind of thing: id, kind (`cube`, `ground`, `decor`, `actor`), name, model and skin by bucket key (`asset/<sha256>`, the content), extent measured at ingest, size, licence, author, colour. The palette this build ships is built from the cell constants plus a row per decor model and per character, over `record/ingested.ts` — the table `deno task ingest:asset` generates from the top-level `asset/` tree, which hashes every model, skin and texture into the bucket and measures each glb's bounds. Nothing imports a model or an image: the client fetches a key, the same key from the dev API's `./out/asset` and from R2. The texture pack is one more table over the palette (`record/texture.ts`, which image a cube type wears on which face) and one object in the bucket: the ingest composes its images into a single WebP sheet, and the client fills its layer array from that one download, a layer at a time. A cube's colour is the palette row's own (`spec/cell/color.ts`), what the minimap and the coarse tiers draw, and a pack decorates it.

Containers stay what they are — a terrain page is dense lanes, a POI page point records, the object store a sparse tree per band — and only the row is unified. The control plane above the records is deliberately narrow: identity, ownership, lifetime, revision, residency, and no universal read/write interface.
