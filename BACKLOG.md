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

- **Close the view's bound by area, and let the eye's rays measure it.**
  The cell is a solid angle (4b1fd556f) and a screen holds at most
  `GRID_CELLS_MAX`, but drawn cells are not yet held to a quarter of one:
  the error box is a cube judged from a chunk's nearest point, so ground
  seen at a grazing angle is drawn four and more times finer than its cell
  (the camera gizmo's blue, the far half of a 60° view). Decide in one
  design: the bound in the form it must hold (visible cells ≤ 4 × the
  grid's), what measures a cell's shown area — the rays of the camera grid,
  a cell's footprint on what it meets, cast on the CPU or read back from a
  GPU feedback pass as virtual texturing does (the decision of 20260928
  already names "the GPU's feedback" as a better estimator) — and how the
  choice stays on the main axis, the grid deciding the chunk, the chunk
  asking the query, nothing beside the hierarchy. Near "How the samples are
  drawn" above.

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

## P2

- **Badges are drawn by kind, not by the POI layer.** What is drawn and
  struck splits by render kind (cube, decor, billboard) and what is stored by
  where it comes from (the geodata's layers, the objects); decor is moving to
  that shape. A billboard today is drawn from the POI layer alone, so a
  user's signpost, an object whose id is a billboard kind, has no way on
  screen. The billboard kind joins the cell catalog and the badges take its
  records from whichever layer answers them, when a signpost is wanted.

- **Terrain's lanes are one record, which keeps it from being a quadtree.**
  A tile is its ground, elevation, decks and index together
  (`crates/spec/src/bake/format.rs` `Terrain`), each with its own lattice and
  fold (majority, average, union), so a child is left out only when all four
  are its parent's again, and elevation almost never is: every land leaf is
  stored. Each lane its own layer, pruned where its parent says it whole,
  would make the sea one case of that rule. First measured: how many leaves
  each lane alone, and all together, could leave out, against the requests
  more layers would cost under the Worker's daily cap
  ([decision/20260816-deployment-hosting.md](./decision/20260816-deployment-hosting.md)).

## Parked, with the reason on file

- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./decision/20260930-wasi-and-the-boundary.md).
