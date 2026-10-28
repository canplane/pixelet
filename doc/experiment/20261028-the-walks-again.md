# The walks again

**2026-10-28.** After the prune went (`7f9195c9c`; [the rows,
seen](./20261028-the-rows-seen.md), [the sun without its
prune](./20261028-the-sun-without-its-prune.md)), the pixels that walk again
at grazing angles grew, from 0.21% to 0.53% in E2's view. This record
counts why they walk (E6), tries one structural answer for the larger share
(E7), and drops it.

## Why

A pixel walks again where the raster leaves it no box to take: the walk
(pxl-shader cast.rs `miss_list_cs`) is exact, but it is the cast's dearest
lane. Removing the prune cost about a millisecond of GPU p95 at the horizon,
mostly there. The question was which reason grew, and whether it can be
removed without adding machinery.

## How

Exports only, with probes that change nothing drawn. They ran in one
Chrome window at dpr 2 (1200×1113 for E6, 1200×1169 for E7), shadows off,
and counted with ray_counts. E2's motion was used, with the walker at the
device's location (terraces by water).

- **E6, the reasons.** cast_cs counted each walked pixel with no pick by
  what the four samples about it offer its own ray:
  - (a) a hole: no box of theirs is entered;
  - (b) off-axis: a box is entered, but only across another axis than its
    sample entered by. The raster draws a box's face across its sample's
    axis alone, so `fs` discards it (raster.wgsl, pxl-shader raster.rs
    `picked`).

  A lack's pick and a refused pick were counted apart; both were nil.
  The exports were HEAD `7f9195c9c` and the tree before the removal
  (`52f0e9dba`).
- **E7, a face for every open side.** The axis refusal was replaced by the
  face's own openness:
  - `face_cs` ran before the draw. Where the sample face's word (`face::word`
    `LINE_SHIFT`) says the cell beside a side is not whole, that side face
    was drawn, taken, and given a word of its own, asked as the sample
    face's is.
  - The same probe also counted the face pass's asks and the raster's
    fragments (an atomic in `fs`).
  - It ran on HEAD `bc4f30997` against that variant. Each pair was checked as
    one frame by its screenshot and its boxes (within 1%).
  - A flatter spot could not be pinned, so a lower, turned grazing view
    stood in.

## What was seen

**E6, walked pixels a cast, horizon:**

| | walked | (a) holes | (b) off-axis |
|---|---|---|---|
| before the removal | 0.34% | 7.5 k | 10.7 k |
| HEAD | 0.75% | 18.6 k | 21.7 k |

- Both reasons roughly doubled.
- At a flatter spot HEAD walked 2.2%, and (b) was 85% of it.
- Looking down: 237 walked a cast, all holes.
- The prune had passed thin, grazing cells; samples now stop on them, and
  the pixels between see more of their sides and more rows between samples.

**E7, HEAD against a face for every open side:**

| view | walked | (b) | asks a box | fragments |
|---|---|---|---|---|
| terraces, horizon | 0.83% → 0.65% | 25.3 k → 15.7 k | 9.9 → 14.5 | +32% |
| turned, grazing | 0.99% → 0.77% | 31.3 k → 19.5 k | 9.7 → 14.6 | +2% |
| down | 0 → 0 | 203 → 0 | 10.2 → 15.7 | +43% |

- (a) and the boxes a cast did not move.
- Only open sides are asked, so a box took about 15 asks, not the 36 of
  three full words.
- The fragments include the boxes drawn over the whole screen near the eye
  (`raster::EYED`). The added ones are the side faces: thin at grazing,
  wide on terrace steps looking down.

**Reviewed before the build** (pixelet-d3, pixelet-1d [faaf39]):

- A LINE bit is a sound openness test at the stop's zoom, if a lenient one.
  A side face can still show past an undrawn finer or partial neighbour, as
  a wrong surface rather than a hole.
- A side face taken needs its own word, or it is painted with the sample
  face's edges. The other way, `face::BOX` on open sides, keeps today's asks
  but outlines every open side on all four edges.

## What it says

The walks again are not worth removing this way.

- They are under one pixel in a hundred.
- Half of them are holes, rows thinner than the samples' spacing, which no
  face drawn can answer. The walk is the answer there.
- Of the off-axis half, the open sides removed about 40%. The rest are
  sides against a whole neighbour, where walking is right.
- So walks fell by about a fifth: a share of the millisecond the removal
  cost. It was bought with half again as many face asks, and with more
  fragments in every view, including looking down, where nothing walks.

Not built, and not timed. The walk stays as the exact answer for what the
raster cannot hand out.
