# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in the
discussion the line points to. A finished item is removed, not ticked — its
record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so it
is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **Faster than the deployed build, on every axis**
  ([the deployed build is the bar](./minutes/20261107-the-deployed-build-is-the-bar.md)). The
  Mac's gate passes, level on GPU time. The S23 at 550 MHz: GPU p50 3.6× the deployed build's,
  p95 1.2×; the bar is 2×. The p95 is the light's. On the Mac's drive, 43% of the light's time is
  the box's depth stepping as the eye moves along the light, which casts every tile read
  ([the journal](./experiment/20261105-the-frame-on-the-s23/journal.md), 11-08).
- **A split's air children skipped (ESVO's child masks).** A split's answer carries its
  eight children's occupancy, kept with it in the node cache; a walk crosses an air child
  without asking or looking it up, the engine's walk by the same rule. Then sibling
  loads (a split's crossed children asked together) and, where the S23 and Safari offer
  them, subgroups.

## P1

- **The light by a list and the raster.** Each tile's voxels at its band listed through the
  query and drawn from the sun, in place of the light's rays and its holes' walk: 11-06's open
  question ([the frame is what it must draw](./minutes/20261106-the-frame-is-what-it-must-draw.md),
  [cell raster](./minutes/20261106-cell-raster.md)). The user holds it until the rest of the
  backlog is done, and it waits on the CPU fill's renderer work. The counts put a list near
  100 k cells (the journal, 11-08). First `list::light` on the CPU, held to `cast::first`,
  which measures the real list.
- **The stops' room is 2^18, the deployed build's 2^19.** In a frame of 2^18 to 2^19 holes a
  whole cell's pixels past the room lack their face word. 2^19 overflows the 128 MB binding.
- **A page closed in the turn before a suspend** may lose an edit: the system's suspend gives
  the save no window of its own.
- **The middle distance's look**: where the ground goes flat, and the far zoom's floating
  boxes, the user's to decide (10-28's floating box, 11-02's one boundary for the effects).

## P2

- **The vocabulary, cleaned.** Agents have left words with several meanings
  (`level`, `chunk`, `frame`), several words for one thing, and names of what
  happened. In order: every domain noun listed with what it names (read-only, by
  area), a glossary decided with the user, conventional names first and cited, put
  in the naming skill; then the renames by area, one names commit each.
- **Lighting run under decor.** A depth-only screen draw with a coverage bit would
  skip it, but the stencil's 8 bits are the gate's.
- **The character stands half inside a step at load.** At 38.1193, 128.4656 the point lies
  0.11 of a unit from the wall of the higher cell east of it, which the view from the south
  sees edge-on at the screen's middle, and a person a unit wide stands through it. A character
  stands at a point, with no width. Whether it keeps clear of a wall is a look decision.
- **How strong a shadow is at dawn on flat ground.** A full shadow takes about 21%
  off at 5° of sun (`SHADOW_BY_UP_TAKEN`, `LIGHT_BY_NORMAL`), and terrain already
  shades most near ground then. A look decision.
- **A login screen is to come.** Another kind of `Cover`, over `Starting` or
  `Loading`, so the world is made while it stands —
  [20261103-what-stands-over-the-app.md](./minutes/20261103-what-stands-over-the-app.md).

## Parked, with the reason on file

- **A query over what is drawn, for an agent to read.** An agent debugging the
  view guesses a pixel's cause from its color in a screenshot. The profiler's
  half is answered: the console's `pxl.profiler.report()` gives its window as
  text (`ts/client/src/shell/debug/console.ts`). What a screen point draws is
  not: the block its ray met, with its face, zoom and distance, and where that
  block was read from, asked of the running page and answered as text. The
  answer is the GPU's, the ray's words read back (`pxl-engine ray.rs`), since
  the CPU's `first_hit` (`pxl-kernel domain/query/ray.rs`) can disagree where
  the samples miss a thin face
  ([20261024-a-proxy-only-starts-the-ray.md](./minutes/20261024-a-proxy-only-starts-the-ray.md)).
  Not urgent: no debugging under way needs it.
- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against; guests
  stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./minutes/20260930-wasi-and-the-boundary.md).
