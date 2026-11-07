# A light tile asks at its band

**2026-11-02** — recorded with the user. Main was at `e85b26b349`; the commit
that carries this record lands it. The commits are named as `original` keeps them.

## What was asked

At 07:00 the GPU took more a frame than at noon, a regression since
`fce4b5d43d`. The user asked for its cause by static analysis first, and for a
fix that reduces state, keeps the eye and the light symmetric and leaves the
code clean.

## The cause

The light map is a cache kept across frames. The light asked the world at the
eye's zooms, so a tile's walk was tied to where the eye stood, and whether the
eye's move kept a tile was computed node by node: each node's margin from the
eye (`cast::Margin`), first to the nearest zoom boundary (81,917 samples a
frame, 2,653 with the margin's check off), then, in `fce4b5d43d`, to
the nearest boundary that changes the answer, which asked each node wide and
again (`wide`, `probe`), with an epoch and run lists beside. The double asks
are the 07:00 cost.

## What the user decided

- **No revert; fix it with the history read** ("되돌릴 필요가 있나? 과거를
  보면서 잡으면 되지 않나?").
- **A tile's zoom is its band**: the camera grid's cell zoom at the tile
  column's nearest point to the eye, its place in the box alone. A slide keeps
  a tile exactly where it stays in the box and its band. Where the sun is low
  or the eye high, the light asks two or three zooms finer than the eye draws
  there. The design's four questions taken as recommended: the column's
  distance, not the receiver's; no floor at the texel; a world change casts the
  whole map, as before (replaced by the user on 2026-11-05: a world change casts
  again only the tiles it reaches, the changed bricks' and edits' columns tested
  against each tile's rays at its band, `Light::reaches`; the whole map only
  where the node cache forgets all or more columns changed than a cast is
  handed; memory not to grow much, quality not to drop noticeably); the run
  lists and the two run modes kept.
- **One reuse rule**: a walk takes another's only where that one asked the same
  `Detail` (`cast::Detail::alike`), for samples and beam lines alike ("고치라.").
- **The review's findings fixed before it lands** ("하라.").

## The premise that broke

The first design let a beam ray walk at the finest band of the tiles it feeds
and the coarser cells resume from it, on the premise that a node empty at a
finer zoom is empty at a coarser one. A test (`air_finer_is_air_coarser`)
found about 450 of 79,000 nodes where it is not: a coarse cell folds what is
under it, so a node empty at z23 is a ground cell an eighth full at z22. The
premise and its walk were dropped for the one reuse rule; a cell whose window
asked other bands walks from the root (`beam::CELL_RESUMES`).

## Rejected

- Reverting `fce4b5d43d` and what followed: the margins come back with it.
- A margin a node, any form: the walk stays tied to the eye.
- A receiver's distance for the band: turning the camera casts again.
- Reuse across bands by monotone air: false (above).

## Measured

`script/compare-perf.py`, base `e85b26b349` against the change, three
rounds, a fresh page a run, medians: the first load's longest frame 819 to
539 ms; noon's GPU p50 10.94 to 9.90 ms, p95 16.65 to 16.25; 07:00's p50 15.34
to 9.11, p95 20.84 to 16.78. The WGSL 5.63 MB to 4.61 MB.
