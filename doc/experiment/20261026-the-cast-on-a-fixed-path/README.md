# The cast on a fixed path

**2026-10-26.** What the eye's cast costs while the camera moves, measured on
one path every load repeats, as the baseline a rasterized proxy is weighed
against ([a proxy only starts the ray](../../minutes/20261024-a-proxy-only-starts-the-ray.md)).
The 10-24 baseline stood the camera still for 30 s after a load, so no cast in
it was one that moving asks for.

## How

The dev build (`make dev`) at cdff8c14e, Chrome 155 on an Apple M1 Max, the
window on the built-in display: dpr 2, a 4112 × 2226 canvas, about 9.2 Mpx.
Settings `profiler` on and `resolution` fixed, so the render scale holds at
1.00 and only the cast's cost moves.

Each load ran [bench.js](./bench.js) from the console: five seconds after boot,
a fresh recording window, then

| Step     | Time  | What                           |
| -------- | ----- | ------------------------------ |
| zoom out | 3 s   | distance 16 → about 4096       |
| hold     | 2 s   |                                |
| zoom in  | 3 s   | to the floor of 2              |
| back     | 1.5 s | 2 → 16                         |
| turn     | 4 s   | once round, four quarter drags |
| hold     | 3 s   |                                |

and the profiler's report over that window, about 19 s.

## Numbers

| Load | Cast frames | cast p95 | air p95 | world p95 | fps  |
| ---- | ----------- | -------- | ------- | --------- | ---- |
| 1    | 229         | 79.7 ms  | 59.0    | 57.1      | 22.2 |
| 2    | 256         | 78.7 ms  | 60.3    | 68.2      | 20.9 |
| 3    | 261         | 79.4 ms  | 70.6    | 65.1      | 21.5 |

Load 1 had screenshots taken during it, which checked that the path zooms and
turns as written. For comparison, the same build standing still for 30 s after
a load: cast p95 56.2 ms over 91 cast frames at dpr 2, and 19.1 ms over 87 at
dpr 1 (1920 × 993, about 1.9 Mpx).

## What it showed

- **The baseline is 79 ms**, three loads within a millisecond of each other: a
  little under five times the 16.7 ms gate, and above the 56 ms the same build
  casts standing still.
- **The cast is not the frame's only large span while moving.** `air` and
  `world` reach 57–71 ms at p95 on the same path, so a proxy that halves the
  cast does not halve the frame; it is judged by the cast span alone.
- **The cast grows slower than the pixels.** Standing still, 4.8 times the
  pixels cost 2.9 times the cast.
