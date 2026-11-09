# A run walks on what it parked, on the S23

**2026-11-09.** What `7c9002f32b` buys on the hot S23, measured against its
parent `ef2298f898`.

## Tried

- The beam rays' and the samples' runs listed the items they parked, as the
  holes' run does, and their second and last walks took only those, where each
  had taken every item and looked at whether it was parked.
- A lane past the end of a walk's items read the counter and did not write it.
- A side's word was settled by the stop that first asks it, where `fill` and
  `forget` cleared all six.

## How it was measured

Release builds with `PXL_PERF_DRIVE=1` of the two commits, each built apart,
`perf/drive.ts` in the S23's Chrome over the delivered tiles. An ABBA order, one
run thrown away first, six runs a build, AP 44.5 to 46.9 °C at each run's start.

## What came of it

| median of six runs | base | change |
| --- | --- | --- |
| GPU p50, ms | 109.8 | 95.9 |
| GPU p95, ms | 200.6 | 195.8 |
| frames in the window | 161.5 | 172.5 |
| `sample_hit` p95, ms | 11.1 | 9.6 |
| `light_sample` p95, ms | 28.1 | 26.5 |
| `light_beam` p95, ms | 18.1 | 17.2 |
| `light_hit` p95, ms | 42.6 | 41.5 |
| `light_walk` p95, ms | 13.1 | 14.1 |

Nothing here tells the two builds apart. GPU p50 rose from 51 ms to about 127 ms
over the session on both sides, at the same AP, and the last five runs lay
between 125 and 130 ms whichever build ran. A difference of 14 ms in the medians
is less than that drift, and the spans moved by a millisecond or two in either
direction.

Kept: the list is what the holes' run already did, and the code is the same
shape for all three kinds of run.
