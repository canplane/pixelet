# The day's structure against the tree before

One run of the app's own scenario (`perf/measure.ts`, `perf/drive.ts`) on the release
bundle of `096349ab89`, after `World::near` went, the orbit took one floor under its eye,
and a node not held whole took no bound and splits in the engine and the shader alike.

## Procedure

```bash
PXL_PERF_DRIVE=1 make preview
./script/perf/chrome.py http://localhost:4173 tmp/perf/chrome-1
```

The Mac, a fresh Chrome profile, the window shown throughout. `report/` holds the run's
`values.json`, `report.txt` and `said.txt`; its trace and shots are not kept.

## Numbers

| | p50 | p95 |
|---|---|---|
| tick, ms | 2.0 | 6.4 |
| GPU busy, ms | 26.2 | 46.1 |

607 frames over 20.0 s, 30.4 a second; the longest frame 99.1 ms; no error, no refused
stream, the device's generation 0.

## Against the tree before

The same day, `096349ab89` (change) against `630c31c8f2` (base, the tree before the day's
work, built in a worktree with the main checkout's packages; the two differ in the Rust
alone), both release bundles with `PXL_PERF_DRIVE=1`, by turns:

```bash
./script/perf/gate.py chrome http://localhost:4174 http://localhost:4173 3
./script/perf/gate.py android-chrome http://localhost:4174 http://localhost:4173 2
```

The phone, an S23, reached both servers by `adb reverse`. `report/chrome/` and
`report/android-chrome/` hold each run's `values.json`, `report.txt` and `said.txt`.

The Mac's Chrome, three rounds, each run's figures:

| run | GPU p50, ms | GPU p95, ms | frames | tick p50, ms | tick p95, ms |
|---|---|---|---|---|---|
| base 1 | 22.7 | 32.9 | 777 | 1.9 | 2.5 |
| base 2 | 24.8 | 35.0 | 723 | 2.0 | 3.4 |
| base 3 | 24.9 | 36.3 | 713 | 1.9 | 2.8 |
| change 1 | 26.1 | 34.7 | 714 | 1.7 | 2.3 |
| change 2 | 27.5 | 37.1 | 676 | 1.8 | 2.6 |
| change 3 | 27.3 | 37.2 | 680 | 1.8 | 2.4 |

The gate's medians: GPU p50 24.84 to 27.33, p95 35.00 to 37.09, frames 723 to 680, tick
p50 1.90 to 1.80, p95 2.80 to 2.40. It passes.

The S23's Chrome, two rounds, with the AP's temperature and the GPU's clock at each run's
last capture:

| run | GPU p50, ms | GPU p95, ms | frames | longest, ms | tick p50, ms | tick p95, ms | AP, °C | GPU, MHz |
|---|---|---|---|---|---|---|---|---|
| base 1 | 51.7 | 105.4 | 321 | 221 | 3.4 | 12.0 | 46.0 | 719 |
| change 1 | 68.0 | 119.7 | 259 | 197 | 3.4 | 9.7 | 48.7 | 680 |
| base 2 | 83.4 | 161.4 | 198 | 351 | 5.2 | 11.5 | 46.6 | 401 |
| change 2 | 121.3 | 155.7 | 157 | 301 | 4.9 | 10.2 | 46.3 | 348 |

The gate fails: the change's GPU p50 is past 1.15 times the base's. In each round the
change's GPU p50 is the higher, by 31% and by 45%, and it draws a fifth fewer frames; the
second round ran with the phone's GPU clocked down on both builds.

The change holds four things at once (`World::near` gone, one floor under the orbit's eye,
a node not held whole splitting in the shader, the pan's eye), and the runs do not part
them. Only the split is in the shader.

## With only the coarsest zoom asked deciding

The split of `a98cfe631a` took a node for not held where the finest clip level asked did
not hold it whole. Read again, a node is asked a range of zooms, each of the part its clip
level holds, and only the coarsest asked must hold it whole; the fix does that, in the
engine and the shader. Measured the same way, the fix on `1969fb1674` in a clean worktree
(change) against `630c31c8f2` (base); `report/fixed/` holds the runs.

The S23's Chrome, two rounds:

| run | GPU p50, ms | GPU p95, ms | frames | longest, ms | tick p50, ms | AP, °C | GPU, MHz |
|---|---|---|---|---|---|---|---|
| base 1 | 55.1 | 123.3 | 293 | 195 | 3.8 | 49.8 | 615 |
| change 1 | 74.2 | 119.5 | 245 | 244 | 3.5 | 54.3 | 615 |
| base 2 | 106.6 | 173.9 | 167 | 307 | 5.6 | 47.9 | 348 |
| change 2 | 75.0 | 174.2 | 192 | 328 | 4.2 | 51.1 | 295 |

The Mac's Chrome, three rounds:

| run | GPU p50, ms | GPU p95, ms | frames | tick p50, ms |
|---|---|---|---|---|
| base 1 | 20.8 | 29.2 | 868 | 1.8 |
| base 2 | 29.3 | 44.8 | 611 | 2.0 |
| base 3 | 26.0 | 38.4 | 693 | 2.1 |
| change 1 | 22.7 | 30.1 | 854 | 1.7 |
| change 2 | 32.3 | 45.9 | 565 | 2.0 |
| change 3 | 24.8 | 33.4 | 773 | 1.7 |

The rounds differ among themselves by more than the builds do: on the Mac the change's GPU
p50 is 9% and 10% over the base's and then 5% under, on the phone 35% over and then 30%
under with the GPU clocked down. These runs do not show whether the fix takes back what
the split cost, nor that the split is where the cost was.

## With the bound read in a brick's place again

`3a6938ffb5` (2026-10-29) records what a block without a bound over a brick not yet held
costs: a walk down from the world's top asked about 1000 nodes at the clip levels' edge,
173,512 nodes against 16,768. A third form put that bound back in the engine's fold, kept
the coarsest zoom asked deciding in `bound_over`, and had the CPU's ray alone ask under a
node read through a stand-in. The S23's Chrome, three rounds against the same base
(`report/labelled/`):

| run | GPU p50, ms | GPU p95, ms | frames | tick p50, ms | AP, °C | GPU, MHz |
|---|---|---|---|---|---|---|
| base 1 | 55.4 | 116.3 | 299 | 3.7 | 47.7 | 615 |
| change 1 | 68.7 | 154.3 | 220 | 5.2 | 46.6 | 401 |
| base 2 | 105.5 | 171.7 | 166 | 5.7 | 45.3 | 348 |
| change 2 | 115.5 | 167.1 | 160 | 5.9 | 45.0 | 348 |
| base 3 | 61.2 | 128.3 | 234 | 4.8 | 49.8 | 348 |
| change 3 | 115.7 | 172.8 | 160 | 6.6 | 46.0 | 348 |

The base alone ran from 55 to 106 ms as the phone clocked its GPU down: the phone, hot
from the rounds before, parts nothing here either.

What was committed after it is not this form: the shaders and `bound_over` are as they
were before the day's work, byte for byte, so the GPU's walk costs what the base's does by
construction, and the CPU's ray alone asks under a node whose bound is read through a
stand-in (`clipmap::stood_in`). That form was not measured.

## What it does not say

The first run above, alone, compares nothing. Safari and the iPhone were not measured.
