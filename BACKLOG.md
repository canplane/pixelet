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

- **POI under the tick.** Opened by four phone reports on 2026-09-13 (iOS
  Safari, dpr 2; no GPU timer, no LoAF, no longtask). Steady state after
  load is fine — dt p50 15 ms on a 13 ms display, the tick 5 ms. The
  stalls (dt p95 220 ms, a third of moving frames missed) went away with
  POI off and stayed with decor off, and three mechanisms are named:
  - **The badge atlas uploads whole.** `atlas.ts`'s `add` sets
    `needsUpdate`, so three re-uploads the full canvas at render — 64 MB
    at 4096 px — once per frame a badge was built in; seen as
    `task ~105 / sched 3` seven times in 25 s, and as the same copy off
    the thread (`task 5 / offthread 220`, `Δtex`) where iOS moves it to
    the GPU process. Upload the badge's rectangle: a scratch texture and
    `copyTextureToTexture(src, atlas, region, position)`, flushed from
    the field's cosmetic step with `state.gl`. Growth and compaction
    still replace the sheet whole, which is once.
  - **The index sweep's unit is a page.** `poi_index_sweep` runs under
    the budget, but `budget.run` over pages lets a page of thousands of
    records be one step — `content/poi_index 1.5/45 ms` in the step
    table — and `back.index.build()` is outside the budget. The unit
    becomes a record range; the build is sliced or bounded.
  - **Placement is a React effect.** `poi_layer.tsx` places on an epoch
    that a state update advances, so the pass (2 ms typical, 180 ms seen
    over a cold index) runs outside the tick, unbudgeted, and the ledger
    books it as off-thread. It becomes a content step that cuts its
    candidate loop under the budget.
  Land all three, then measure once
  ([20260909-cooperative-scheduler-and-task-ledger.md](./discussions/20260909-cooperative-scheduler-and-task-ledger.md),
  "Method note").

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

- **Decor: the projected-size criterion, then the low-poly tier.** Decor
  is 87% of the phone's triangles (tree 386k, car 145k of 390k mean; peak
  1.09M) and the A/B of 2026-09-13 showed it is not the phone's stall:
  with decor off, tris fell 58% and dt p50/p95 and the missed share did
  not move. So this is battery and headroom, not the stutter. The design
  is settled in
  [20260911-decor-identity-orientation-and-one-cell-format.md](./discussions/20260911-decor-identity-orientation-and-one-cell-format.md)
  (follow-up of 2026-09-12): a kind brings its extent, one pixel
  threshold shared with the terrain tiers decides full / low / none, and
  the hand-written density ladders go. Extents measured from the GLBs:
  trees 2.4–3.0 high on a 1×1 footprint, cars 1.5×2.6–2.9, trains
  1.3×2.5, boats up to 2.8×3.9.
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
