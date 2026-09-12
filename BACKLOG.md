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

- **The phone, after POI went under the tick (2026-09-13).** Same phone,
  same procedure: dt p95 227 to 68 ms, the upload tasks gone. What the
  next report showed, in the order of steady-state cost at a zoom with
  1.26M triangles on screen:
  - **Decor is the GPU load at that zoom.** off-thread p50 11 ms at 363k
    triangles, 17 ms at 780k–1.26M, and 87% of them decor. The near-zoom
    A/B cleared decor of the stalls, not of this. The projected-size
    criterion is in at twelve pixels (every kind on the nearest tier
    only); re-measure this spot on the phone. If decor is still the cost,
    the next lever is culling by page (P3).
  - **`interaction/companion` 4.8 ms a frame, max 299.** The largest single
    step on the phone and O(1) by contract; two walkers each sampling the
    surface every frame. Profile the step before touching it.
  - **A chunk bake is 14 ms on the phone** (3 ms on the desktop), so the
    visible phase's unit exceeds the budget and forward progress runs it
    anyway: `visible/chunks` max 112, commit max 77. The unit is a chunk
    and the mesher is one call; splitting it is the scan/bake boundary or
    the worker (P3).
  - **The content share** is in: content gets 20% of the budget even when
    the visible phase would take all of it. Re-measure `wait place` on
    the phone; it read 3.3 s over 25 frames before.
  - **A repacked sheet still uploads whole.** Growth replaces the texture
    and its first bind uploads the canvas — 16 MB at 2048, 64 at 4096 —
    once per size step; the desktop read a 580 ms off-thread frame at the
    step to 4096. An empty texture plus the live rectangles would be a
    third of that.

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

- **Move the chunk bake to a worker when the ledger says so.** The signal
  is the visible phase spending its whole share frame after frame with
  pending not falling (today: p95 at the still budget, pending 0). The
  memory side is ready — sched/slabs.ts holds the little memory and its
  ownership record — and the job is a Demand through the fetch scheduler
  whose run borrows a slab; what is left is the worker itself and the copy
  in and out of big. Not before the signal
  ([20260911-single-thread-core-and-when-to-spawn-a-worker.md](./discussions/20260911-single-thread-core-and-when-to-spawn-a-worker.md)).

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
- **"Everything is a file."** One record format for objects, terrain and
  decor, extension headers per kind, glTF by external reference. After the
  linear-memory checklist; the POI packet above is its first record.
- **Characters and the rest without a record format.** Not before the format.
