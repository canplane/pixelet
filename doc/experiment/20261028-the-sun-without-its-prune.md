# The sun without its prune

**2026-10-28.** E5 of the grazing rows, after the eye's prune was found to
be their cause ([the rows, seen](./20261028-the-rows-seen.md)) and the
mitigations that kept it were dropped
([the cone touches](./20261028-the-cone-touches.md),
[the ring](./20261028-the-ring.md)).

## Why

The sun's map passed what its texel could not tell apart by the same
machinery as the eye's prune (pxl-spec-core cast.rs `walk`, `meets_box`'s
inner shrink and its `facing`). With the eye's prune gone, the sun's was
the only reason left for that machinery. The question was whether the sun
needs it: how the shadows look without it, and what it costs.

## How

Two exports of `d8f712deb`, side by side in one Chrome tab at dpr 2, a
2056×1113 CSS viewport and a fixed render scale of 1.00:

- **:5176**, HEAD;
- **:5178**, the prune zero for every walk, the eye's and the sun's.

The look was taken at the default view, the start fresh on each, with
shadows at 7 a.m. and 3 p.m. Each was zoomed on the terraces and trees,
where the shadows fall long.

The profile used the horizon view and E2's motion with shadows on: one
warm-up run, then two timed runs, then one with ray_counts.

## What was seen

**The look.** At 7 a.m. the two cannot be told apart, the terraces' and the
trees' shadows alike. The shadow stripes of the 10-27 11:54 screenshot were
not in this view, so whether they change is not seen here.

**Horizon, shadows on** (p95 ms unless marked):

| | :5176 HEAD | :5178 no prune |
|---|---|---|
| interval p50/p95 | 17/26, 17/26 | 17/27, 17/26 |
| missed frames | 13–16% | 29% |
| gpu p50/p95 | 8.1/21.6, 8.6/21.6 | 9.2/23.5, 9.0/29.2 |
| sun_beam | 2.5 | 2.4–2.5 |
| sun_sample | 1.2–1.3 | 1.2 |
| sun_raster / sun_pick | 0.3 / 0.3 | 0.2–0.3 / 0.3 |
| sun_walk | 0.7–0.8 | 0.7 |

**The sun's lanes** (ray_counts, nodes a cast):

| lane | :5176 HEAD | :5178 no prune |
|---|---|---|
| beam | 328 839 | 311 480 |
| sample | 260 944 | 252 723 |
| miss | 377 754 | 365 628 |
| picked / walked | 6.02% / 0.62% | 5.81% / 0.60% |
| boxes | 10 679 | 9 712 |

The eye's lanes on :5178 are E2's with the eye's prune off: miss 2.04 M
nodes against HEAD's 643 k, walked 0.50% against 0.18%. That is the
frame's rise in missed frames, not the sun's.

**Not run:** the down view, and :5177 (the eye's prune off alone) beside
these.

- The sun's lanes and spans had not moved at the horizon, where its prune
  had the most to pass.
- The browser extension then dropped.
- The user took the horizon's result as enough.

## What it says

The sun's prune buys nothing that can be measured or seen. Half a texel is
small against the nodes its walks cross, so it passes almost nothing. Its
visits are within the spread of one view to the next, and slightly fewer
without it.

So both prunes go, and the machinery with them (`parts`, `along`,
`facing`, `meets_box`'s inner shrink). The cone is left as the beam's
spacing alone.
