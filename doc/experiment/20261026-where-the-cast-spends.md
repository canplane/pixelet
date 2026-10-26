# Where the cast spends

**2026-10-26.** The cast as built is p95 ~80 ms on the fixed path
([the cast on a fixed path](./20261026-the-cast-on-a-fixed-path/README.md)),
and what to hand a raster was weighed without knowing which of its parts that
is. This splits it.

## How

The cast's one compute pass made a pass a dispatch, each in its own GPU span
(branch `experiment/20261026-cast-split`, 9176327e6): `cast` keeps the
forgetting, then `beam` (a ray a camera-grid corner), `cells` (a start a
cell), `sample` (rays a quarter cell apart), `pixel` (`cast_cs`: each pixel
takes the nearest of the blocks the 4 × 4 samples about it stopped at that
its ray enters, else walks from its cell's start) and `miss` (the pixels the
reading walk could not finish, walked again writing). Fixed path, dpr 2, the
window 2056 × 1169 (about 9.6 Mpx, against 9.2 Mpx in the baseline),
`resolution` fixed, three loads.

## Numbers

| Span     | p95, three loads    |
| -------- | ------------------- |
| `cast`   | 0.7 / 2.0 / 0.6 ms  |
| `beam`   | 16.4 / 17.4 / 17.3  |
| `cells`  | 0.7 / 0.7 / 0.6     |
| `sample` | 18.0 / 22.5 / 18.6  |
| `pixel`  | 32.1 / 38.8 / 32.6  |
| `miss`   | 21.2 / 23.7 / 21.6  |

Each pass is timed alone, so their p95s do not add to the one pass's.

## What it showed

- **The largest part is handing the samples' blocks to the pixels**, not
  finding them. `pixel` tests each pixel's ray against sixteen samples' blocks
  and keeps the nearest it enters: a choice among candidates by depth, which
  is what a rasterizer does in hardware.
- **The beams and samples together cost about 35 ms**, and they are also what
  writes the observed pyramid a cell at a time, which the proxy could not do
  without ([a ray from the box](./20261026-a-ray-from-the-box.md)).
- **`miss` is a fifth of it**: pixels whose own walk met a node the pyramid
  did not hold.

So the next split to try keeps the rays where the work is shared, beams and
samples finding the blocks and writing the pyramid, and gives the raster the
choice `pixel` makes: the samples' blocks drawn as boxes, the depth test
keeping each pixel's nearest.
