# What the clouds cost

**2026-10-26.** Above 1.5 km the cloud sheets are a fullscreen pass with depth
always, a march along every pixel's ray to both sheets before it can discard.
Drawing them as an eye-centered disc, depth-tested, would cover only the pixels
a sheet can be on. Before building that, this weighs what the pass costs.

## How

Branch `experiment/20261026-cloud-sheet`, on the hook of 76b223a65 (not on
main), the time line of
[the air waits](./20261026-the-air-waits.md): the world pass is measured from
the cast's last pass's end to its own, since its stamped begin falls inside
the cast. Fixed path, dpr 2, 2056 × 1169, `resolution` fixed, shadows off,
AO corner. The `clouds` setting on and off, two loads each. Legs as the
path's script runs them; the first load's zoom out fell in a 3 s stall and
has no timed frames.

## Numbers

`world` from the cast's end, p50 / p95, ms:

| Leg      | Clouds on, load 1, load 2  | Clouds off, load 1, load 2 |
| -------- | -------------------------- | ------------------------ |
| zoom out | —, 6.5 / 12.0              | 3.2 / 6.8, 3.2 / 9.7     |
| hold     | 6.6 / 10.4, 6.6 / 11.6     | 3.6 / 4.6, 3.2 / 3.2     |
| zoom in  | 4.5 / 7.7, 4.5 / 8.4       | 4.8 / 4.9, 4.4 / 6.6     |
| back     | 4.5 / 4.6, 4.5 / 4.7       | 4.6 / 4.9, 4.9 / 5.3     |
| turn     | 4.5 / 4.7, 4.5 / 4.6       | 4.6 / 4.9, 5.3 / 5.7     |

Interval p95: on 86 / 83 ms, off 82 / 88 ms.

## What it showed

- **The clouds cost 3 ms at p50 and 6–8 at p95, and only high up**: in the
  zoom out and the hold at about 4096, where the pass runs over every pixel.
  Near the ground (back, turn) they cost nothing measurable.
- **The interval does not move**: 82–88 ms at p95 either way. The frame is
  bound by the cast's passes, and the cloud pass is a few milliseconds of a
  world pass that is itself 5–9.

So clouds as geometry would save at most the high legs' 3–8 ms of GPU time
and none of the felt interval while the cast costs what it does.
