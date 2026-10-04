# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

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
  against faces alone.

## P1

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
