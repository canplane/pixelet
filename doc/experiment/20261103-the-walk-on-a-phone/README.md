# The walk on a phone

**2026-11-03.** Why the page runs at 6–7 fps on a Galaxy S23 as the view moves,
at `d25f5b631`, the deployed commit: what sets how often `beam` and `sample` run,
what a lattice of 2 buys, and how many nodes the walks visit against what each
takes on the S23 and on a Mac. What this means for the shaders is in
[the walk on a phone, and who owns it](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md).

## How

- **S23**: SM-S911N, Adreno 740, Chrome 156, viewport 411×750 at a device pixel
  ratio of 2.625, canvas 1080×1969. **Mac**: M1 Max, 32 GPU cores, Chrome 156,
  viewport 540×984 at 2, canvas 1080×1968. `Grid::of` lays the same grid on both:
  span 53×96, 217×389 samples, computed from it, not read back from a page.
- Both on the same dev build of a clean worktree of `d25f5b631` (`watch` wasm
  profile, delivery data), each at the same place (38.1193, 128.4656), driven over
  the DevTools protocol (`tools/run4.mjs`, `tools/cdp.mjs`). The profiler is
  `pxl.perf`, which only a dev build installs.
- **The circle**: Fixed, shadows off, default view (distance 16 u, polar 60), a
  right-drag circle of radius 100 CSS px turning every 6 s. Windows of 25 s for
  time and 15 s with `ray_counts` on for nodes.
- **The drive**: `pxl.perf.drive(25)`, which turns the shadows on.
- **Auto**: the S23 alone, the circle for seven windows of 25 s.
- **Resolution**: `reports/s23-by-resolution.json`, the S23 alone, an earlier run
  of the same circle on the same commit, with the shadows at their default, off.

`reports/` holds each window's whole report. `tools/show4.py` prints them. A span
is its p95 in ms over the frames a read timed that ran it, as the profiler reports
it, and a node's time below is that p95 over the mean nodes a cast, so it compares
one device with the other and is no mean cost.

## The work is the same

Nodes a cast, the circle, `ray_counts` on:

| | S23 (29 casts) | Mac (444 casts) |
| --- | --- | --- |
| `beam` | 318,922 | 337,602 |
| `sample` | 1,120,897 | 1,223,913 |
| `miss` | 265,428 | 347,364 |
| nodes a sample | 13.3 | 14.5 |
| pixels that took the visible sample | 99.54% | 99.44% |

The S23 visits no more than the Mac does.

## A node's time is not

The same circle, the same window:

| | S23, ms | Mac, ms | S23, ns a node | Mac, ns a node | ratio a node |
| --- | --- | --- | --- | --- | --- |
| `sample` | 124.7 | 2.2 | 111 | 1.8 | 62 |
| `miss` | 69.0 | 2.0 | 260 | 5.8 | 45 |
| `beam` | 44.0 | 1.8 | 138 | 5.3 | 26 |

The passes that walk no tree, from the windows without `ray_counts`:

| | S23, ms | Mac, ms | ratio |
| --- | --- | --- | --- |
| `finish` | 1.4 | 1.8 | 0.8 |
| `px` | 1.2 | 0.9 | 1.3 |
| `sraster` | 1.8 | 1.2 | 1.5 |
| `world` | 8.7 | 2.9 | 3 |
| `resolve` | 0.9 | 0.3 | 3 |
| `face` | 4.8 | 0.6 | 8 |
| `generate` | 40.8 | 1.4 | 29 |

The frame: 6.9 fps against 60; GPU p50 133.5 ms against 7.7, p95 244.0 against
10.0. On the S23 a turn is under 8 ms and none is long; 24.4 s over 169 frames is
144 ms a frame, the GPU's time, so the GPU sets the pace. Thermal status 0
throughout, the AP at 44–47 °C once running.

1.7 million nodes a cast at 16.7 ms is under 10 ns a node with nothing else in the
frame. The S23's are 111–260 ns.

On the Mac a `sample` node costs a third of a `beam` node; on the S23, four fifths.
`sample` reads what `beam` already wrote to the pyramid, so its visits mostly hit.
A reading: what a visit costs before it asks anything, which a hit does not avoid,
is what the S23 pays for. A reading, not measured apart.

## What runs by what

`beam` and `sample` dispatch on `Grid::corners` and `Grid::samples`
(`gpu/ray/cast.rs` `dispatch`), from the screen's tangents and the lattice
(pxl-engine `beam.rs` `Grid::of`): an angle, `CELL` = 1/96 rad and `SAMPLE` = 4,
whatever the pixels. It is the contract (`ARCHITECTURE.md`, Drawing). On the S23,
the circle:

| Scale | Mpx | fps | GPU p50 | `sample` | `beam` | `miss` |
| --- | --- | --- | --- | --- | --- | --- |
| 1.00 | 2.1 | 5.9 | 154.7 | 133.2 | 55.4 | 92.1 |
| 0.71, Auto | 1.1 | 6.2 | 152.1 | 147.3 | 55.5 | 62.9 |
| 0.38, Low | 0.3 | 6.8 | 139.2 | 138.0 | 53.2 | 19.3 |

Seven times fewer pixels take 10% from the frame; `miss` alone follows them.

## A lattice of 2

Auto on the S23, each window 25 s, as the judge moved:

| Window | Scale | Lattice | fps | GPU p50 | `sample` | `beam` | `miss` | `generate` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 0.71 | 1 | 7.6 | 120.5 | 119.5 | 44.4 | 52.5 | 20.9 |
| 1 | 0.50 | 1 | 7.9 | 114.4 | 121.1 | 44.7 | 24.4 | 24.6 |
| 2 | 0.50 | 2 | 7.7 | 107.3 | 170.1 | 22.0 | 26.0 | 30.0 |
| 3 | 0.50 | 2 | 6.9 | 124.6 | 171.1 | 22.0 | 30.5 | 30.3 |
| 4 | 0.50 | 2 | 6.5 | 145.4 | 182.0 | 23.2 | 34.3 | 35.4 |
| 5 | 0.50 | 2 | 6.2 | 144.6 | 189.4 | 24.8 | 27.7 | 37.0 |
| 6 | 0.50 | 2 | 6.2 | 144.4 | 188.8 | 24.7 | 30.7 | 36.2 |

`beam` halves, `sample` grows by 50–70 ms, the frame gains nothing. The judge,
with both knobs at their floor, holds 6–8 fps. The windows' views differ, as the
circle turns, and the later ones ran hot for longer; thermal status stayed 0. Nodes
were not counted at a lattice of 2.

## The drive

Not one cost. On the S23 two runs gave GPU p50 415.6 and 229.1 ms, with the view
at 2.0 u and 525.5 u as each was read; `light_walk` p95 324.3 and 951.3. Asked at
its end the S23's light walked 1,828,794 `miss` nodes a cast, 1,030.6 ms p95, about
560 ns a node. The Mac's frames at that view stood (`gated`), so it counted none.
On the Mac the drive's `cast` frames are 264 of 1,491 timed; on the S23 all 38, so
the slow phone does again what the Mac's frames skip.

## First load

To the world, cold: about 94 s on plei.me, with one restart of the page near 60 s
that logcat does not explain, and 91 s on the dev build with none. Warm: 15 s.
Moving on plei.me over `requestAnimationFrame`: 7.5 fps, p50 33.9, p95 550.1 ms, no
long task (`tools/raf.mjs`).

## What a run must keep clear of

- A phone left alone locks, and a locked page draws nothing and reports nothing
  amiss: canvas 300×150, no frame. Hold a wake lock from the first moment, not once
  the world is shown.
- `Emulation.setDeviceMetricsOverride` with a scale factor did not reach the canvas
  on the Mac: 411×750 at 2.625 stayed 822×1500. 540×984 at a factor of 0 gives
  1080×1968. Read `canvas.width`.
- A window with no GPU read is no measurement, whatever its fps: `stood`, `draws
  0`, `no timestamps`. `run4.mjs` stops where no read landed.
- The toast the script waits on is Korean. Chrome on the Mac starts with
  `--lang=ko`.
- `main` past `d25f5b631`: at `69ff22b9a` the Mac's page drew nothing, its
  `FS_MOUNT` refused with `EINVAL`. That commit changes `spec/spec/bake/publication.json`,
  so the delivery data may lag it. The cause was not traced.

## Static reach

`tools/scip.py`, `tools/reach.py` and `tools/block.py` read rust-analyzer's SCIP
index of the main and the shader workspaces (`rust-analyzer scip`) into what the
GPU and the CPU reach of pxl-engine and pxl-content, the figures in the minutes'
"What one source is for". They match trait impls by name and take every non-test
function of a client or server crate as a root, so the lines are a static bound and
no run-time count.

## Not measured

- What of a node's time is the visit's atomics (each loads up to `pyramid::BUCKET`,
  4, keys), the WGSL's shape or the driver and Chrome's path.
- The same WGSL in the S23's native app.
- A hand-written walk.
- The distribution of a ray's nodes: only the mean a cast.
- Nodes at a lattice of 2, or in the Mac's drive.
- Another Adreno or Mali.
