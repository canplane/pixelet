# A tile is asked where it lies, and the ray reads its height

**2026-10-28** — decided by the user. Written against main at `ede581671`.
Under the floor's zoom, a ground tile is asked for by where it lies in x and y
alone. The ray decides whether it is drawn, and at which zoom, from the height
inside the query. A ground tile carries one height of its surface, its samples,
and no second one is added to it, neither a mean nor a bound. A bound baked into the tile was
proposed twice, on 2026-10-17 and on 2026-10-28, and both times it was dropped.

## The user's case

- An eye far above the ground, out in space, is no reason to ask differently.
  Fetching by x and y is enough, because whether a tile is drawn is the ray's
  question, and the ray's query is where the ground's height is read.
- The ground already holds its height. A second height would hold again what
  the tile has.

## What holds

- **The demand is by x and y.** At each zoom under the source's top it asks
  every tile within that zoom's reach of the eye
  (`pxl-kernel domain/view/observation.rs` `ByEye`), a distance in which the
  eye's height plays no part (`pxl-spec space/layer/demand.rs` `band`). That
  keeps the demand a function of the eye alone, never of what has arrived. The
  top zoom is the floor's, whose ring the eye's height over the ground narrows
  (`pxl-kernel domain/layer/residency/floor.rs` `ring`).
- **The ray reads the height.** It picks a cell's zoom by its 3D distance from
  the eye (`pxl-spec-core grid.rs` `cell_at`), inside the world's one query. A
  zoom that has not arrived is read from the coarser one that has.
- **A horizontal distance is never longer than a 3D distance.** So the demand
  may ask for more than a ray reads, never less. The excess is bounded: a zoom
  asks for at most its lattice's `reach_tiles` wherever the eye stands, the
  ground's `REACH_TILES` (`pxl-spec space/layer/demand.test.rs`
  `a_zoom_demands_at_most_its_reach_in_tiles_wherever_the_foot_stands`). The
  residency's frames are counted at compile time from that bound
  (`pxl-kernel domain/layer/residency.rs` `FRAMES`), never from the eye's
  height, so an eye high above the ground holds no more than one standing on
  it.
- **The tile holds one height of its surface.** A terrain packet is its
  samples `ele`, its looks, its occupancy and its separation
  (`pxl-spec bake/tile/terrain.rs` `encode`); the separation's heights are its
  ways' tops and clearances (`pxl-spec bake/separation.rs`), not the ground's. A coarser tile's samples are the box mean of those under it
  (`pxl-spec bake/fold.rs` `fold_ele`). They are the surface drawn at that zoom,
  not a bound on the finer surfaces under it. Its padding is taken from its
  neighbors' samples, its own edge repeated where a neighbor is not baked yet,
  so the tile's edge can be drawn without them, and is no second height.
- **Air is skipped by the windows' levels.** Each square holds, per level, the
  lowest and highest under an entry, the ground's and the ways' apart
  (`pxl-spec-core window.rs` `level_at`), made on the client from what the
  window holds (`pxl-spec-core fill.rs` `level`).

## Why no bound goes into the tile

- The demand needs none. Its rule is a ratio of distance to cell size
  (`pxl-spec space/camera/grid.rs` `split_distance`). A coarse surface differs
  from the fine one by the relief inside one of its cells, and that difference
  is small against the distance at which the cell is drawn.
- The ray needs none past what the windows' levels give, since those are made
  from what the window holds.
- A bound beside the samples is a second form of the height they already hold,
  which the reshape skill forbids. Measuring the demand in 3D would also have it
  read heights that arrive, where `ByEye` asks by the eye alone.

## The two times it came up

- **2026-10-17.** An agent found that a mean folded upward loses a lone column:
  a column of 40 over flat ground reads 10 one zoom up and 2 two zooms up. A
  vertical ray could then read that column as air. The agent proposed baking a
  per-tile [min, max] beside the mean, a maximum mipmap (Tevs, Ihrke, Seidel
  2008). On 2026-10-19, another agent found that the windows' levels already
  are that structure. That stretch was squashed into `36f84e955`, and the tile
  at HEAD carries no such lane.
- **2026-10-28.** A per-tile max height in terrain tiles, FORMAT 13, was built
  in a private export so the demand could measure in 3D. The bake started.
  Then it was stopped and the work was discarded before any commit, because it
  was a rule on the ground alone. The user's case above then settled that the
  demand needs no height at all.
