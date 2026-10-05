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
- **Subway bores.** Every tunnel flag is a bore now, so a subway's lines are
  drawn underground and stretch each chunk's bounds down; whether that cost
  stays is not decided.
- **Past the coverage, a coarse page is baked as sea.** A page over the
  coverage's edge folds the children outside it as ocean (`pxl-spec
  bake/fold.rs` `fold_ele`), which the client cannot tell from real water;
  folding them as VOID needs only the edge's pages baked again.
- **The bake's `_ocean` page.** The client no longer reads it (a missing
  page is drawn from the one over it), but the fold and the tile server
  still make it (`geo-bake terrain/leaf.rs` `bake_ocean`, `demand.rs`).
- **The view washed out from the globe.** Zoomed out over Korea near
  sunset, the frame settled very bright a few seconds after it was dark;
  the eye's exposure, not the sun, which stood where it should.
- **The ground dark under a low sun.** `ADAPTATION` 0.5 (`pxl-render
  look/atmosphere.rs`) opens the eye for half the light a sinking sun takes
  away, so at 3° the ground shows about two stops under noon while a clear
  day outside reads bright; a hyperparameter to settle.

## Parked, with the reason on file

- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./decision/20260930-wasi-and-the-boundary.md).
