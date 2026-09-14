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
     (crates/pxl-core/src/bake/scan.rs, wasm_scan.test.ts byte for byte against
     voxel_scan.ts; 1.9x on V8), reading the composite's bands where the
     compositor left them. The terrain layer followed
     (crates/pxl-core/src/bake/terrain.rs: the bicubic sample, the water recess,
     the channels' override, held cell for cell to the TypeScript adapter by
     terrain.test.ts; paint 1.8x and the direct path 2.8x on V8), reading a
     page's lanes where the client keeps them and painting the module's own
     volume. Then the object store and, with it, the object layer's paint
     (crates/pxl-core/src/{sparse_tree,object}.rs, held entry for entry and
     voxel for voxel to the TypeScript by store_reference.test.ts), the
     direct path's writer (the scan's scratch, the terrain's runs written
     inside the module) and the raycast's column walk (the scan's own,
     read as lanes). Then the chunks' builds themselves (2026-09-14,
     crates/pxl-core/src/world/build.rs, held byte for byte to the TypeScript
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
     the tests hold the module to. Then the scheduler (2026-09-14,
     crates/pxl-core/src/{sched,lod}.rs): the chunk table, the frustum test,
     the LOD criterion and its blends, the priority order, the commits, the
     tier cache and the retain budget's reclaim, over a frame the glue
     writes where the module reads it; the node is a sink for what the
     module decides. Then the world's change log (crates/xpute-core/src/ipc/change.rs):
     the journal and the revisions are the module's, read as views; the
     object store bumps its column and cell topics inside its reindex, and
     the sources still in TypeScript — the page tables, decor's assets, the
     eraser's hide — bump through an export. Then the range mount: which
     chunks exist is the module's, from the range anchor, the render
     distance and its hysteresis in the frame and the pages with a tile
     (held to module/world/frame/range.ts's in_range_chunks by
     core_sched.test.ts); the stage makes a node and an anchor holder for a
     chunk the module mounts, and drops them when it unmounts. Then decor's
     build (2026-09-14, crates/pxl-core/src/world/decor.rs, held instance for
     instance to decor_kinds.ts's build_decor_instances by
     core_decor.test.ts — the scatter's streams bit for bit, splitmix64
     drawn as JavaScript rounds it): the kinds are taught with their rules
     as data (a channel is any type, void, or one of a list), the module
     builds a tier's decor beside the tier and hands it over as one block of
     lanes per (kind, variant) before the layout, a decor-only change
     (a tree placed, the eraser's hide, the setting) rebuilds it with
     `pxl_decor_now` and tells the layout again with the cubes left
     attached, and the block goes with the tier's eviction. The node holds
     blocks; the field reads their groups as views. Then the raycast
     (2026-09-14, crates/pxl-core/src/world/raycast.rs, held box for box and face
     for face to camera/ray.ts's chunk walk over chunk_raycast.ts by
     core_raycast.test.ts): the chunk DDA, the cell DDA and the slab test
     over the scan's column walk, and compositor.ts's little cache of
     composites — now the module's, keyed by the chunk and the count its
     sources were told, composed of the real set with no hover edits — over
     the chunks the scheduler holds. Retired to `archive/20260914-ts-raycast`:
     the TypeScript walk of the world, the ray LRU with its layer
     revisions, and the temporary composite for a drawn build. The
     TypeScript composite and the chunk walk stay as the references.
     What is left outside the module, and the line it moves to — JavaScript
     keeps what the browser provides, interrupts, I/O and the UI, and
     nothing that holds or decides the world's state
     ([20260914-what-stays-in-javascript.md](./20260914-what-stays-in-javascript.md))
     — in the order each unlocks the next: the view with PVF3 (the orbit
     camera, the root frame, the damping, map_center, the range anchor;
     pointer events as commands, the frame written by the module), page
     demand and residency with an XTP reader in xpute-core (the module
     says which page it wants, the glue lands the bytes, the module reads
     them where they lie), the POI index and placement, the object
     store's persistence (the record the module's, the write I/O). The
     crates are cut by layer on the way — `pxl-render` next, the
     renderer's contract (draw list, block layouts, shaders, pipeline
     records) that six world modules fill and nothing in it reads back;
     `pxl-shared` when PVF3 and the reader give it a contract to be.
     XTP packets land aligned (8 bytes, 16 for SIMD) and are read through
     checked casts. The glue and the core talk through rings in the core's
     linear memory with a drain export as the doorbell — the submission
     ring is built here, where the boundary first exists
     ([boundaries, "Rings"](./20260913-boundaries-commands-and-what-comes-back-from-april.md)).
     Built 2026-09-14 (crates/xpute-core/src/ipc/ring.rs, @xpute/core/ipc/ring.ts):
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
     bump allocator (crates/xpute-core/src/ds/fixed_heap.rs) since the memory never
     grows; the module states its size and the client checks the map
     against it at instantiation. The same day, the buddy over every
     section's range moved into the module (crates/xpute-core/src/ds/buddy.rs,
     held to the TypeScript pool's addresses request for request by
     core_pool.test.ts): the module is instantiated before the map is cut,
     and the TypeScript pool names blocks through it. Then the volume
     (crates/pxl-core/src/bake/volume.rs): bands are the module's, taken from the
     fixed section for the sixteen standing stores and the mesh section past
     them, every paint a call into it, and readers get views; the Rust scan
     opens a volume by id. Then the terrain layer's reading of a page, and
     the object store with the object layer's paint, the direct path's
     writer and the raycast walk, and the chunks' builds under one tick
     through the rings (2026-09-14). The moves read against the
     TypeScript they replaced the same day
     ([the safety valves the module did not have](./20260914-the-safety-valves-the-module-did-not-have.md)):
     four things the runtime around the TypeScript had done — a React
     child asking for the next frame, a page painted from past the page
     section, a pool that spilled rather than refuse, a page mounted as
     one React commit — had no counterpart in the module, and each was a
     stall; the module has its own now. **Before the renderer, the
     performance pass the profile of 2026-09-14 points at** (measured with
     the ledger, dev build, desktop). Taken the same day, on the module's
     own counters: a chunk with nothing placed takes the direct path at
     every tier, since its runs are the same either way (pinned by
     core_build.test.ts) — a zoom-in scanned every chunk in view, about 25
     a second, most of them empty of objects; direct runs go with the last
     tier wanted rather than filling the section with small blocks; and a
     frame that had a block refused releases off-screen layouts at 64 a
     frame rather than 2, since what they still show is the room the
     refused block needs. A zoom-in over a thousand chunks settles in a
     second with under a hundred refusals, from six seconds and counting.
     Left for the frame-based pass after the renderer: `BakeContext::step`
     itself (face masks and the mesh write), and a large-block region if
     refusals show again. What the profile
     also shows and the renderer removes: three's `updateMatrixWorld` over
     2,500 groups (1.9 s of 27), and decor drawn for the whole loaded
     world (tree 281k, car 99k triangles a frame; P3 below). The renderer
     is through (2026-09-14, render/gpu, crates/pxl-core/src/{draw,ghost,
     model,marker,billboard,environment,debug}.rs): the module lists what
     to upload, free and draw, and a WebGPU canvas executes it with no
     decision of its own — the sky, the chunks (three's Lambert in WGSL),
     decor as instanced draws of the module's blocks, the hover ghost
     (which ended the ghost faults recorded here since the object store
     moved), the companions as actors of the module with their palettes
     written each tick, the location puck, the billboards (badges and the
     chat bubble, over one sheet, with the depth trick asked of this
     frame's depth), the environment (sun, moon, light and sky from the
     clock and the theme), the dev gizmos. The view — camera and root frame
     — is doubles of our own (module/world/frame/view.ts), the models are read by
     a parser of our own (render/gltf), decor's eraser hit test is the
     module's over its blocks and triangles, the picker's preview is a
     display case of the renderer, and the frame loop is the tick's own
     (sched/tick.ts). Three, R3F, drei and postprocessing are out; the
     retired paths are on archive/20260914-three-draw-paths. The post
     effects are in (2026-09-14, render/gpu/{frame_wgsl,shadow,depth,
     mips}.ts): one shading source every lit pass calls — the rim light,
     the sun's shadow map (2048², 1024² handheld, 3x3 PCF), the vignette
     and the grade folded into forward shading rather than a screen-sized
     post pass — 4x MSAA as a quality setting, mip chains on the sheets and
     layers, reversed-Z (`depth32float`, the compare turned) for the far
     coplanar fighting, and the GPU's own time by timestamp query. What each
     target costs is the ledger's `targets` line; the MSAA comparison still
     to be taken is in
     [20260911-taa-tried-and-closed.md](./20260911-taa-tried-and-closed.md).
     The shaders and the pipelines are the crate's (2026-09-14,
     crates/pxl-core/src/render/shader.rs and shader/*.wgsl): every pipeline a
     record of words — shaders, vertex lanes, bind group layouts,
     primitive, depth and blend state — that render/gpu/pipeline.ts turns
     into a descriptor, the passes owning no literal; `cargo test`
     compiles every shader with naga and holds each record to what its
     shader declares (inputs by location, bindings by group and slot, the
     frame uniform's offsets), and the glue reads the frame layout from
     the module. The WGSL is text the GPU process compiles either way;
     what the move bought is one home for the lanes' layouts, the slots
     and the constants, not concealment. Open, minor: one POI page
     (6362873910329344) read as a truncated packet on one machine under
     dev:remote, the fetched bytes whole in the console — into_core's view
     the suspect, not reproduced elsewhere. For the frame-based pass
     after it, at least considered: work that is data-parallel per element
     and whose result only goes to the screen, moved into compute — culling
     into an indirect draw buffer (`drawIndirect`) in place of a listed draw
     a chunk, decor instances filtered by tier and range, and a chunk's face
     culling. What the CPU has to read back stays the module's: a readback
     arrives frames late, and WGSL has no 64-bit type for the coordinates,
     no allocation and no I/O. The mesher's face culling is the open
     question, since the raycast and the caches read what it produces.

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
  built against it. First seen on the desktop, 2026-09-14, after a zoom
  through a city: on-screen work 158 MB against the 128 MB budget, the
  mesh section 181 of 256 MB live and the rest lost to the buddy pool's
  rounding (a request takes the power of two above it), off screen
  nothing left to reclaim, 1,091 chunks pending on tiers whose blocks the
  pool refused, for the rest of the session — the bake slot held by one
  of them, so nothing else baked either, and the tick spinning a hundred
  milliseconds a frame over the refusals. Built the same day: a refused
  block ends the tick's builds and commits, the refused chunk gives up
  its slot and is not tried again until the pool gives a block back
  (`PoolStat::releases`), and the budget is paid in LOD — the ladder's
  pressure (lod.rs): a step in when a block is refused and reclaim found
  nothing, at most every half second, up to three tiers; a step out after
  five seconds without a refusal with the section off its mark; the
  ledger's `lod ... pressure`. Still open: what the rounding costs the
  section (about a third at the sizes tiers come in) — size classes
  between the powers of two, or a large-block region — once the pressure
  has been seen doing its work.

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

- **The scheduler's port, read against the TypeScript it replaced.**
  Read line for line on 2026-09-14 after the move (crates/pxl-core/src/world/sched.rs
  against 9ff57fe's chunk_node.ts, chunk_stage.tsx, chunk_table.ts,
  priority.ts). Fixed: the reclaim walked the order backwards and gave back
  the nearest off-screen chunk first (core_sched.test.ts now pins farthest
  first); a layout the node re-attached on its own — decor changing — lost
  the blend the module does not repeat. Brought back to the TypeScript's
  behaviour: builds move at most eight chunks a frame, counting only chunks
  that made progress; the pending lists are taken before the recheck; a
  rebuild on the spot trims the tier cache. Hardened: a layout naming a tier
  the node does not hold is warned and the chunk rebuilt from its sources,
  outside the ring read, instead of throwing inside the frame. What has no
  equivalence test is this layer: the pieces under it (mesher, scan, volume,
  terrain, object store, build) are held to the TypeScript byte for byte,
  the scheduler only by its order's unit tests and core_sched.test.ts's
  cases — worth a trace-driven comparison if its behaviour is ever in doubt.

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
