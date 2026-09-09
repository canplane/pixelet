# Architecture

A spatial engine for turning real-world geographic data and player-built objects into one voxel-style 3D world.

The engine is split into a pixelet-specific domain layer (**`@pxl/shared`**: coordinate system, cell/terrain model, spatial hierarchy, bake wire format) and a project-agnostic utility layer (**`@xpute/core`**: collections, codecs, and the binary tree protocol those formats use) — see [Repository Structure](./README.md#repository-structure).

> Earlier iterations explored a decentralized P2P/WebRTC runtime (`xpute` as a peer-symmetric IPC substrate) and a Rust/WebGPU rewrite of the spatial/bake stack. Both are parked in favor of a plain client-server MVP. Their last states are preserved on `checkpoint/20260107-float`, `checkpoint/20260405-xpute`, and `checkpoint/20260409-wasm-aborted`.

---

## Coordinate System — PVF3

PVF3 (Packed Vector Fixed-point 3D) is a 96-bit fixed-point coordinate:

```text
PVF3 = { i: IVec (64-bit), f: FVec (32-bit) }
```

- **IVec** packs three signed integer lanes in a `24/24/16` X/Y/Z layout. X is cyclic so the canonical world wraps at the longitude seam.
- **FVec** packs three unsigned 10-bit fractional residues, giving `1/1024` resolution within one canonical unit.
- Arithmetic is deterministic across platforms through BigInt.
- The bit fields contain no guard lanes: IVec uses its full signed `24/24/16` ranges, while FVec uses all 10 bits per residue lane (the top two bits of its 32-bit word remain unused).
- The EPSG:3857 adapter assigns physical meaning: currently `1u = 1 L1 block ≈ 2.389` planar Mercator meters at the equator. Geographic distortion and page-constant vertical correction live in the adapter, not in PVF3 itself.

Placed-object render payloads quantize their sub-cell offset and size to `1/256` of an L1 block. That compact render format is distinct from PVF3's finer `1/1024` coordinate capacity.

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

Objects have no server persistence or transfer protocol yet — the current prototype is client-local only, storing one SVO per occupied L1 cell and keeping just the L2-column render index live. When a persistence layer is built, the design intends to reuse one full-3D L2 Volume as the storage/transfer envelope rather than introduce a second spatial level at the same `64³` extent.

One React renderer component owns one L2 XY column and composes however many signed 64-cell L2 Z bands its content touches. One L3 page therefore contains 256 possible L2 columns.

### Layer, space, and shape

Three independent axes describe any piece of world data, and none of them implies the others:

- **Layer** — which domain owns the data: `terrain` or `object`. See [Unified World-Layer Pipeline](#unified-world-layer-pipeline).
- **Space** — which scale tier addresses it: the L0–L5 hierarchy above. Purely a size/addressing concept; a level's own name (Page, Chunk, Anchor, ...) says nothing about what shape of data lives at that tier.
- **Shape** — how the data itself is structured: a **Tile** is a 2D/2.5D raster (terrain's own storage form, currently sized at L3/Page granularity); a **Volume** is a full 3D structure (the object octree's storage form, and the renderer's own temporary composited field at L2/Chunk granularity). A **Cell** is just a position/address — no size or dimensionality of its own, no payload; any space level's `addr`/`idx` is a Cell. A **Pixel** is a Cell plus its 2D payload — the atomic sample inside a Tile. A **Voxel** is a Cell plus its 3D material payload — the atomic sample inside a Volume. A **Band** is one materialized `64³` Volume: the renderer's concrete in-memory instance holding a single occupied Z-band of an L2 column (see [Temporary dense bands](#temporary-dense-bands)) — renderer-only working storage, not a hierarchy level or a persisted format.

Terrain today is Tile-shaped data addressed at Page granularity; objects are Volume-shaped (sparse octree) data anchored at Block granularity, with the renderer composing an L2/Chunk-sized Volume as their shared transport/render envelope (see [Temporary dense bands](#temporary-dense-bands)).

| Layer                               | Space level              | Shape  | Concrete type                                    | Atomic unit |
| ----------------------------------- | ------------------------ | ------ | ------------------------------------------------ | ----------- |
| terrain (source)                    | L3 Page                  | Tile   | `Tile` / `TileStore`                             | Pixel       |
| object (source)                     | L1 Block (sub-block SVO) | Volume | sparse SVO octree                                | Voxel       |
| terrain+object (renderer, composed) | L2 Chunk                 | Volume | `ChunkVolume`, sliced into `VoxelBand`s ("Band") | Voxel       |
| — (any)                             | L4 Anchor                | none   | `AnchorRegistry`                                 | —           |

## Terrain Data and Streaming

L3 pages are the primary terrain streaming unit, storing terrain's Tile-shaped data. The client requests pages around the current center and keeps terrain in its source-appropriate 2.5D form: dense categorical ground rasters plus a padded elevation raster reconstructed with bicubic sampling.

Each page carries four ground channels rasterized from Overture/OSM geometry (Buildings/Transportation/Base from Overture Maps, coastline from Overture's Base `water` theme via the same OSM-coastline-tool lineage its own coarse land/ocean pre-filter still reads OSM for):

| Channel     | Contents                                            |
| ----------- | --------------------------------------------------- |
| `LAND`      | Ground surface type (bare, green, sand, rock, etc.) |
| `WATER`     | Inland water body type (river, lake, wetland, etc.) |
| `STRUCTURE` | Buildings and constructed surfaces                  |
| `TRANSPORT` | Roads, paths, railways                              |

Sea cells use `SEA_FLAG = 0x80` to distinguish sea from ordinary land while retaining the byte-sized channel representation.

## Bake Pipeline

```text
Overture/OSM PostGIS rows
  → XTP decode
  → overture_adapter / osm_adapter (row → feature)
  → normalizer (classify, rank, assign tags — policy-parameterized: overture_policy / osm_policy)
  → ground_baker (rasterize geometry into ground channels)
  → encode_terrain (elevation + ground → XTP)
  → compressed HTTP page response
```

Polygon rasterization uses even-odd fill with hole support. Linear features use a thickness kernel. Arena-backed scratch allocation reduces garbage-collection pressure during geometry work. Elevation is stored as signed 16-bit cell units, using one page-constant Mercator Z scale so the API, terrain, and objects do not drift by applying different pointwise scale rules.

## Unified World-Layer Pipeline

Terrain and objects deliberately keep different source storage:

- **Terrain** is dense and varied — a Tile-shaped layer — so L3 pages retain dense 2D channels and elevation data.
- **Objects** are sparse and fully 3D — a Volume-shaped layer — so each occupied L1 block owns a small sparse octree. Leaf byte `0` is `OBJECT_UNDEF`, meaning absence/fallback. `0x80` is explicit AIR (retains the leaf, erasing whatever a lower layer would otherwise show), `0xff` is UNKNOWN, and every other real value is a placed cube color, terrain-styled pick, or decor kind (`cell/cube.ts`, `cell/decor.ts`, `cell/mod.ts`).

They unify only at the renderer boundary through `WorldLayer`:

```text
terrain page ──→ TerrainLayer ──┐
                               ├─→ temporary signed-Z voxel bands
object octrees → ObjectLayer ──┘       ├─→ shared mesher → GPU instance buffers
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

The common payload is five bytes per voxel. One `64³` band is therefore about `1.25 MiB`; retaining one for hundreds of visible chunks would exceed a mobile memory budget. Bands are pooled scratch storage (a small 2-entry pool) and released immediately after meshing — `ChunkVolume` never survives past one synchronous composite pass.

What _does_ survive that pass, per in-frustum chunk, is `column_runs: MeshRun[][]` — every one of the chunk's `64²` columns' already-merged material runs (`voxel_scan.ts`'s `scan_all_column_runs`), plain copied values rather than a view into the released bands. It's scanned once and reused across every LOD tier and every LOD transition after that, until the chunk leaves frustum or its source revision changes — the expensive part (walking bands) happens once per chunk's visible lifetime instead of once per rebuild. This replaced an earlier ~36–40 KiB top-only (or top-and-bottom) heightmap shortcut; `column_runs` is real per-run data instead of one flattened sample per column, so it costs meaningfully more per chunk (roughly several hundred KiB to ~1 MiB, depending on how many runs a chunk's columns actually have) in exchange for not needing a separate coarse-LOD-only shortcut format at all — the same `column_runs` feeds LOD1 and every coarse tier. `chunk_stage.tsx` also keeps a small `BuiltChunkBucket` (GPU-ready instance buffers) per LOD tier a chunk has actually built, retained rather than evicted on every tier change, since rebuilding one from `column_runs` isn't free either.

Raycasting needs exact composed voxels but only near the pointer ray, so it uses a separate LRU capped at four chunks and eight total bands (about 10 MiB of common payload). An oversized vertical chunk is consumed once instead of entering the cache.

## Rendering and LOD

The calculation side — layer composition, column scanning, meshing, bounds, and voxel ray tests — returns plain typed-array data and is renderer-independent. Three.js / React Three Fiber begins at the submission boundary, where it owns meshes, materials, texture arrays, camera/frustum math, scene lifecycle, and GPU resources. In the repo: `apps/web/src/render/` (including `render/layer/pipeline/`) stays Three.js-free; the R3F-owning consumers (`ChunkStage`, `Cube`, the anchor/frame components, ...) live under `apps/web/src/modules/world/` instead.

LOD samples the 64² chunk at strides `1/2/4/8/16/32/64`. Handoffs occur at `192/384/768/1536/3072/6144` world units, proportional to the stride so projected coarse-cell size changes smoothly. The current fog distance is 900 units, so visible chunks normally reach at most stride 8; strides 16/32/64 remain valid for future longer ranges and fallback states.

The visible handoffs use complementary stochastic visibility between adjacent LOD meshes rather than alpha-overlapping them. LOD work is frame-budgeted, while a newly visible chunk starts from its distance-derived tier instead of always flashing through the coarsest proxy.

Frustum culling uses the exact union of terrain/object and primary/secondary mesh bounds. Measured bounds survive GPU/surface eviction and are invalidated with their source revisions. Until a chunk has measured geometry, a conservative camera-projection guard prevents the low-camera-polar false negative that previously removed the ground directly beneath the view.

### Coarse-LOD box averaging

Coarse LOD (`lod_step > 1`, `chunk_mesher.ts`'s `build_chunk_instances_coarse`) box-averages every real fine column under each coarse cell — the mipmap / voxel-cone-tracing style of screen-footprint filtering, not a nearest-neighbor pick of one representative column. For each `lod_step`-wide XY cell, every one of its `lod_step²` real columns' runs (from `column_runs`, see [Temporary dense bands](#temporary-dense-bands)) gets binned into `lod_step`-tall Z buckets and accumulated: color sums are weighted by how many fine cells of that bucket the run actually covers, and terrain and object runs are summed into the _same_ bucket unconditionally — material class plays no role in the average. Blending across a material boundary this way is intentional: at a coarse enough distance a mixed-color cell reads as a blended color the same way a photographed or mip-mapped surface would.

Coverage (how many of a bucket's `lod_step³` fine cells actually got hit) doubles as the cell's fill fraction — air simply never contributes to a bucket's sum, so a partly-empty bucket's average dilutes toward "less full" on its own, with no separate occupancy threshold. A bucket with any coverage at all still emits exactly one box, isotropically shrunk on X, Y, _and_ Z together (`span * Math.cbrt(fill)`, never Z alone) instead of being dropped — an earlier design that grid-snapped Z into fixed slabs and required a cell to clear a 50%-occupancy threshold before drawing anything could make a real, fully-placed object flicker out of existence entirely whenever its footprint happened to straddle two coarse cells (each half individually under the threshold); continuous fill-based shrinking can't produce that failure mode, since there is no threshold to fall under. Sub-cell (fractional, precisely-placed) runs contribute their true volumetric fraction (linear size fraction cubed) rather than a full cell's weight. The resulting box carries no texture-array tap (`layer_idx` is always written as `NO_LAYER`) — once a box's color can be a blend of several materials, sampling any one material's texture for it no longer means anything, so the shader's existing flat-RGB fallback path handles it instead.

Both halves of building a coarse LOD tier — scanning a chunk's `column_runs` in the first place, and this per-tier averaging pass — are cooperatively scheduled rather than always paid for immediately: each estimates its own likely cost from a cheap signal (occupied Z-band count for a scan; total run count for an average build) against a running session average, and defers to a later frame — at most once per chunk, to guarantee eventual progress — when the estimate clears a budget multiple. This is deliberately not a Web Worker: the renderer follows a single-threaded, cooperative-yield scheduling model (closer to Figma's than to a preemptive-multithreaded one) so that expensive work never blocks a frame for longer than a bounded slice, without the cross-origin-isolation/`SharedArrayBuffer` deployment cost real threading would add.

At LOD1 (`lod_step === 1`, exact per-run boxes, no averaging), a run that is fully boxed in by solid material — not the topmost run in its own column, and all 4 horizontal neighbor columns solid across its exact `z0..z1` span — never gets an instance at all, since none of its faces could ever be visible. Chunk-edge columns are exempt (a neighboring chunk's data isn't available to check, and assuming it's solid would risk a visible hole for a cosmetic win).

## XTP — Xpute Tree Packet

XTP is the binary wire format used by the bake pipeline and terrain-page API:

- **Tree-shaped**: scalar leaves, typed-array sequences, and branch nodes;
- **C-layout-compatible**: 8-byte word alignment, little-endian, relative offsets;
- **Relocatable**: packets can be moved or spliced without rewriting absolute pointers;
- **Zero-copy reads**: typed-array sequences are views into the packet buffer;
- **GRAFT**: a pre-encoded subtree packet splices with the same wire layout as an inline node;
- **Type-preserving nulls**: optional nodes retain type metadata while physically absent.

TLV is used separately for flat, self-delimiting stream arguments. XTP lives in `@xpute/core`, not `@pxl/shared`, because the codec itself contains no pixelet domain knowledge.
