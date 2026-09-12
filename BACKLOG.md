# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **POI pages as XTP.** The API's POI page becomes an XTP packet: coordinate
  and category lanes, names and details as `str` leaves read lazily; the
  client's page table holds the packet, the POI index reads the lanes, a
  badge is keyed by record index, a tap reads one record's strings. Both
  providers (Overture now, Naver if added) normalise on the server into the
  same packet. Re-bake the published POI tiles. Measured 2026-09-12 on a
  10k-record page: encode 2.1 ms, packet 648 KB against 858 KB of JSON; open
  and view the lanes 0.01 ms against 2.1 ms of `JSON.parse` plus a copy; 80
  names by index 0.06 ms.
  [20260912-linear-memory-what-is-left.md](./discussions/20260912-linear-memory-what-is-left.md).

## P1

- **XTP string-table leaf.** A sequence of strings as one UTF-8 blob plus a
  u32 offsets lane, in place of one `str` node per string: per string today
  is an 8-byte header, a NUL and word padding (~150 KB of the 648 KB above)
  and one node object at encode. The standard layout (Arrow, FlatBuffers
  vectors of strings). Wanted once POI names are in packets; not before.

## P2

- **Layout bucket handle table.** Each node's two `Map<LodStep, …>` caches
  (tiers, decor steps) as a fixed (slot × tier) handle table. The vertex
  data is already pooled and three.js's `BufferAttribute` takes the typed
  views, so the handle is the glue boundary; this removes two Maps per node
  and nothing else. Decide, do not drift into it.
- **`PageChunks` node map.** The per-page `Map<offset key, ChunkNode>` that
  React's mount diff uses could be a `find(base)` on the scheduler table.
- **Terrain page blocks with the page.** `resident.ts` keeps the pool
  blocks a page owns in a `WeakMap<Terrain, Block[]>`; they belong in the
  page table's handle beside the views.

## P3

- **Ledger ring as lanes.** The dev ledger's frame records are objects
  rebuilt every frame for the overlay. Measurement, not state; only worth
  it if the overlay itself shows up in a profile.
- **POI fetch on an empty body.** `fetch_poi` throws `Unexpected end of
  JSON input` when the proxy returns an empty 200 (seen 2026-09-12, several
  pages at once); the page is retried by the next sync. Goes away with the
  XTP page (a short packet fails the header check instead) — verify then.

## Parked, with the reason on file

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
