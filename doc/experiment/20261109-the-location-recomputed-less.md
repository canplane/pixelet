# The location indicator recomputed less, on the S23

**2026-11-09.** What `ef2298f898` buys on the S23.

## Tried

The clipmap reported a change for each coarse tile it took while the camera
moved, and that report covered the location indicator's zone, so its memo was
thrown away. It now reports by the brick the change touches.

## How it was measured

Release builds, `script/perf/android-chrome.py` in the S23's Chrome, the camera
moving over 166 frames in the first. One run each. The base is `b3b243d67b`. AP
39.2 to 44.1 °C on the base and 42.2 to 45.4 °C on the change. The change's
build also held the renderer's `3ef95cc29c`, so its GPU numbers are not this
change's.

## What came of it

| | base | change |
| --- | --- | --- |
| effects p50, ms | 9.2 | 4.5 |
| effects p95, ms | 10.9 | 10.8 |
| `location` p95, ms | 10.8 | 10.7 |
| core turn p50, ms | 19.8 | 13.7 |
| `decor` p95, ms | 0.1 | |
| GPU p50, ms (renderer's commit between) | 100.8 | 50.7 |
| frames a second (renderer's commit between) | 8.3 | 14.6 |

## How we know

Half the frames no longer recompute: the p50 of effects halved. A recompute
costs the same: the p95 did not move. One run each, the temperatures differing
by 1 to 3 °C.

## Kept

Kept. A recompute's ~10 ms is unchanged and queued.

## Then: what recomputes it, and what a recompute costs

At `bda0cb51d8`, after the ways' move landed. Temporary counters beside the profiler's
effects, in a tree of its own, and the same drive in the S23's Chrome, release. One run
each, 290 and 294 moving frames, AP 35.7 to 46.5 °C and 35.3 to 45.5 °C, both with the
counters. The change: an answer every read gave exactly at the leaf cells is dropped only
by a ground node that holds those cells, and by what objects wrote.

| | base | change |
| --- | --- | --- |
| frames recomputing, of the frames | 76% | 34% |
| effects p50, ms | 4.0 | 0.1 |
| effects p95, ms | 11.1 | 10.3 |
| `location` p95, ms | 11.0 | 10.2 |
| the interval's turns p50, ms | 13.2 | 13.6 |
| recomputes by the coarsest node that touched: past zoom 15 / zooms 12 to 15 / under zoom 12, counted to the run's 95th percentile frame | 18 / 65 / 126 | 11 / 37 / 45 |

A recompute asks 58 heights. Its time, in ms, at p95 of the frames (base, then change):
`SplitCell::new` 0.3 and 0.2, the surface 0.6 and 0.5, `lift` 5.4 and 4.6, `below` 5.2 and
5.0. The cost is the two rays of each height.

The recomputes left are those of an answer not exact at the leaf, where a coarser node
does move it, and those of the leaf's own tile and brick. The turns' p50 did not fall in
this one run, and a recompute costs what it did. Kept.
