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
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./decision/20260928-what-is-drawn-is-what-the-screen-covers.md),
  "the steps, measured").

## P1

- **The device's image binding, built once.** `a59f64e4` changed
  `pxl_image_hold` to take the kernel's handle, and with it the Swift and
  Kotlin bindings (`PlatformModule`, `PxlCore`); the Rust side is checked for
  both targets, the bindings only by a device build, at the next one.

## P2

- **A bore's approach that stops short of a bridge.** At Uidong (37.2977,
  127.0580) a Grade line leaving a tunnel ends 4 cells under the ground where
  a deck starts at +5, a 9-cell step: the stretch's `across` lookup
  (`geo-bake deck/approach.rs` `branches`) seems not to find the deck's end,
  so neither reaches the ground nor meets the other.
- **A bore's swept air against its cover.** The paint clears from the lowest
  to the highest surface a column sweeps (`pxl-spec bake/deck.rs`
  `occupies`); the solve's ceiling (`geo-bake deck.rs` `clear_m`) does not
  know the sweep, so on a steep step the air eats into the 3 m cover.
- **Clearance by the road's own grade.** `clear_cells` holds a clearance
  across the road at `MAX_GRADE`'s secant for every way; the segment's own
  grade would give each what it needs.
- **A resumed terrain bake's flat count.** `leaf.json`'s
  `flat_elevation_pages` counts the last run's pages only; terrain
  v7.2026-10-04.1 says 7 for 5,984 of its 167,110 leaves.
- **The XTP POI and geocode pages, read back.** POI v9.2026-10-04.1 and geocode
  v5.2026-10-04.1 were baked on the new wire and never fetched and decoded
  through the dev server.
- **Subway bores.** Every tunnel flag is a bore now, so a subway's lines are
  drawn underground and stretch each chunk's bounds down; whether that cost
  stays is not decided.

## P3

- **What the sky left open.** Recorded 2026-09-22
  ([the sky is the air](./decision/20260922-the-sky-is-the-air.md)), each small and
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

## Parked, with the reason on file

- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./decision/20260930-wasi-and-the-boundary.md).
