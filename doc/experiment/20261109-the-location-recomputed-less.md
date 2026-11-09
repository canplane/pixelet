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
