# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

Nothing queued.

## P1

- **The chunk bake to a worker.** Decided 2026-09-13, not conditional;
  the case, with the phone's numbers, in
  [20260909-cooperative-scheduler-and-task-ledger.md](./discussions/20260909-cooperative-scheduler-and-task-ledger.md)
  ("The worker's case"):
  a bake is 14–21 ms a unit on the phone against an 8 ms budget, the
  unit is a chunk and the mesher is one call, so on the main thread it
  cannot be cut and forward progress runs it whole (`visible/chunks` max
  64–112). Off the thread its main-thread cost is zero; that is
  arithmetic, not a hypothesis. The memory side is ready — sched/slabs.ts
  holds the little memory and its ownership record — and the mesher takes
  typed arrays and returns typed arrays. The job is a Demand through the
  fetch scheduler whose run borrows a slab, copies the run set in from
  the big memory, posts it, and copies the mesh back. The same worker
  hosts the WASM bake later
  ([20260911-single-thread-core-and-when-to-spawn-a-worker.md](./discussions/20260911-single-thread-core-and-when-to-spawn-a-worker.md)).
- **"Everything is a file": the Placement and the palette.** Decided
  2026-09-13, first:
  [20260913-everything-is-a-file-the-placement-and-the-palette.md](./discussions/20260913-everything-is-a-file-the-placement-and-the-palette.md).
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
  - **The bucket path and the ingest**: models by content hash, the
    palette row written by the server, `model_hash` filled.
- **The phone, at the end of 2026-09-13.** Same spot, same procedure,
  after POI went under the tick, the badges became one instanced quad,
  the companion stopped probing, placement was cut into units and decor
  took the projected-size threshold: dt p50 17 ms on a 16 ms display, the
  frame's own task 2 ms, 13% of moving frames missed (from 39%). The
  render-path bisect put MSAA and the composer at about a millisecond of
  p50 and five points of the missed share — not worth removing. What is
  left, in order:
  - **A ~300 ms off-thread frame every twenty seconds or so**, on a still
    frame with nothing mounted or uploaded. Outside the task and without a
    GPU timer or LoAF on iOS there is no instrument for it; Safari's own
    Timelines would be the next look, if it is ever the complaint.
  - **A chunk bake is 14–21 ms on the phone** (3 ms on the desktop) while
    a zoom-out is filling in: the worker, above.
  - **A repacked sheet still uploads whole.** Growth replaces the texture
    and its first bind uploads the canvas — 16 MB at 2048, 64 at 4096 —
    once per size step (`poi_field` max 76 ms on the phone, 151 at 4096).
    An empty texture plus the live rectangles would be a third of that.
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
    [20260910-why-xtp-and-what-would-end-it.md](./discussions/20260910-why-xtp-and-what-would-end-it.md):
    `[keys, count, columns...]` for homogeneous records, a shape table for
    the few heterogeneous ones. The POI page is positional today, its
    field order held by format.ts's encode/decode pair; the key list makes
    it self-describing and lets a reader skip a column it does not know.
    Not before a second record kind exists — one record does not need a
    convention, and building it twice is the cost to avoid.
- **XTP encoder without per-leaf objects.** A node object per leaf and a
  closure per scalar at encode; mechanical, format-unchanged, bake-side
  only. With the above, not on its own.

## P2

- **Decor: the impostor tier.** Decided 2026-09-13
  ([20260911-decor-identity-orientation-and-one-cell-format.md](./discussions/20260911-decor-identity-orientation-and-one-cell-format.md),
  "Closed"): each variant rendered once to a sprite, far instances drawn
  as a camera-facing quad through the billboard field. The criterion is
  in and every shipped kind is on the nearest tier only; the impostor is
  what would draw them further for two triangles each. Not a phone fix —
  build it when a wider decor range is wanted.
- **Terrain page blocks with the page.** `resident.ts` keeps the pool
  blocks a page owns in a `WeakMap<Terrain, Block[]>`; they belong in the
  page table's handle beside the views.

## P3

- **Spill past the reservation must become a budget signal before a WASM
  kernel reads the pool.** The core memory is fixed (`initial == maximum`,
  never grown — grow would detach every view, and resizable buffers read
  2.5x slower and are refused by WebGL/WebGPU); past 512 MB the buddy pool
  hands out plain ArrayBuffer regions, which a kernel over the wasm memory
  cannot see. Harmless while everything computes in JS; when a kernel
  lands, a spill has to trigger reclaim or refuse, not silently leave the
  memory. Today's live set is a third of the reservation.

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
- **POI fetch on an empty body.** `fetch_poi` throws `Unexpected end of
  JSON input` when the proxy returns an empty 200 (seen 2026-09-12, several
  pages at once); the page is retried by the next sync. Goes away with the
  XTP page (a short packet fails the header check instead) — verify then.

## Parked, with the reason on file

- **Layout bucket handle table.** Each node's two `Map<LodStep, …>` caches
  as a fixed (slot × tier) handle table. The vertex data is already pooled
  and three.js's `BufferAttribute` takes the typed views, so the handle is
  the glue boundary; this removes two Maps per node and nothing else.
  Deferred 2026-09-12.

- **Coarse tiers: draw calls, a wider unit.** Not a bottleneck after the
  direct path; not without a measurement —
  [20260912-coarse-tiers-what-the-field-does.md](./discussions/20260912-coarse-tiers-what-the-field-does.md).
- **Characters and the rest without a record format.** Not before the format.
