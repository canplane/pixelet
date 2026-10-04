# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **How the samples are drawn.** On the iPhone 13 mini, four shapes over the
  same pages and views: today's boxes, the same as masked faces, a ray a
  pixel, and a bounded observation a cell — what the rule asks for, a cell
  being an angular footprint and not a sample point. The steps a cast pays
  are measured already, and where on the screen they lie
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./20260928-what-is-drawn-is-what-the-screen-covers.md),
  "the steps, measured").

## P1

- **Decor: the impostor tier.** Decided 2026-09-13
  ([20260911-decor-identity-orientation-and-one-cell-format.md](./20260911-decor-identity-orientation-and-one-cell-format.md),
  "Closed"): each variant rendered once to a sprite, far instances drawn
  as a camera-facing quad through the billboard field. The criterion is
  in and every shipped kind is on the nearest tier only; the impostor is
  what would draw them further for two triangles each. Not a phone fix —
  build it when a wider decor range is wanted.

- **Decor and the badges on the drawn chunks, seen running.** `4da038fc`
  and `98e4f790` moved them onto the chunks the terrain draws; checked by
  the tests and the build only. Zoom and pan on the dev stack: decor and
  badges change with the ground under them, and no tree is lost or doubled
  at a chunk's top or edge.

## P3

- **Strings out of the kernel's records.** Read 2026-10-01 and left on the
  heap: a chat line is one string set when it arrives, and a badge's text is
  built when the badge is, so neither touches the heap on a settled turn. A
  section would need a bound on a chat line's length, which is a product
  number not yet said; with one, the line and its laid-out lines fit the
  string-sequence leaf's shape (XTP's STRS).
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
