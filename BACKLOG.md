# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

Nothing now.

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
  its kind's label are still this tree's own 35 kinds (`poi_badge.json`),
  shown by OpenMapTiles' class rank, and a name's depth is its own ladder
  (`spec/spec/place.json`). Carto is the default for both: a place's kind is
  the Carto feature its OSM tag draws as, with Carto's minimum zoom, icon
  group and color (`amenity-points.mss`), and a name's depth OSM's
  `admin_level` and `place=*`. The theme is fixed there; the data is not.
  Terrain's sources stay Overture and OSM, where POI and geocode take any
  provider (Naver for Korea, say) through an adapter mapping its vocabulary
  onto Carto's, as Overture's `category_of` does now. openstreetmap-carto is
  CC0, the cartographic design and `symbols/` included, so nothing binds its
  use; its name and OpenStreetMap's are trademarks, kept out of ours.

## P2

- **A marker keeps its own words.** Someone puts a marker up from the
  drawer, and every one reads "foo": the objects' store holds an id a leaf
  and no text. Its words come from the message input, and are kept beside
  the entry under a new storage key, with what words someone writes in the
  world bringing reporting and moderation nearer.

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
- **A bake is kept a shard at a time, as the ingest is.** The catalog says
  which shards the database holds, but a terrain bake keeps its state for
  the whole coverage: one land list (`land_tiles.json`), one `leaf.json` of
  what built the leaves, and a region named by tiles, `--grow` deriving the
  shards each run (`crates/server/geo-bake/src/bin/bake_tiles.rs`). Each
  shard's land list, leaves, coast and source identity its own would make a
  shard one unit from ingest to version: a shard ingested again is baked
  again alone, and `--region` and `--grow` become one question, which shards.

## Parked, with the reason on file

- **Publishing**: a push to `deploy`, any upload to the bucket (terrain, POI,
  geocode) and the copy into production's db-geo, held until the user says
  the time is right, since the bake side has refactoring ahead. Meanwhile a
  bake is looked at through `dev:baked` and `preview:baked`.
- **How the samples are drawn**, held until the user says the time is
  right. On the iPhone 13 mini, four shapes over the same tiles and views: today's boxes, the same as masked faces, a ray a
  pixel, and a bounded observation a cell — what the rule asks for, a cell
  being an angular footprint and not a sample point. Each shape is built
  whole on the CPU and whole on the GPU, one representation either way:
  CPU or GPU is a question of the whole drawing, never of one kind of
  thing sent to the other side. The steps a cast pays
  are measured already, and where on the screen they lie
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./decision/20260928-what-is-drawn-is-what-the-screen-covers.md),
  "the steps, measured").
- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./decision/20260930-wasi-and-the-boundary.md).
