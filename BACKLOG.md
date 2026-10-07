# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **How the samples are drawn.** On the iPhone 13 mini, four shapes over the
  same tiles and views: today's boxes, the same as masked faces, a ray a
  pixel, and a bounded observation a cell — what the rule asks for, a cell
  being an angular footprint and not a sample point. Each shape is built
  whole on the CPU and whole on the GPU, one representation either way:
  CPU or GPU is a question of the whole drawing, never of one kind of
  thing sent to the other side. The steps a cast pays
  are measured already, and where on the screen they lie
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./decision/20260928-what-is-drawn-is-what-the-screen-covers.md),
  "the steps, measured").

## P1

- **POI and reverse geocode are off.** The client leaves their roots
  unbound (`apps/web/src/kernel/boot.ts` `OFF`) while the terrain moves to
  Carto, Arnis and the zooms, one problem at a time; their tiles'
  names did not change (decision/20261006-a-zoom-names-a-page-grid.md), so
  turning them back on is checking them against the new terrain. Their
  demand now follows `POI_LATTICE`, not the ground's, and which children
  are tiles is the occupancy the bake writes, as terrain's is; the POI set
  is to be baked again in that format.
- **POI and reverse geocode follow Carto.** The ground is Carto's looks
  (decision/20261005-the-ground-is-a-carto-palette.md); a place's icon and
  its kind's label are still Overture's taxonomy as the search list names it
  (`pxl-geo-spec overture/properties.rs`), where Carto's `amenity-points.mss`
  gives each tag an icon and a color.

- **A tile's key does not name the code that baked it.** A region bake
  links its `--base` whole and rebakes only the region
  (`crates/server/geo-bake/src/bin/bake_tiles.rs`); nothing compares the
  base's bake code, which the manifest records only as a `git_sha`, so a
  region baked after a change to the drawn surface or the decks sits beside
  tiles from the old code with no error. The dev cache already keys by the
  bake binary and the holdings (`crates/server/geo-bake/src/demand.rs`
  `source_id`); that identity in every manifest, and a region bake refusing
  a base that differs in anything but holdings, closes it. Which leaves a
  coverage grow rebakes is the operator's `--region` list, where the
  coverage could say it.

## Parked, with the reason on file

- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./decision/20260930-wasi-and-the-boundary.md).
