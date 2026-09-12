# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **Publish the v3 POI bake.** The client now requests `poi/v3` and holds
  the page as an XTP packet (commit 3dd631c); the repacked bake is at
  `out/poi/v3` (22,259 pages, 60 MB gzipped, `manifest.json` inside),
  re-encoded from the local v1 bake, which matches the published v2 page
  for page. Upload it before deploying the client — a v3 miss is an empty
  page, so a client deployed ahead of the upload shows no places.

## P1

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

- **POI: two things seen 2026-09-12, both minor, both unexplained.**
  (a) On a fresh load the badges sometimes do not appear until the first
  pan, zoom or rotate — not reproduced in a session where nothing was
  touched and they appeared after a few seconds, so it is timing: a tile
  arriving after the index built it, or the placement effect not asking
  for a frame under `frameloop="demand"`. (b) Badges fight the terrain in
  depth: the occlusion shader's upright twin writes depth
  (billboard/anchor_occlusion.ts), and at the anchor dot it sits on the
  surface it tests against. Both are rendering, closed by rule; recorded so
  they are fixed together, with a repro, not chased on sight.

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

- **Decor: the projected-size cutoff and a low-poly tier per kind**, calibrated
  against how Google Earth and Maps draw 3D buildings by range —
  [20260911-decor-identity-orientation-and-one-cell-format.md](./discussions/20260911-decor-identity-orientation-and-one-cell-format.md).
  Rendering is closed until a measurement reopens it.
- **Coarse tiers: draw calls, a wider unit.** Not a bottleneck after the
  direct path; not without a measurement —
  [20260912-coarse-tiers-what-the-field-does.md](./discussions/20260912-coarse-tiers-what-the-field-does.md).
- **"Everything is a file."** One record format for objects, terrain and
  decor, extension headers per kind, glTF by external reference. After the
  linear-memory checklist; the POI packet above is its first record.
- **Characters and the rest without a record format.** Not before the format.
