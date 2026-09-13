# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **The move to a Rust core in WASM, drawing through WebGPU.** Scheduled
  2026-09-13 ([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md)).
  Taken from the smallest separable piece outward: a piece is first cut
  out behind a boundary that carries typed arrays, still in TypeScript, so
  the boundary is proven before the language changes behind it. Done on
  the way, 2026-09-13: the bake takes a context; the bake as a command over
  a frame, port and link; the read direction as a change log; the core
  memory's sections. In order:
  1. **The mesher in Rust.** The first Rust. Ported 2026-09-13 as a
     line-for-line translation — crates/pxl-core's mesher behind
     crates/pxl-wasm's numeric exports, no wasm-bindgen, over the core
     memory the module imports (a run set baked where it lies, the mesh
     written into its block) — stepped on the main thread as
     `bake_path: wasm`, the default since the phone reading
     ([experiment/20260913-mesher-in-rust/](./experiment/20260913-mesher-in-rust/)),
     and held byte for byte to the TypeScript bake at every tier by
     wasm_bake.test.ts. The whole-chunk scan followed the same day
     (crates/pxl-core/src/scan.rs, wasm_scan.test.ts byte for byte against
     voxel_scan.ts; 1.9x on V8), reading the composite's bands where the
     compositor left them. The terrain layer followed
     (crates/pxl-core/src/terrain.rs: the bicubic sample, the water recess,
     the channels' override, held cell for cell to the TypeScript adapter by
     terrain.test.ts; paint 1.8x and the direct path 2.8x on V8), reading a
     page's lanes where the client keeps them and painting the module's own
     volume. Then the object store and, with it, the object layer's paint
     (crates/pxl-core/src/{sparse_tree,object}.rs, held entry for entry and
     voxel for voxel to the TypeScript by store_reference.test.ts), the
     direct path's writer (the scan's scratch, the terrain's runs written
     inside the module) and the raycast's column walk (the scan's own,
     read as lanes). Then the chunks' builds themselves (2026-09-14,
     crates/pxl-core/src/build.rs, held byte for byte to the TypeScript
     pipeline by core_build.test.ts): the node says which tiers it lacks,
     and the module makes the runs — the direct way, or by composing terrain
     and objects into a volume and scanning it — bakes the tiers under the
     frame's budget in one `pxl_tick`, and hands each mesh over in a block of
     the mesh section. Retired with it, to
     `archive/20260914-ts-build-paths`: the TypeScript fallback for a trapped
     module, the `bake_path` setting's stepped and worker paths (the worker
     had been measured to buy nothing,
     [experiment/20260913-bake-worker-on-the-phone.md](./experiment/20260913-bake-worker-on-the-phone.md)),
     the bake port and its little slabs, and staging a mesh past the core
     memory: a block the pool refuses leaves the work where it is. The
     TypeScript scan, mesher and volume readers stay only as the references
     the tests hold the module to. Still TypeScript: the recheck (the
     camera, the frustum and the LOD criterion), the chunk table's order,
     decor, and the raycast's composite.
     XTP packets land aligned (8 bytes, 16 for SIMD) and are read through
     checked casts. The glue and the core talk through rings in the core's
     linear memory with a drain export as the doorbell — the submission
     ring is built here, where the boundary first exists
     ([boundaries, "Rings"](./20260913-boundaries-commands-and-what-comes-back-from-april.md)).
     Built 2026-09-14 (crates/pxl-core/src/ring.rs, @xpute/core/ipc/ring.ts):
     a submission ring and a completion ring, a pair in the module's heap,
     64-byte entries; `pxl_tick` is the doorbell and the glue reads the
     completions when it returns. The chunks' builds are the first commands
     on them.
     **Carried into the port, not fixed in TypeScript first** — an audit
     of the mesher, scan and volume on 2026-09-13 found these, and each is
     what Rust's types or an algorithm rewrite resolve on the way, so
     changing them here would be paying twice:
     - callbacks and objects in the kernel: `RunSink`, `fill_column_runs`'s
       writer closure, `locate_voxel`'s returned object, `sub_cell?.` — enums
       and structs in Rust (the mesher's own — the bucket arena's objects and
       the `fine!`/`coarse!` pair — went with the translation);
     - the scan's module-level scratch (`SC`, `COLUMN_STARTS`, `open_scan`)
       is a context in the Rust scan; the TypeScript's stays for the direct
       path and the reference until they follow;
     - algorithm: `SubVoxels` as five parallel Vectors and its compaction
       on every write — the volume's, rewritten with the compositor (the
       Rust scan reads the five lanes as they are). The mesher's two
       were taken the same day: the coarse pack sorts its buckets; the O(k²)
       neighbour walks were rewritten as forward cursors, measured to buy
       nothing at the stack heights the scan produces, and reverted — where
       a fine bake's time goes is the face masks and writing the mesh
       ([experiment, "The rewrite, measured"](./experiment/20260913-mesher-in-rust/#the-rewrite-measured));
     - `Infinity`-style sentinels and exceptions as API guards in the
       volume — `Option` and `Result` (the mesher's and scan's `-1`s are
       `Option`; their asserts stay, as a malformed input is a bug).
  2. **The core to WASM on the main thread, and a WebGPU renderer.**
     three.js and R3F leave together (R3F can go earlier on its own). What
     three.js does for us today is listed in the destination issue as a
     checklist the renderer fills, not a gate on starting. WebGPU removes
     the WebGL-side costs; it does not shrink the working set — that is the
     byte budget in P1. Settled on the way, 2026-09-13: the module imports
     the core memory, and the map's first 32 MB is the module section —
     its stack, data and heap, where a wasm32 linker puts them, behind a
     bump allocator (crates/pxl-wasm/src/arena.rs) since the memory never
     grows; the module states its size and the client checks the map
     against it at instantiation. The same day, the buddy over every
     section's range moved into the module (crates/pxl-core/src/buddy.rs,
     held to the TypeScript pool's addresses request for request by
     core_pool.test.ts): the module is instantiated before the map is cut,
     and the TypeScript pool names blocks through it. Then the volume
     (crates/pxl-core/src/volume.rs): bands are the module's, taken from the
     fixed section for the sixteen standing stores and the mesh section past
     them, every paint a call into it, and readers get views; the Rust scan
     opens a volume by id. Then the terrain layer's reading of a page, and
     the object store with the object layer's paint, the direct path's
     writer and the raycast walk, and the chunks' builds under one tick
     through the rings (2026-09-14). Next: whatever else stands before the
     renderer; then the renderer, starting with the hover cursor. The cursor goes first because it is
     where the two sides already disagree: since the object store moved
     into the module, the eraser's half-transparent ghost over a placed
     cube shows late or not at all while the hide itself lands (decor's
     ghost is unaffected; the build before the move, 9ff57fe, is fine) —
     suspected to be ordering across the boundary, not chased in three.js.
     With the same move its side textures lie on their side on some walls,
     which 9ff57fe does not do either. Both go with drawing the ghost
     through the chunk pipeline — one box through the mesher and the chunk
     material with an opacity — so it cannot diverge from what it previews.

## P1

- **One byte budget over everything drawn, paid for in LOD.** The phone's
  page dies because the working set exceeds a budget of about two
  gigabytes, and a WebView (Capacitor) runs in the same WebKit content
  process under the same limit — reported by others as a 2048 MB jetsam
  ceiling on iPhone, consistent with the mini's 1.6–2.1 GB. Today's
  `RETAIN_BUDGET_BYTES`
  ([20260912-what-to-render-when.md](./20260912-what-to-render-when.md))
  covers only what is held off screen; what is on screen has no ceiling.
  The precedent is CesiumJS: a tileset has `cacheBytes` and
  `maximumCacheOverflowBytes`, and when the tiles a view needs exceed both
  it raises the screen-space error until they fit — the view gets coarser,
  the page does not die
  ([Cesium3DTileset](https://cesium.com/learn/cesiumjs/ref-doc/Cesium3DTileset.html)).
  MapLibre sizes its tile cache from the viewport rather than a count
  ([MapOptions](https://maplibre.org/maplibre-gl-js/docs/API/type-aliases/MapOptions/)).
  The same lever exists here: the one LOD criterion's distance thresholds.
  Needed on the web whatever the graphics API; the renderer in P0-2 is
  built against it.

- **"Everything is a file": the Placement and the palette.** Decided
  2026-09-13, first:
  [20260913-everything-is-a-file-the-placement-and-the-palette.md](./20260913-everything-is-a-file-the-placement-and-the-palette.md).
  One `Placement` record for every placed thing (id u64, page-local
  f32 position, span, ENU heading, palette id; ancillary scale,
  rotation, tags) and one global `Palette` record whose rows point at
  models by content hash in the bucket. Done 2026-09-13: the column layer
  (`record/table.ts`), both records with tests, the shipped palette as a
  record built from the cell constants with every model's measured
  extent, the decor bundles bound to it, and the object store's persisted
  form as one `Placement` record per page in a page-keyed container (v7;
  positions round-trip exactly, since the store's steps are binary
  fractions and a page-local `f32` holds them). Left, in order:
  - **The scatter emits `Placement`** when something reads it — the
    worker is the first consumer; until then the instance lanes are the
    same columns and a packet would be a copy nobody opens.
  - **The ingest route** (`POST /api/models`): validate, hash, measure,
    store, insert a palette row from `USER_ID_BASE`. The bucket path it
    stands on is done (2026-09-13, evening): every shipped model, skin
    and texture is `asset/<sha256>` in the bucket, ingested from the
    top-level `asset/` tree by `deno task ingest:asset`, and the shipped
    palette's `model_key`/`skin_key`/`model_hash` are filled from the
    generated table.
- **The phone, at the end of 2026-09-13.** Same spot, same procedure,
  after POI went under the tick, the badges became one instanced quad,
  the companion stopped probing, placement was cut into units and decor
  took the projected-size threshold: dt p50 17 ms on a 16 ms display, the
  frame's own task 2 ms, 13% of moving frames missed (from 39%). The
  render-path bisect put MSAA and the composer at about a millisecond of
  p50 and five points of the missed share — not worth removing. What is
  left, in order:
  - **A chunk bake is 14–21 ms on the phone** (3 ms on the desktop) while
    a zoom-out is filling in: the worker, above.
  - **The load storm** is unchanged: `rerender 66`, `mount +384` frames at
    six seconds, and page fetches queued ten seconds behind six in flight
    (P3).

- **XTP: the column-table layer, and a string-sequence leaf.** Two pieces
  that go together, and the first work of "everything is a file":
  - A `str` sequence leaf — one UTF-8 blob plus a u32 offsets lane — in
    place of one `str` node per string. A new type code, so existing
    packets still read. Per string today is an 8-byte header, a NUL and
    word padding (the POI repack: 60 MB gzipped against 51 MB of JSON)
    and one node object at encode.
  - The named-column convention on top of the tree, as designed in
    [20260910-why-xtp-and-what-would-end-it.md](./20260910-why-xtp-and-what-would-end-it.md):
    `[keys, count, columns...]` for homogeneous records, a shape table for
    the few heterogeneous ones. The POI page is positional today, its
    field order held by format.ts's encode/decode pair; the key list makes
    it self-describing and lets a reader skip a column it does not know.
    Not before a second record kind exists — one record does not need a
    convention, and building it twice is the cost to avoid.
- **XTP encoder without per-leaf objects.** A node object per leaf and a
  closure per scalar at encode; mechanical, format-unchanged, bake-side
  only. With the above, not on its own.

- **Placed objects come back moved after a reload.** Found 2026-09-14: a
  wall of fifteen cubes (five along X, three up Z) reloads as thirteen
  cells on one Z layer spread along Y. `decode_objects` turns a
  `Placement` row back through `local_world_to_canonical` (the
  projection), which is not the inverse of the page-local offset
  `encode_objects` writes. In since v7; not the module's.

## P2

- **Compression, chosen by traffic.** Recorded 2026-09-13, not scheduled:
  [20260913-compression-by-traffic.md](./20260913-compression-by-traffic.md).
  Brotli q11 takes a third off the baked terrain pages and a fifth off POI
  against gzip, and every browser on the floor takes `br`; zstd is below
  the floor in Safari. The baked pages go first when this is taken up (a new
  terrain version, the bake, the uploads and the monthly workflow). Live
  responses want a fast level, realtime messages want an encoding and a
  stream more than a codec, and the codec stays in the transport, never in
  the command frame.

- **Decor: the impostor tier.** Decided 2026-09-13
  ([20260911-decor-identity-orientation-and-one-cell-format.md](./20260911-decor-identity-orientation-and-one-cell-format.md),
  "Closed"): each variant rendered once to a sprite, far instances drawn
  as a camera-facing quad through the billboard field. The criterion is
  in and every shipped kind is on the nearest tier only; the impostor is
  what would draw them further for two triangles each. Not a phone fix —
  build it when a wider decor range is wanted.
- **Terrain page blocks with the page.** `resident.ts` keeps the pool
  blocks a page owns in a `WeakMap<Terrain, Block[]>`; they belong in the
  page table's handle beside the views.

## P3

- **Decor culling by page.** Decor is drawn for the whole loaded world
  in one instanced mesh per variant; with the threshold it exists only in
  the nearest ring, so culling saves the half behind the camera. If the
  phone re-measurement still puts decor first, split into one mesh per
  page and variant so three's bounds culling applies.
- **First load on a phone: the fetch queue, not the network.** Terrain
  and POI pages p50 9.7 s / 13 s from request to adopt, of which the
  network was 1.4–1.9 s; the rest was queue wait behind `MAX_INFLIGHT`
  6 with 64 pages asked at once, 4.2 MB per terrain page. The ordering
  is right (nearest first); what is worth asking is whether the quota
  and the page size are, on a phone's connection.
- **Ledger ring as lanes.** The dev ledger's frame records are objects
  rebuilt every frame for the overlay. Measurement, not state; only worth
  it if the overlay itself shows up in a profile.
- **The run set's per-axis inset has no writer.** `sub_sx`/`sub_sy` are
  always a copy of `sub_sz` (the scan, the coarse pack and the object
  adapter all set the three equal), so the per-axis design column_run.ts
  documents — a pillar narrow in XY and full in Z — costs two bytes a run
  for nothing today. Either the coarse bake learns to fit XY and Z
  separately, or the lanes go; decided with the Rust mesher, since the
  format is its input.
- **POI fetch on an empty body.** `fetch_poi` throws `Unexpected end of
  JSON input` when the proxy returns an empty 200 (seen 2026-09-12, several
  pages at once); the page is retried by the next sync. Goes away with the
  XTP page (a short packet fails the header check instead) — verify then.
- **The build number moves to CI when there is one.** Official builds are
  counted like a build lab's (2026-09-13): one counter outside git, in the
  bucket at `meta/build-number`, taken by `deno task deploy` because that is
  the only central build today (apps/web/script/deploy.ts). A build lab
  numbers every build of main and ships some of them; with a CI building
  main, the increment moves into it, deploy picks one of its numbered
  builds, and the count continues from the counter as it stands — never
  reset.

## Parked, with the reason on file

- **Where the fill-in frame's commit goes.** While terrain fills in on the
  phone the main-thread task p95 is ~27 ms whichever thread bakes, so the
  cost is scan, attach or mount rather than the bake; splitting `commit` into
  those would name it. Not now: attach and mount are three.js geometry and
  React's reconciler, which the WebGPU renderer (P0-2) replaces, and a
  breakdown of code that is leaving decides nothing before it leaves. Scan
  is core and stays; measure it when it is the piece being ported.
- **The bake worker path.** Behind the dev setting `bake_path`, default
  `wasm`. Measured on the phone and not taken
  ([experiment/20260913-bake-worker-on-the-phone.md](./experiment/20260913-bake-worker-on-the-phone.md));
  kept, not deleted, because it is small and tested and the frame, port and
  link under it are the boundary later pieces use. Remove it when a piece
  does use that boundary, or when the renderer makes the question moot.

- **Phone costs that sit on the three.js boundary.** Recorded 2026-09-13
  and left to the WebGPU renderer, which creates these resources itself:
  - **A ~300 ms off-thread frame every twenty seconds or so**, on a still
    frame with nothing mounted or uploaded. Outside the task and without a
    GPU timer or LoAF on iOS there is no instrument for it; Safari's own
    Timelines would be the next look, if it is ever the complaint.
  - **A repacked sheet still uploads whole.** Growth replaces the texture
    and its first bind uploads the canvas — 16 MB at 2048, 64 at 4096 —
    once per size step (`poi_field` max 76 ms on the phone, 151 at 4096).
    An empty texture plus the live rectangles would be a third of that.

- **Layout bucket handle table.** Each node's two `Map<LodStep, …>` caches
  as a fixed (slot × tier) handle table. The vertex data is already pooled
  and three.js's `BufferAttribute` takes the typed views, so the handle is
  the glue boundary; this removes two Maps per node and nothing else.
  Deferred 2026-09-12.

- **Coarse tiers: draw calls, a wider unit.** Not a bottleneck after the
  direct path; not without a measurement —
  [20260912-coarse-tiers-what-the-field-does.md](./20260912-coarse-tiers-what-the-field-does.md).
- **Characters and the rest without a record format.** Not before the format.
