# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **The move to a Rust core in WASM, drawing through WebGPU.** Scheduled
  2026-09-13 ([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md))
  and through by 2026-09-15: the kernel (`crates/pxl/kernel`, built by
  `crates/pxl/wasm` into one module whose exports are `boot` and
  `interrupt`) builds and schedules the chunks, holds the world, the camera,
  the map, the stores and the fetch scheduler, and lists what to draw; the
  shell forwards events, runs fetches and loads, and executes the draw list
  through WebGPU
  ([20260914-what-stays-in-javascript.md](./20260914-what-stays-in-javascript.md)).
  How it got there is that document,
  [the audit after the move](./20260914-the-audit-after-the-move.md),
  [the safety valves](./20260914-the-safety-valves-the-module-did-not-have.md)
  and the commits. What is left of it:
  - **The frame-based performance pass.** `BakeContext::step` itself (face
    masks and the mesh write), and a large-block region if refusals show
    again. At least considered: work that is data-parallel per element and
    whose result only goes to the screen, moved into compute — culling into
    an indirect draw buffer (`drawIndirect`) in place of a listed draw a
    chunk, decor instances filtered by tier and range, and a chunk's face
    culling. What the CPU has to read back stays the module's: a readback
    arrives frames late, and WGSL has no 64-bit type for the coordinates, no
    allocation and no I/O. The mesher's face culling is the open question,
    since the raycast and the caches read what it produces.
  - **Open, minor:** one POI page (6362873910329344) read as a truncated
    packet on one machine under dev:remote, the fetched bytes whole in the
    console — into_core's view the suspect, not reproduced elsewhere.

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
  First seen on the desktop, 2026-09-14, after a zoom through a city:
  on-screen work 158 MB against the 128 MB budget, the mesh section 181 of
  256 MB live and the rest lost to the buddy pool's rounding (a request
  takes the power of two above it), off screen nothing left to reclaim,
  1,091 chunks pending on tiers whose blocks the pool refused, for the rest
  of the session — the bake slot held by one of them, so nothing else baked
  either, and the tick spinning a hundred milliseconds a frame over the
  refusals. Built the same day: a refused block ends the tick's builds and
  commits, the refused chunk gives up its slot and is not tried again until
  the pool gives a block back (`PoolStat::releases`), and the budget is paid
  in LOD — the ladder's pressure (lod.rs): a step in when a block is refused
  and reclaim found nothing, at most every half second, up to three tiers; a
  step out after five seconds without a refusal with the section off its
  mark; the ledger's `lod ... pressure`. Still open: what the rounding costs
  the section (about a third at the sizes tiers come in) — size classes
  between the powers of two, or a large-block region — once the pressure
  has been seen doing its work.

- **XTP: a string-sequence leaf.** A `str` sequence leaf — one UTF-8 blob
  plus a u32 offsets lane — in place of one `str` node per string. A new
  type code, so existing packets still read. Per string today is an 8-byte
  header, a NUL and word padding (the POI repack: 60 MB gzipped against
  51 MB of JSON) and one node object at encode.
- **XTP encoder without per-leaf objects.** A node object per leaf and a
  closure per scalar at encode; mechanical, format-unchanged. Measured on
  the rings (2026-09-14,
  [experiment/20260914-xtp-by-fixed-shape.md](./experiment/20260914-xtp-by-fixed-shape.md)):
  a ray query is 709 ns through TreeView and TreeReader and 215 ns with the
  request written by shape; the request's encode is 492 ns against 54, a
  reply's read 303 ns against 5. The shape of it is a writer and reader
  per command generated from `@pxl/shared/spec/cmd.ts`, over XTP, for both
  sides — the schema on the format, which also checks the packets' types.
  The pools raised it (2026-09-15): a block acquired and released is
  1,752 ns through the rings against 202 through the exports, a three-word
  packet 488 ns of it each way, on a path that runs per block.

- **Placed objects come back moved after a reload.** Found 2026-09-14: a
  wall of fifteen cubes (five along X, three up Z) reloads as thirteen
  cells on one Z layer spread along Y. In since v7.

  The cause first written here — that `local_world_to_canonical` is not the
  inverse of the page-local offset `encode_objects` writes — does not hold
  against the code as it stands (read 2026-09-17, by reading only; the
  symptom has not been reproduced since). Five links of that path were
  checked and each is symmetric:

  - the page base is `FROM_L3_IDX(page_idx)` on both sides, and `page_idx`
    is written and read as the same `u64`;
  - the fixed-point identity holds. `pvf3::to_real(pvf3::X(v))` is
    `(i * ONE + f) / ONE`, `ONE` is `2^FRAC_W` and an fvec lane is an
    unsigned residue in `[0, ONE)`, so it equals
    `projection::unproject_comp(i, f)` — which `project` inverts;
  - `ivec::sub` is per-lane with sign extension (`as_int_n`), not a packed
    subtract that could borrow across lanes;
  - the anchor is the same PVF3 on both sides: the encoder writes
    `entry.corner()` and `write_object_value` takes the corner as given;
  - `lower` and `corner` are inverses — `pvf3::sh` by `-OBJECT_LAYOUT.min`
    one way and `+min` the other.

  What is not ruled out: `x`/`y`/`z` are `f32` lanes in the saved form
  (`object/store.rs`), so a coordinate's precision is f32's at page-local
  magnitudes; and a chunk's entry lanes hold one entry per cell for a leaf
  larger than one, each carrying `entry.size`, so what a large leaf is on the
  way back out is worth checking. Fifteen becoming thirteen is two placements
  landing on one cell, which is what either of those would do.

- **The POI index with its pages, and POI back on.** Disconnected
  2026-09-15: the index took about 20 MB of the 32 MB module heap before a
  record existed — two sets of lanes and a spatial hash reserved at a fixed
  65536 (`poi_index.rs:68-136`), copied out of the pages the page section
  already holds — and past its heap the kernel traps rather than refuses
  ([the heap all of it runs in](./20260915-the-audit-against-the-rings-checkpoint.md)).
  Terrain is bounded by its section and backs off; POI should be too: each
  resident page carries its derived lanes (grounded height, cell, tie-break,
  tier) and a spatial hash of its own in blocks of the page section, a query
  walks the pages in reach, and the capacity is the resident page count. Then
  the module heap is sized to what is left in it.
- **Strings out of the kernel's records.** Most of the kernel's `String`s
  name something rather than hold text — a task's key is a category and a
  page, its error a status — and a record holding one cannot sit in a lane of
  a section, so the ledger's task ring stays on the module heap while its
  frame ring moved out (2026-09-15,
  [the heap all of it runs in](./20260915-the-audit-against-the-rings-checkpoint.md)).
  A name becomes numbers (a category index, a page index, `fnv1a64` of what
  is not one); text that is data — a query, a place's name, an error's detail
  — goes as bytes into a UTF-8 lane with an offset lane beside it, the shape of
  the POI page's strings and of the string-sequence leaf below; what is built
  to be handed to the shell once (a report, a log line, the snapshot) stays
  on the heap.

  Done 09-16/09-17, in the records the shell reads: a `DemandKey` is a
  category and a `u64` subject, a failure is `FetchFail`'s number with its
  status beside it, a `Message` and a `CameraTerrain` are numbers, a
  placeable cube is a sum type over the catalog's own numbering, and the
  ledger's report, trace and live stream are documents of numbers the shell
  writes the words for. A colour is a number too — the catalog's swatches and
  the themes' lighting are packed by the generator, and nothing in the kernel
  parses `#rrggbb` any more.

  Not to be done, checked 09-17: the panic messages. They looked like the
  same kind of thing and are not. 200 runtime sites carry 6,287 bytes of
  prose, and 188 of them assert an internal invariant — "the cell is
  indexed", "the direct writer left open" — with runtime values interpolated
  into a good many ("pipeline {id}: location {location} fed twice"). An errno
  cannot carry either. The split exists for a consumer that routes on the
  code, as the shell routes on `FetchFail`; nothing routes on a panic, because
  the module traps and the client is over. Twelve sites name a resource
  condition, and of those only four can be reached on a device
  (`render/gpu/renderer.rs` twice, `render/gpu/api.rs`, the outbox's full
  spill) — what those want is to refuse rather than trap, the way the decor
  scratch and the mount pass were made to, not a number nobody reads.

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
- **Terrain page blocks with the page.** `terrain/resident.rs` keeps the
  pool blocks a page owns in a table of its own; they belong in the page
  table's handle beside the views.

## P3

- **Decor culling by page.** Decor is drawn for the whole loaded world
  in one instanced draw per variant; with the threshold it exists only in
  the nearest ring, so culling saves the half behind the camera. If the
  phone re-measurement still puts decor first, split into one draw per
  page and variant and cull by page bounds.
- **First load on a phone: the fetch queue, not the network.** Terrain
  and POI pages p50 9.7 s / 13 s from request to adopt, of which the
  network was 1.4–1.9 s; the rest was queue wait behind `MAX_INFLIGHT`
  6 with 64 pages asked at once, 4.2 MB per terrain page. The ordering
  is right (nearest first); what is worth asking is whether the quota
  and the page size are, on a phone's connection.
- **The run set's per-axis inset has no writer.** `sub_sx`/`sub_sy` are
  always a copy of `sub_sz` (the scan, the coarse pack and the object
  adapter all set the three equal), so the per-axis design column_run.ts
  documents — a pillar narrow in XY and full in Z — costs two bytes a run
  for nothing today. Either the coarse bake learns to fit XY and Z
  separately, or the lanes go; decided with the Rust mesher, since the
  format is its input.
- **The build number moves to CI when there is one.** Official builds are
  counted like a build lab's (2026-09-13): one counter outside git, in the
  bucket at `meta/build-number`, taken by `deno task deploy` because that is
  the only central build today (apps/web/script/deploy.ts). A build lab
  numbers every build of main and ships some of them; with a CI building
  main, the increment moves into it, deploy picks one of its numbered
  builds, and the count continues from the counter as it stands — never
  reset.

- **The scheduler's port, read against the TypeScript it replaced.**
  Read line for line on 2026-09-14 after the move (crates/pxl/kernel/src/world/sched.rs
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

- **Coarse tiers: draw calls, a wider unit.** Not a bottleneck after the
  direct path; not without a measurement —
  [20260912-coarse-tiers-what-the-field-does.md](./20260912-coarse-tiers-what-the-field-does.md).
- **Characters and the rest without a record format.** Not before the format.
