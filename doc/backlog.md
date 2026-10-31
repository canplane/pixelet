# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in the
discussion the line points to. A finished item is removed, not ticked — its
record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so it
is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **The screen keeps its tiles as the light does.** After the other
  session's line (the light rename, ages by square, the kept finish) lands,
  the steps in order, 1 to 5 whatever the picture without them, 6 only where
  the numbers ask; steps 3 and 5 are for the symmetry the user asked (the
  user, 2026-10-30) —
  [20261029-the-screen-keeps-its-tiles-as-the-light-does.md](./minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md).

## P1

- **The eye's samples and what they miss.** Orbiting, 1.6 % of the pixels
  find no sample's cell and walk alone, some 16 nodes each, some 5 ms
  (`miss`), the eye's cast some 13 ms of a moving frame's 24; samples
  closer where they miss cost what they save. Both observers share the
  passes (pxl-shader observer.rs), so a change holds for the light alike —
  [20261029-the-screen-keeps-its-tiles-as-the-light-does.md](./minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md),
  "Proposed after the steps".
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
