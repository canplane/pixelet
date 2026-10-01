# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **Terrain v7, baked and published.** The client asks for terrain v7
  (merged to main 2026-10-01), so a deploy waits for it: baked and
  published after `deno task usage` (about 227,000 Class A) — once, not per
  revision of the format, and not by demand, since a page's index bounds its
  whole subtree and a coarse page cannot be made before everything under it.
- **The atmosphere in the observation's test, an optimization held.** A
  difference is observed where its projected angle leaks past a cell _and_
  the contrast it makes is still distinguishable after the air (`T(d)`,
  layer/environment/fog.rs). Transmittance weakens contrast, not geometry,
  so the two are tested side by side, not multiplied into one cell. With no
  bound on the contrast a region's representation can be off by, the
  contrast test is only today's far edge — the visual range, where `T`
  falls to the threshold — which is correct as it stands. What it would
  save inside the far edge needs a conservative bound on the difference a
  representation's error can make in the final image, and that is not a
  record's variance: it turns on material, light, shadow, emission and what
  stands behind a silhouette. No lane is added to a page until such a
  quantity is defined. The planet's horizon stays a separate, geometric
  limit.
- **How the samples are drawn.** Raster against casting into the records,
  on the iPhone 13 mini, with today's box format (36 unindexed vertices)
  against faces alone; seams where levels that are not neighbours meet, and
  a blend on arrival; decor as its own solver — instances at the asset's
  level of detail, an impostor far away, its lifetime off the chunk slot,
  and its hit with them: the pointer's ray scans the chunks shown, each
  grown by a reach derived from nothing (layer/decor/hit.rs `DECOR_REACH`),
  where the ground and the objects are each walked along the ray.
- **One order out from the eye, the rest.** Every scheduler takes work in
  one order out from the eye, a chunk is a cube of its level holding
  something, and a placed object lives in the cube its band is in
  ([20260926-the-view-is-cubes-about-the-eye.md](./20260926-the-view-is-cubes-about-the-eye.md)).
  Hidden ground is no longer asked for: demand is what the screen's tiles
  meet (world/observation.rs `ByFootprints`), and ground behind what they
  meet is required of no level. Left: a column crossing a cube's floor or ceiling is two chunks — 1.5 a column
  near Gangnam and 1.7 on Bukhansan, 2026-09-26 — each scanning the whole
  column's runs again, where the runs could be the column's once and each
  chunk a cut of them.

## P1

- **XTP: a string-sequence leaf.** A `str` sequence leaf — one UTF-8 blob
  plus a u32 offsets lane — in place of one `str` node per string. A new
  type code, so existing packets still read. Per string today is an 8-byte
  header, a NUL and word padding (the POI repack: 60 MB gzipped against
  51 MB of JSON) and one node object at encode.
- **Strings out of the kernel's records.** A name in a record is numbers
  now — the profiler's task row carries a category index and a subject,
  and sits in a lane of a section. What is left is text that is data — a
  chat line (layer/character/chat_bubble.rs), a place's name
  (layer/poi/badge.rs) — held as `String`s on the module heap: as bytes in
  a UTF-8 lane with an offset lane beside it, the shape of a
  string-sequence leaf, it could sit in a section too. What is built to be
  handed to the shell once (a report, a log line, the snapshot) stays on
  the heap.

- **Decor: the impostor tier.** Decided 2026-09-13
  ([20260911-decor-identity-orientation-and-one-cell-format.md](./20260911-decor-identity-orientation-and-one-cell-format.md),
  "Closed"): each variant rendered once to a sprite, far instances drawn
  as a camera-facing quad through the billboard field. The criterion is
  in and every shipped kind is on the nearest tier only; the impostor is
  what would draw them further for two triangles each. Not a phone fix —
  build it when a wider decor range is wanted.

## P3

- **The address label by the zoom.** Asked 2026-09-23, not now: it wants a
  rebake. The label is one baked answer a leaf page
  (`use-reverse-geocode.ts`, `geo-bake/geocode/provider/overture.rs`) — region,
  county, the fine polygon and a nearby point — shown whole at every zoom, so
  zoomed out to a province it still names a 동. The prior art is Nominatim's
  reverse `zoom`, which the client used before the bake: 5 a state, 8 a
  county, 10–13 a city or suburb, 14 on a neighborhood. Here, with level `k`
  at zoom 15 − k, something like the whole chain to level 2, region and county
  to level 5, region alone above.
  - Cut the leaf page's chain rather than bake one per level. A level page's
    center is not the map's: at level 4 a page is some 16 km, and its label
    would name the next county over. The leaf page is about a kilometer, so
    its chain is right and only needs shortening.
  - Each part carries its `subtype`, which means rebaking the geocode set
    and a new published version. Cutting by position is wrong where a level
    is missing: Sejong has no county, so "the first two" is not "region and
    county".
  - Zoomed out, a pan crosses leaf pages faster and asks more often; the
    500 ms debounce holds that to two a second, nothing against the Worker's
    100k a day.

- **What the sky left open.** Recorded 2026-09-22
  ([the sky is the air](./20260922-the-sky-is-the-air.md)), each small and
  none blocking:
  - The terrain stands at its elevation and the planet's ground past it at sea
    level, so from high up a far piece of terrain floats over it with its
    chunk walls showing. Setting the ground at the map center's height, or
    skirting the coarse levels down to it, would close it.
  - Clouds with a thickness: a slab a kilometer or two deep, sampled a few
    times a pixel, for the volume a side view misses — as a quality setting,
    since it is several times today's cost. Volumetric clouds proper are out
    while the client draws at rest every two seconds.
  - The Milky Way, which a naked-eye catalog does not make (a low-resolution
    all-sky texture, whose licenses are attribution ones), and the bright
    planets, which the sun's and the moon's low-precision formulas extend to.
  - The themes' remaining fill — ambient, hemisphere, the floor — from the
    sky's irradiance, the way the sunlight now comes from the air. The floor
    is there for a phone's night and would resist it.

- **A changelog, when `xpute` is published and not before.** Asked 2026-09-19
  and answered no: a changelog's reader is someone deciding whether to move
  between versions, and its load-bearing lines are the ones that change what
  that reader must do — removed, changed, deprecated, upgrade now. Continuous
  deployment has no such reader, since nobody chooses a build. What was wanted
  instead — which commits a build number carries — is a build log, and
  `check-and-deploy.yml`'s `record` job appends one line per published build to
  `_meta/build-log.jsonl`, subjects and all, which `./script/build-log.ts`
  reads back. Subjects rather than tags or ids because main's history is
  rewritten here routinely, and a name for a commit outlives the commit only if
  it is not a reference to one. The trigger that
  would make a real changelog earn its place is
  [the spin-out](./20260708-vision-and-scope-pivot.md): the day `@xpute/core`
  and `xpute-core` ship as packages, their consumers have neither this git log
  nor `issue/`, and the changelog belongs inside the package with its semver —
  generated from the commits at that point, not accumulated by hand until
  then. An installable build through Capacitor
  ([20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md))
  is the same trigger from the other side, since a store submission wants
  release notes per version. Until one of those, `issue/` is the history and it
  records why, which a changelog never does.

## Parked, with the reason on file

- **Coarse tiers: draw calls, a wider unit.** Not a bottleneck after the
  direct path; not without a measurement —
  [20260912-coarse-tiers-what-the-field-does.md](./20260912-coarse-tiers-what-the-field-does.md).
- **Characters and the rest without a record format.** Not before the format.
- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./20260930-wasi-and-the-boundary.md).
