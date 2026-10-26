# The air waits

**2026-10-26.** On the fixed path the spans `air` and `world` read 9–82 ms at
p95 and swing between loads. Whether that is their own work or time their
timestamps take from work around them decides whether either is worth moving
to the raster.

## How

Branch `experiment/20261026-air-overlap` (76b223a65; not taken, so neither the
span nor the hook is on main). The air's froxel pass is timed in a
span of its own, `froxel`; `air` keeps the tables (sky view, moon view,
exposure texel) in the `ahead` encoder. A measurement hook (gpu/api.rs
`STAMP_TIMELINE`) reports each span as its begin past the frame's first begin
and its length, so a frame's passes can be laid on one time line. Fixed path,
dpr 2, 2056 × 1169 (about 9.6 Mpx), `resolution` fixed, shadows off, AO
corner, clouds on, the dev build.

Two builds, two loads each: A, the froxel pass after the tables as on main,
durations only; B, the froxel pass first in the `ahead` encoder and the time
line on. A's loads ran while the machine was busy elsewhere (interval p95
128 and 173 ms against 83 in B's), so their absolute numbers are not compared.

## Numbers

| Load | `air` p95 | `froxel` p95 | `world` p95 | interval p95 / max |
| ---- | --------- | ------------ | ----------- | ------------------ |
| A1   | 21.2 ms   | 2.4          | 24.2        | 128 / 326          |
| A2   | 51.0      | 1.3          | 46.6        | 173 / 1347         |
| B1   | 23.9      | 0.7          | 22.0        | 83 / 830           |
| B2   | 29.4      | 0.9          | 19.7        | 83 / 166           |

From B2's time line (176 of 204 timed frames; 28 dropped where a span's
stamp lay over 400 ms from the rest), p50 / p95 / max:

| Figure                                             | ms                 |
| -------------------------------------------------- | ------------------ |
| `air` as stamped                                   | 5.4 / 29.4 / 41.1  |
| `air` begin to the frame's first compute pass      | 4.5 / 28.8 / 36.3  |
| `air` past the frame's first compute pass          | 0.6 / 6.6 / 10.3   |
| `world` from the last cast pass's end to its own   | 5.3 / 8.5 / 12.9   |

B1's `world` so measured: p95 8.1, max 11.7; in frames with no cast, 7.9.

## What it showed

- **The air costs about a millisecond, not 20–50.** The froxel pass is under
  3 ms in every frame, first in its encoder or not. The tables' render passes
  stamp their begin before anything of the frame has run, while the GPU still
  works on the frame before, and end as the frame's compute begins: a frame
  whose `air` read 30.2 ms began its first compute pass 29.4 ms after `air`
  began.
- **`world` is 5–9 ms of its own.** Its begin is stamped while the cast's
  `miss` pass still runs, and it ends 5–9 ms after the cast's last pass; that
  holds in still frames too. Its swing to 30–50 ms is the cast's tail.
- On this GPU (Apple M1 Max, Metal through Dawn) a render pass's span is not
  its cost: the begin is taken before the passes it waits on finish. A compute
  pass's span is. A render pass is weighed by its end against the pass before
  it ends.

So neither `air` nor `world` is where the frame's time goes while moving; the
cast's passes are (`beam`, `sample`, `miss`, 15–40 ms each at p95). What can
be saved in the world pass (clouds, decor, the screen pass) is bounded by its
5–9 ms.
