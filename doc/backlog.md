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

- **The bucket's switch, after the next deploy.** That deploy is the first
  to serve `pixelet-bucket` and to read `_meta` with its own key
  (`4d2a58fcf`). If the Worker's upload is refused for R2, the new
  `pixelet-worker-ci` takes Workers R2 Storage: Read. Once it is out and the
  smoke passes: `pixelet-tiles` taken off `pixelet-bucket-publish`, and
  `pixelet-tiles` emptied and deleted.

## P2

- **Every beam ray walks from the root.** Counted (a47145e81, S1 turned
  and walked, `ray_counts`, 9.6M pixels): the eye's beam 797k nodes a cast,
  38 a corner over its 20.8k corners, `beam` 1.1-1.2 ms; the light's 610k
  over its 71k. Neighboring corners descend the same nodes from the root
  until they part, and a node first asked in a pass is asked of the world
  again by every walk of that pass that meets it (the eye's walks 42.8k such
  asks a cast, of 48k in its beam and samples). The fix: the beam's lattice
  walked a level at a time from the coarsest, each corner resumed off the
  least of the coarser lines about it as a cell's samples resume off its
  corners (observer.rs `cell`, `resume`), both observers alike; it adds a
  pass and lines a level, so it waits on what it would take off `beam`
  (at most its 1.2 ms) against the passes' cost.

- **The stencil and the early depth test on the devices.** Answered from
  the code and the vendors' records (the minutes' "Open"); left to a device
  in hand: the iPhone's hidden-surface removal for the boxes and the
  impostors, which discard and write depth, and what a given Android
  driver offers (`Device.depth32float_stencil8`), taken with the device
  builds.

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
