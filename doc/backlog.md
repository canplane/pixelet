# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in the
discussion the line points to. A finished item is removed, not ticked — its
record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so it
is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

Nothing now.

## P1

- **The occlusion's screen mode dearer since the round.** A moving frame
  some 5 ms dearer with AO `screen` (its span 4.4 ms to 9.9 at the p95),
  the gated frames 4 ms cheaper; supposed, the world pass split about it on
  a tiled GPU. Taken back, or found again without splitting the pass —
  [the tiles' baseline](./experiment/20261030-the-tiles-baseline.md), "The
  occlusion's screen mode after the round".
- **The eye's samples and what they miss.** Orbiting, 1.6 % of the pixels
  find no sample's cell and walk alone, some 16 nodes each, some 5 ms
  (`miss`), the eye's cast some 13 ms of a moving frame's 24; samples
  closer where they miss cost what they save. Both observers share the
  passes (pxl-shader observer.rs), so a change holds for the light alike —
  [20261029-the-screen-keeps-its-tiles-as-the-light-does.md](./minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md),
  "Proposed after the steps".
- **The eye's beams and samples.** The cast's passes before the misses:
  in the scenario, `beam` p95 some 5 ms and `sample` 4 to 5 ms, together a
  quarter of a moving frame's busy time (13 % and 12 %). What each walks
  again that a neighbor or the frame before already walked, and what the
  observed pyramid keeps of it; the passes are both observers', so a change
  holds for the light alike — [the tiles' baseline](./experiment/20261030-the-tiles-baseline.md),
  "Decor put back, and the round after the steps".
- **The screen reprojected while the eye moves.** The light holds its tiles
  across the eye's motion, fixed in the world; the screen, fixed to the
  view, draws whole each frame. Last frame's rays moved to the new view,
  what they uncover cast anew, an approximation the picture must be held
  to — same minutes.
- **The light's marks only where the eye cast anew.** Orbiting, the marks
  are some 4 ms (`light_beam` less the beam and the cells), every drawn
  pixel's each moving frame; marked only where the screen's tiles changed,
  they follow the eye's own change — same minutes.

## P2

Measured, not yet: each against the tip before it, the scenario of
[the tiles' baseline](./experiment/20261030-the-tiles-baseline.md) ("Decor
put back, and the round after the steps"), A and B by turns.

- **How much the light holds with exact margins**, the slots cast a moving
  frame, and what the light passes' list read as atomics costs them
  (`light_beam`, `light_sample`, `light_miss`).
- **The light map's gate over every tile in a gated frame** (65,536
  instances where the casters' rectangles were) and the marks' atomics
  (`tile_mark.wgsl`), step 6 part 2's cost.
- **Decor generated again where a brick lands**: `generate` p95 3.5 ms on
  frames a brick arrived in against 1.6 on the rest; against `1db9b6b8d` at
  the same place (a trace of the tip alone,
  `~/Downloads/profiler-2026-10-31T00-50-20-348Z.json`).
- **The conditions the tiles' plan left unmeasured**: 7:00, night, the
  clouds moving, render scales other than 1, the quiet view; and step 3
  part 4's 1.2 ms, unexplained —
  [20261029](./minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md),
  "Open".
- **The air's volume found again while the camera turns or the light
  eases**, held only where its reads stand: a volume laid about the light
  as the sky view is would keep it across turns; far out, a pan north or
  south trips the altitude's share; the sky view's second march, for its
  slope, doubles each redraw.
- **The stencil and the raster's gate on the devices.** Whether the iPhone,
  the Quest and Android offer `depth32float-stencil8`, and whether the
  raster's gate of depth rejects the boxes' fragments early there; taken
  with the device builds — same minutes, "Open".

## Parked, with the reason on file

- **What each reuse buys over a pure ray a pixel.** Each switched off alone at
  the tip, against the tip and against all of them off; why rays at all is on
  file, and the 9/26 build is no raster to set them against. Parked by the
  user (2026-10-28), reframed with them (2026-10-31) —
  [20261031-why-rays-and-what-makes-them-fast.md](./minutes/20261031-why-rays-and-what-makes-them-fast.md),
  first asked by commits in
  [20261026-what-made-it-smooth.md](./experiment/20261026-what-made-it-smooth.md).

- **A query over what is drawn, for an agent to read.** An agent debugging the
  view guesses a pixel's cause from its color in a screenshot. The profiler's
  half is answered: the console's `pxl.profiler.report()` gives its window as
  text (`ts/client/src/shell/debug/console.ts`). What a screen point draws is
  not: the block its ray met, with its face, zoom and distance, and where that
  block was read from, asked of the running page and answered as text. The
  answer is the GPU's, the ray's words read back (`pxl-spec-core ray.rs`), since
  the CPU's `first_hit` (`pxl-kernel domain/query/ray.rs`) can disagree where
  the samples miss a thin face
  ([20261024-a-proxy-only-starts-the-ray.md](./minutes/20261024-a-proxy-only-starts-the-ray.md)).
  Answering it might be where a language for agents to query the view starts.
  Not urgent: no debugging under way needs it.
- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against; guests
  stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./minutes/20260930-wasi-and-the-boundary.md).
