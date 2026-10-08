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

- **Places is off.** The client leaves its root unbound
  (`apps/web/src/kernel/boot.ts` `OFF`) until places are drawn as the next
  item says; the reverse geocoding is on again, read as divisions. Its demand
  follows `PLACE_LATTICE`, not the ground's, and which children are tiles is
  the occupancy the bake writes, as terrain's is.
- **Places: Overture's schema, Carto's look.** A place's kind is still this
  tree's own 35 (`place_badge.json`), shown by OpenMapTiles' class rank. What
  a place is follows Overture's schema, as the divisions now do: its
  taxonomy and basic category, its names, confidence, sources and GERS id,
  since Overture was made to merge vendors' places and every vendor's tree
  (Naver, Kakao, Google) maps onto it as Overture's own sources do; OSM's
  tags are an editors' vocabulary, a kind spread over several keys. How a
  place is drawn follows Carto (the ground is its looks,
  decision/20261005-the-ground-is-a-carto-palette.md): one table, the
  theme's, from an Overture kind to a Carto icon group, color and minimum
  zoom (`amenity-points.mss`). A provider is an adapter onto the schema,
  baked or asked live as its terms allow; terrain's sources stay Overture
  and OSM. The taxonomy changes between releases, so the table is checked
  against the release the ingest records. openstreetmap-carto is CC0, the
  cartographic design and `symbols/` included; its name and
  OpenStreetMap's are trademarks, kept out of ours. `place` meaning a venue
  (`chunk/place.rs`) follows the schema's word in the same change.

## P2

## Parked, with the reason on file

- **Publishing**: a push to `deploy`, any upload to the bucket (terrain, places,
  divisions) and the copy into production's db-geo, held until the user says
  the time is right, since the bake side has refactoring ahead. Meanwhile a
  bake is looked at through `dev:shelf` and `preview:shelf`.
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
