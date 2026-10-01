# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **A view's level from the eye, and the crossfade.** The level a piece of
  ground is drawn at is one rule for every layer, from its distance to the
  eye (`Tau::level`), finer than z16 too; what a layer draws at a level is
  its own tuning, never a second rule
  ([20260925-what-a-view-costs-is-the-eye-s.md](./20260925-what-a-view-costs-is-the-eye-s.md)).
  Which pieces refine is now the screen's cells
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./20260928-what-is-drawn-is-what-the-screen-covers.md)).
  What is left:
  - **What a crossfade leaves.** A change of what a chunk draws dissolves
    over 300 ms of frames (chunk/table.rs `Fade`), a cube for another and a
    stale chunk for what replaced it, its decor with it on the same half of
    the pattern (pxl-render dissolve.wgsl). The 300 ms is a number chosen by
    eye, to be measured. Above z16 a kind's footing and a transport-aligned
    kind's heading are read off the level's cells, so an instance can
    appear, go or turn at a level change, as the ground under it does.
- **What is drawn is what the screen covers: to main, and closed.** Built
  2026-09-28 on `observation-paging`, pushed, not on main
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./20260928-what-is-drawn-is-what-the-screen-covers.md);
  ARCHITECTURE.md "The view"). The client asks for terrain v7, so main
  waits for its publish. In order:
  - **Before main, held until the format settles.** Terrain v7 baked and
    published after `deno task usage` (about 227,000 Class A) — once, not
    per revision of the format, and not by demand: a page's index bounds its
    whole subtree, so a coarse page cannot be made before everything under
    it; the offline bake checked for the dev fold's race; a device build
    run; then merged with `--no-ff` and deployed. Until then the branch is
    checked locally and through the tunnel.
  - **Closing the model.** The cell set from the screen (2026-09-29) is to
    be seen on a device: on an iPhone 13 mini it is now about a third
    coarser than the reference, where a debug build measured 2026-09-27 (the
    ratio fixed at 2) drew 2.07 million triangles for most of the window and
    the GPU's time followed them and not the pixels — 6 ms at a quarter of a
    million, 14 at half, 26.6 at that — so 78% of frames ran past 16.6 ms,
    and a turn past 16 ms came in a frame the GPU ran long in 532 times of
    537. An `Observer` on the GPU casts every tile at once — held back while
    a rebuild casts a hundred-odd corner rays, where the records on the GPU
    and a readback a frame or two late cost more than they save; it earns
    its place when tiles shrink toward the cell. With the capacity contract
    held (2026-09-29, a tier fills as many slots as it takes), the readers
    of the mesh's surface (`Scheduler::surface_at`) are what is left of it:
    decor stands on its own chunk's tier, the same piece it is built into —
    what compensated for the retreat, now only for a cell averaged 2 × 2,
    inside a cell — and goes with decor as its own solver; places count a
    place only where the ground over it is drawn, which leaks the build's
    state into a count, and go with how places are drawn. What recurs no
    longer allocates (2026-09-29): the observation's, the domain's, the
    frontier's and a build's working lists are their owners' and reused;
    what `Known` read through a cache is a block of its page's index, made
    as the page arrives and gone with it; a chunk holds its layout's tiers
    and decor and one more; and a mounted chunk is one number in tables
    sized once — a moving view, and a chunk mounted, built, let go and
    another mounted in its place, allocate nothing once warm. What is sized
    once and never grows or goes back stays in the heap: moving it to
    `.fixed` is where it lies, not this. The arena's pools are a
    representation each (2026-09-29): the leaves' own cells, a level's
    averaged 2 × 2, and the cover, one cell a chunk, whose pool is as many
    slots as any source's top has roots and holds exactly that. What is left
    of sizing it is policy: the boxes a coarse slot holds (4551, the unit a
    tier is held in) and the share the quality gets (5 × 330) are chosen
    when the cell is seen on a device, from the tier sizes recorded at 4551,
    which give any other size's waste, draws and surface lanes offline; both
    stay until then. What reaches the GPU is relative to the eye
    (2026-09-29), so what a pixel shows there is the model's error and not
    the coordinates'. Objects keep their records a level and a 3D index of
    their own (2026-09-30). The fallback is all residency holds hard
    (2026-09-30): nothing leased and nothing depended on, since a page reads
    on its own and every reader of the ground takes the fallback's answer. A
    consumer that cannot — physics under a moving body, a placement, an edit
    — declares the pages it needs to each observation as the fallback does,
    and its place in the fetch order is argued then.
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
  meet is required of no level. Left: a cut ceiling flashes on a cold load
  while a coarser page stands in; and a
  column crossing a cube's floor or ceiling is two chunks — 1.5 a column
  near Gangnam and 1.7 on Bukhansan, 2026-09-26 — each scanning the whole
  column's runs again, where the runs could be the column's once and each
  chunk a cut of them.
- **A seam while a chunk is drawn from a page over it.** Since
  2026-09-26 a chunk whose page has not come is drawn in its place from the
  nearest resident page over it, upsampled (world/pyramid.rs `drawn_from`).
  Since 2026-10-01 a chunk's apron is read from what the page it lies in is
  drawn from (pxl-render mesh/paint/terrain/adapter.rs `Around`), and a
  neighbor page's source is among a chunk's build sources
  (world/chunk/table.rs `Sources`), so both sides of an edge are one surface
  whichever is borrowed. Left: seeing it hold after a jump to a coast, where
  brown walls stood about the character for a second.

## P1

- **A change of polar re-cuts the whole screen.** Seen 2026-09-29: moving
  the polar makes the whole view flicker as pieces are replaced. Measured
  2026-10-01 near Seoul Station at a distance of 600, the polar swept
  0.3 → 1.3 → 0.3 rad (the profiler's `quality` section): `τ_eff` and the
  feasible step held at the target, residency took in and gave up no page,
  no drawn piece changed cube, and the step to 1.3 made 38 refinements —
  300-odd builds, a refinement's children — with at most 8 changes to what
  is drawn between two of the frontier's steps. So it is not the quality,
  the pages, a burst of swaps or the view's order: the cut moves because a
  tilt changes the angle all the ground is seen at, which is the criterion
  working as stated (domain.rs). A coarsening now waits
  `param::COARSEN_HOLD_MS` before it is made (frontier.rs), which takes 22%
  of the builds out of a quick tilt back and forth and changes nothing on a
  slow one — build churn, not the flicker. What is left is how a refinement
  is shown: each dissolves over 300 ms through a pattern fixed to the
  screen (pxl-render dissolve.wgsl), under ground the tilt moves. Next:
  the sweep with `lod_crossfade` off; if the shimmer goes, a parent and its
  children swap whole and at once (`Fade::In`/`Out`), and only a piece's
  change of cube dissolves (`Fade::Tier`).
- **The move to React Native, the rest.** Built for iOS 2026-09-26
  (apps/native, crates/pxl/web/native); what is left of the move itself —
  what the device costs to run is the budgets' items, not this one:
  - **Android and Quest.** The Kotlin half of `modules/platform` — the
    surface on a Vulkan layer, the kernel's memory and doorbell, storage,
    text through Android's own engine, the compass — and the Android
    targets in `build-native.ts`; the kernel, the renderer and the shell
    take them as they are. Once the shell has settled.

- **One byte budget over everything drawn, paid for in LOD.** Its
  pressure is to go (P0, the view's cost derived from its rule): a resident
  byte derived from the view leaves no refusal to pay for. What stays is
  the device's own ceiling, the arena and wgpu's allocations. The phone's
  page dies because the working set exceeds a budget of about two
  gigabytes, and a WebView (Capacitor) runs in the same WebKit content
  process under the same limit — reported by others as a 2048 MB jetsam
  ceiling on iPhone, consistent with the mini's 1.6–2.1 GB. Today's
  `RETAIN_BUDGET_BYTES`
  ([20260912-what-to-render-when.md](./20260912-what-to-render-when.md))
  covers only what is held off screen; what is on screen has no ceiling.
  The precedent is CesiumJS: a tileset has `cacheBytes` and
  `maximumCacheOverflowBytes`, and when the tiles a view needs exceed both
  it raises the screen-space error until they fit — the view gets coarser,
  the page does not die
  ([Cesium3DTileset](https://cesium.com/learn/cesiumjs/ref-doc/Cesium3DTileset.html)).
  On a device the same ceiling is the app's: a debug build on the mini
  still took the system's critical memory warning once, seconds after
  start, with the kernel's region mapped rather than written (2026-09-27),
  and a device's heap row counts wgpu's own allocations beside the
  kernel's (crates/pxl/web/native/src/memory.rs).
  MapLibre sizes its tile cache from the viewport rather than a count
  ([MapOptions](https://maplibre.org/maplibre-gl-js/docs/API/type-aliases/MapOptions/)).
  The same lever exists here: the one LOD criterion's distance thresholds.
  First seen on the desktop, 2026-09-14, after a zoom through a city:
  on-screen work 158 MB against the 128 MB budget, the mesh section 181 of
  256 MB live and the rest lost to the buddy pool's rounding (a request
  takes the power of two above it), off screen nothing left to reclaim,
  1,091 chunks pending on tiers whose blocks the pool refused, for the rest
  of the session — the bake slot held by one of them, so nothing else baked
  either, and the tick spinning a hundred milliseconds a frame over the
  refusals. Built the same day: a refused block ends the tick's builds and
  commits, the refused chunk gives up its slot and is not tried again until
  the pool gives a block back (`PoolStat::releases`), and the budget is paid
  in LOD — the ladder's pressure (lod.rs): a step in when a block is refused
  and reclaim found nothing, at most every half second, up to three tiers; a
  step out after five seconds without a refusal with the section off its
  mark; the profiler's `lod ... pressure`. Still open: what the rounding costs
  the section (about a third at the sizes tiers come in) — size classes
  between the powers of two, or a large-block region — once the pressure
  has been seen doing its work.

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

- **Compression, chosen by traffic.** Recorded 2026-09-13, not scheduled:
  [20260913-compression-by-traffic.md](./20260913-compression-by-traffic.md).
  Measured again 2026-09-22 at 512 cells: zstd -22 is within 3% of Brotli
  q11 and decodes twice as fast, 46% under the bake's gzip -6. Safari takes
  zstd from 26.3, not on macOS before Tahoe; whether the host or the kernel
  decodes a stored page is open, since a React Native host would not decode
  what a browser does. Decided later that day: zstd -19 from the next bake,
  decoded by the transport, for what the bucket holds; what is published now
  stays gzip. Live
  responses want a fast level, realtime messages want an encoding and a
  stream more than a codec, and the codec stays in the transport, never in
  the command frame.

- **Decor: the impostor tier.** Decided 2026-09-13
  ([20260911-decor-identity-orientation-and-one-cell-format.md](./20260911-decor-identity-orientation-and-one-cell-format.md),
  "Closed"): each variant rendered once to a sprite, far instances drawn
  as a camera-facing quad through the billboard field. The criterion is
  in and every shipped kind is on the nearest tier only; the impostor is
  what would draw them further for two triangles each. Not a phone fix —
  build it when a wider decor range is wanted.

## P3

- **A staging file named by what it stages.** `pxl-geo-bake util.rs`
  `write_atomic` stages every write through `<name>.part`, so it is atomic
  only while one writer at a time writes a name. The dev tile server's
  cache and the offline bake keep that today — the shared ocean page is
  written once when a generation opens, since two folds re-publishing it
  raced on the rename and left pages a 500 for the generation (2026-09-29)
  — but the next writer outside the cache's `once` would race again. A
  staging name unique to its writer takes the assumption away.
- **A POI page read as a truncated packet.** Once, on one machine under
  dev:remote (page 6362873910329344), the fetched bytes whole in the
  console; into_core's view the suspect. Not reproduced, and POI is out of
  scope until it is back.
- **A terrain read that hung.** Seen once on the simulator, 2026-09-27:
  every terrain page's read went unanswered for a minute and more while
  assets, geocode pages and the site's root answered at once, and a minute
  later the same pages came back in under two seconds. Not reproduced. The
  JavaScript was being reloaded under the kernel at the time, and a reload
  then left every credit with reads no shell would answer — the kernel now
  lets them go when a shell attaches (door.rs `host_attached`); whether that
  was all of it is not known.

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
  - Not yet seen on a screen: the clouds at sunset, and the moonlit sky
    toward the moon itself.

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

- **The TypeScript's private marker, ported as a leading underscore.** 74
  places outside the tests: 53 functions and 21 fields, concentrated in
  `wire/xtp/cursor.rs` (25), `wire/tlv.rs` (23), `collection/arena.rs`
  (8) and `wire/xtp/encode.rs` (6), with a few each in
  `ipc/change.rs` and `wire/xtp/view.rs`,
  `pxl-spec`'s `sparse_tree/mem.rs` and its three `cell/` files. Rust has one
  meaning for a leading underscore and it is not privacy — privacy is the
  absence of `pub` — it silences the unused warning, which `rustc` confirms
  for items as well as bindings: `fn _never_called()` draws nothing where
  `fn never_called()` draws `never used`. So the marker turns the dead-code
  lint off for exactly the functions most likely to go dead. Two kinds:
  - Forced, where a private helper shares a name with the public method that
    calls it — `tlv.rs`'s `_u8`/`_i64`/`_f64`, `cursor.rs`'s
    `_bool`/`_f32_array`, about twenty in all. These need some
    disambiguator; `heap.rs`'s `build_of` is the suffix precedent.
  - Free, where nothing collides: `_grow`, `_need`, `_put`, `_take`,
    `_check_unsealed`, `_prune`, `_walk`, and every field — `_cap`, `_len`,
    `_size`, `_head`, `_buf`, `_off`, `_sealed`, `_factory`, `_mark`.
    `pxl-spec`'s `cell/type.rs` carries `pub _meta`, which says both at once
    and wants reading before it is renamed. `collection/heap.rs`'s `build_of` is the
    precedent for the forced kind, and the free kind just loses the underscore.

- **The TypeScript's numeric model, still named in the Rust.** `Number` and
  `BigInt` are the only numeric types TypeScript has, and nothing here was
  ever meant to correspond to them: the port is not allowed to carry the
  other language's constraints across. What is left is
  `crates/pxl/web/kernel/src/shell/setting/`'s `SettingKind::Number` and
  `SettingValue::Number` — code rather than prose, so read first whether
  the schema's value kinds are JSON's or JavaScript's. `ArrayBuffer` in
  `mem.spec.rs` stays, since the module's `memory.buffer` really is one on
  the host's side; `codec/json.rs`'s `JsonKind::Number` stays, JSON's own
  name.

- **The run set's per-axis inset has no writer.** `sub_sx`/`sub_sy` are
  always a copy of `sub_sz` (the scan, the coarse pack and the object
  adapter all set the three equal), so the per-axis design column_run.ts
  documents — a pillar narrow in XY and full in Z — costs two bytes a run
  for nothing today. Either the coarse bake learns to fit XY and Z
  separately, or the lanes go; decided with the Rust mesher, since the
  format is its input.
- **The const enum pass, once.** Asked 2026-09-27, not now. Vite
  (`apps/web/vite.config.ts` `const_enum`) and Metro
  (`apps/native/babel.config.js` `inline_enums`) each read every
  `export const enum` into a table of their own, a near copy, and already
  drift: one holds values as source text, the other as values. The reading
  goes to one file under `apps/web/script/`, plain JavaScript, since Metro
  `require`s the Babel config under Deno — whether CommonJS there can
  `require` an ES module is to be tried first. Vite's replacement is text:
  a member that prefixes another (`SENSOR_ENABLE` under a table read before
  it became `SENSOR_ENABLE_LOCATION`) left `7444_LOCATION` for the parser
  until the match was held to whole names. Replacing on the syntax tree, as
  Babel does, takes strings, comments and prefixes out of it as a class, for
  a parse per module on Vite's path.
- **Native tests lay out `.fixed` as a host does.** A native test lays out
  no `.fixed`, so `range` answers none there and what lives in it answers
  empty or no room under a test (the surface, a page's lanes, the decor
  lanes, the profiler's rings) — a second memory model for the tests. Laid
  out over a buffer of its own, the tests run on the ranges the module does.
  Only then is it worth asking whether what is sized once and kept in the
  heap for the tests' sake (world/chunk/table.rs `Scheduler::new`) moves
  there; it need not.

## Parked, with the reason on file

- **`ds/treap_set` and `kit/comp`'s Rust side, removed from the pair.**
  Their consumers and what would bring them back are in
  [20260919-the-pair-audited-what-left.md](./20260919-the-pair-audited-what-left.md).
- **Coarse tiers: draw calls, a wider unit.** Not a bottleneck after the
  direct path; not without a measurement —
  [20260912-coarse-tiers-what-the-field-does.md](./20260912-coarse-tiers-what-the-field-does.md).
- **Characters and the rest without a record format.** Not before the format.
