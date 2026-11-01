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
- **The light cast again on every walking frame.** Measured (2026-11-02,
  `light_moved` over 10 s walks): the detail and receivers bits on every
  walking frame, content on 54 of 113 at noon with 9 bricks landed; a
  detail-and-receivers frame casts as many rays as a new epoch. Confirmed by
  a probe (the margin test in `holds` made never true): samples 81,917 to
  2,653 and walks 30,457 to 294 a frame, 37 of 424 such frames casting at
  all. The cause: `cast::Margin::asked` takes `Detail::margin` of every node
  a walk asks, the distance to the nearest band edge whether or not crossing
  it changes that node's answer, so a tile's least sits under one frame's
  step. The fix: a node counts toward the margin only through the band edges
  that change its own answer (where its zoom is crossed, or a zoom its
  content lies at); one whose answer is the same at every zoom asked counts
  none, which needs the query to say which asked zooms its answer read. And
  the content bit's trips at noon past the bricks, supposed the clip levels
  sliding (`world_moved`), not traced — [the tiles' baseline](./experiment/20261030-the-tiles-baseline.md),
  "The closing round"; [20261029](./minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md),
  "Taken in the closing round", the recast not taken.

## P2

- **What the occlusion's screen mode costs.** Busy with AO `screen` over
  AO `corner` some 2 to 4 ms on both builds of the closing round; the
  regression against the prepass was the `ao` span's, which reads the world
  pass's first half with its own — [the tiles' baseline](./experiment/20261030-the-tiles-baseline.md),
  "The closing round".
- **Samples closer only where their faces part.** Counted (ddd72660c, S1
  with `ray_counts`, 9.15M pixels drawn, about 279k samples, 33 pixels a
  sample): the eye's walks 0.55 % of the pixels at 26 nodes each, `miss`
  6.6 ms at the p95, some 128 ns a walk; `sample` 4.7 ms, some 17 ns a
  sample at 10 nodes; boxes 14.2k a cast, a box every 20 samples. A refine
  where neighbouring samples stopped on different nodes flags some 45 % of
  the squares between four samples: three samples more each, +6 ms on
  `sample` for at most the walks' 6.6, so it does not pay, and a camera
  cell's refine less. What could pay: a square refined only where its
  corners' faces lie in different planes (a step, a silhouette, one met
  nothing), coplanar node boundaries left; at 12 % of the squares, +1.7 ms
  on `sample` with the boxes and their raster for 40 to 60 % of the walks.
  First a tally lane counting such squares; build only under 15 %.
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
