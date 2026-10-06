# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **Entropy left after the chunk rework** (decision/20261006-a-chunk-goes-past-the-leaves.md).
  The chunk still knows the ground: which tile it reads (`world/pyramid/chunk.rs`
  `tile_zoom`, `in_tile`, `tiles_beside`) and `chunk_zoom`/`LEAF_CHUNK` in
  `space/pyramid.rs`. Demand (`space/observation.rs` `ByBounds`, `band`) is
  the ground's but named as shared, where a layer could map the grid's cell
  to its own tiles. A chunk keeps two forms, a cover piece's one cell and its
  cells, with retiering between them (`world/chunk/table.rs`). Interaction
  asks the profiler's `drawn_at` whether a cell is folded
  (`control/interaction.rs` `folded_at`). The crates' unit tests were not run
  after the last edits of b3c81babc.
- **How the samples are drawn.** On the iPhone 13 mini, four shapes over the
  same tiles and views: today's boxes, the same as masked faces, a ray a
  pixel, and a bounded observation a cell — what the rule asks for, a cell
  being an angular footprint and not a sample point. The steps a cast pays
  are measured already, and where on the screen they lie
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./decision/20260928-what-is-drawn-is-what-the-screen-covers.md),
  "the steps, measured").

## P1

- **The object store is one hierarchy in two structures.** From the band up
  to `INDEX_TOP` a hash index keeps each cube's bound, finest zoom and fold
  (`layer/object/store.rs`); under it each band is a sparse tree, whose
  layout takes any root (`space/sparse_tree/mem.rs` `SparseTreeLayout`). A
  tree keeping those per node, rooted at the top, would leave no band and no
  second index; an entry's packed position is the format's own question.
  The render and storage hierarchies are to be reduced to one each.
- **POI and reverse geocode are off.** The client leaves their roots
  unbound (`apps/web/src/kernel/boot.ts` `OFF`) while the terrain moves to
  Carto, Arnis and the zooms, one problem at a time; their tiles'
  names did not change (decision/20261006-a-zoom-names-a-page-grid.md), so
  turning them back on is checking them against the new terrain.
- **POI and reverse geocode follow Carto.** The ground is Carto's looks
  (decision/20261005-the-ground-is-a-carto-palette.md); a place's icon and
  its kind's label are still Overture's taxonomy as the search list names it
  (`pxl-geo-spec overture/properties.rs`), where Carto's `amenity-points.mss`
  gives each tag an icon and a color.

## Parked, with the reason on file

- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./decision/20260930-wasi-and-the-boundary.md).
