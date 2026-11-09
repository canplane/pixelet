# A pixel's check of its voxel, on the S23

**2026-11-09.** What three removals from the cast buy on the hot S23. Built in
`3ef95cc29c`, measured against its parent `b3b243d67b`.

## Tried

- `cast_px` took the voxel the eye drew for it, without asking again whether
  the voxel is fresh.
- The walks found a ray's aim once, where they had found it again after every
  `QUEUE_STEPS`.
- `fill` left a slot's last draw to the stop that first sets it to draw.

## How it was measured

Release builds with `PXL_PERF_DRIVE=1` of the two commits, each built apart,
`perf/drive.ts` in the S23's Chrome over the delivered tiles. The deployed
build has no such flag and was not measured.

`gate.py` over three rounds failed the change, GPU p50 82.8 to 109.5 ms, and
that is not the change: the base ran first in each pair while the phone went
from 32.6 to 41.1 °C. In its third round, 105.5 and 109.5 ms.

Then an ABBA order on the phone already hot, one run thrown away first, six
runs a build, AP 46.6 to 48.9 °C.

## What came of it

| median of six runs | base | change |
| --- | --- | --- |
| GPU p50, ms | 71.9 | 69.8 |
| GPU p95, ms | 178.5 | 201.8 |
| frames in the window | 223 | 227 |

The runs spread 65 to 72 ms at p50 on each side, and 165 to 229 ms at p95;
three throttled runs, over 100 ms at p50, fell on both sides. Nothing here
tells the two builds apart. The `px` span's p95, where the check ran, was 4.2
ms before and 4.4 ms after. The spans over it are `cast` 11, `beam` 10, `hole`
10, `sample` 8 and `world` 7.

Each removal is what the structure no longer needs, and none of them is a
gain on this device.
