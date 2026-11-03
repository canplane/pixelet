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
is what the S23 pays for. A reading, measured apart below.

## What a visit pays

A synthetic pass (`tools/visit.mjs`, a page of its own, no renderer): the real
pass's size, workgroup 8×8, dispatch 28×49, 14 steps, 1.23 million visits, over a
table of 2^19 entries of 12 words, 25 MB, read at a hash of the coarse pixel and
the step, so neighbours share nodes. A visit hits at its first slot in 13 loads, as
`sample.gen.wgsl` does, where every read of the pyramid is an `atomicLoad`. Each
thread's hits are read back. Each variant is warmed for a second and timed by the
pass's timestamps, 3 rounds of 12, ns a visit. Wall clock agrees (`reports/visit-*.json`).

| Variant | S23 | Mac |
| --- | --- | --- |
| Plain loads, a read-only binding | 0.53 | 0.03 |
| Every load atomic, as now | 8.85 | 0.03 |
| The keys atomic in their own buffer, the rest plain | 1.42 | 0.03 |
| Plain, every address random | 8.36 | 0.46 |
| Every load atomic, every address random | 22.50 | 1.03 |
| Plain, the 33-entry stack, read and written at a run-time index | 21.56 | 0.18 |
| The same, index only, no fill | 19.35 | 0.15 |
| The same, fill only | 0.53 | 0.03 |
| The same, a stack of 8 | 0.59 | 0.04 |
| Keys atomic, the 33-entry stack | 19.94 | 0.18 |
| Keys atomic, a stack of 8 | 1.40 | 0.04 |

Locals that never run, behind a uniform that is 0, each assigned in the loop and
read only after it, on a dispatch of 7×12, as the full dispatch lost the S23's
device at 2,000 of them:

| Locals | S23 | Mac |
| --- | --- | --- |
| 0 | 0.61 | 0.26 |
| 125 | 1.09 | 0.30 |
| 500 | 434 | 0.61 |
| 1,000 | 610 | 1.09 |
| 2,000 | 4,998 | 2.13 |

- **The 33-entry stack costs 20 ns; 8 entries cost none.** The indexing, not the
  fill.
- **All-atomic loads are 6–19 times keys-only.** It moved between runs: 8.85 here,
  28.63 and, unwarmed, 10.3 in the runs before (`reports/visit-s23-first.json`), so
  it is no figure to hold, only its order. The two rows of random addresses are of
  that earlier run, the rest of the last.
- These do not add: atomic with the 33-entry stack took 17.4 where plain with it
  took 21.4.

### Live words, not locals

The locals above are live across the loop, each assigned in it and read after it;
`sample`'s are not. `tools/live.py`, a structured backward liveness over the
generated WGSL, counts function_1 at `d25f5b631`:

| | Vars | At the walk's head | Peak, vars and lets |
| --- | --- | --- | --- |
| `sample` | 1,584 | 16 vars, 150 words (132 the stack), 83 lets | 301 words, line 8461 |
| `miss` | 1,601 | 16 vars, 150 words, 86 lets | 304 words |
| `beam` | 1,460 | 17 vars, 151 words, 85 lets | 327 words |

A let counts a word here, whatever its type, so the words are a lower bound.

The pass again, on a dispatch of 7×12, ns a visit, S23 and Mac
(`reports/visit-*-ladder.json`), with locals of `sample`'s shape beside them: declared
at the function's top, assigned and read inside the block that never runs, so none
is live across anything:

| Locals, live across the loop | S23 | Mac |
| --- | --- | --- |
| 50 | 0.65 | 0.57 |
| 100 | 1.04 | 0.48 |
| 125 | 1.09 | 0.48 |
| 150 | 1.13 | 0.57 |
| 175 | 4.88 | 0.52 |
| 200 | refused: `VK_ERROR_UNKNOWN` | 0.52 |
| 250 | 81–150 | 0.61 |
| 300 | 102.5 | 0.57 |
| 400 | 164.6 | 0.65 |
| 500 | 242 | 0.70 |

| Locals scoped to the block | S23 | Mac |
| --- | --- | --- |
| 100 | 0.48 | 0.39 |
| 500 | 0.61 | 0.44 |
| 1,500 | 0.65 | 0.35 |
| 3,000 | 0.65 | 0.35 |
| 6,000 | 0.65 | 0.35 |

- **The count of locals costs nothing; live words do.** 6,000 scoped locals cost the
  S23 nothing. Live across the loop, it holds to 150, rises at 175, refuses a
  pipeline at 200 and is at 80–240 ns past it. A reading: what fits in registers.
- **`sample`'s peak of 301 words lies past that cliff, and the synthetic at 300 takes
  102.5 ns against the real 111.** A likeness, not a proof: the peak is one point in
  the code, not the whole loop, and a let may hold more than a word.
- **The same driver error as the shaders.** `VK_ERROR_UNKNOWN` in
  `vkCreateComputePipelines` at 200 live words is what
  [the shaders on two phones](../20261103-shaders-on-two-phones/README.md) met, there
  without a cause in the WGSL.

### What the S23 did to its table

With every load atomic on a read-write binding, and 250 or more locals live, the
S23's pass left the table changed, though the shader only loads. The whole table
read back after `atomic_rw live 250`: 279 words of 6.3 million differ from what was
written, none of them 0, the first at word 103,352. The Mac's table is untouched.

From that pass on, in that device, every pass answers as if the table were empty,
plain and read-only ones too: every thread's hits 0, its `a` 0. 279 words cannot
do that, and the copy of the table still reads the rest whole. A word written by
the queue reads back, so the read is sound. No error, the device not lost,
timestamps running as before. It took a new device to come back.

| | S23 | Mac |
| --- | --- | --- |
| Atomic, 125 live | right | right |
| Atomic, 250, 500 or 1,000 live | wrong, and everything after | right |
| Plain read-only, up to 500 live | right | right |
| Atomic, up to 6,000 scoped | not read apart: they ran after the fault | right |

Seen on each of the 6 runs that reached it. A plain read-write binding past
the cliff, and whether the real page does this, were not tried. `sample` is an atomic
read-write binding with a peak past that count, and that the page draws says no more
than that its reads are not all lost. Not reported to anyone.

## Step 1a: the short stack

`05e8ae4a5` on the branch `walk-visit`: a walk with no line holds 8 entries where it
held 33. `sample.gen.wgsl`: 2,201 `var`, peak 207 words where it was 301, 58 words at
the walk's head where it was 150; `beam`, which keeps its 33, 311.

The matched circle on the S23, the same dev build's profile, a cooler phone at the
start (`reports/s23-step-1a.json`, against `reports/s23-matched.json`):

| | `d25f5b631` | `05e8ae4a5` |
| --- | --- | --- |
| fps | 6.9, 7.0 | 8.0, 8.3 |
| GPU p50, ms | 133.5, 129.2 | 106.0, 101.6 |
| `sample` p95 | 121.8, 120.6 | 120.8, 118.6 |
| `miss` p95 | 67.6, 68.7 | 49.1, 56.4 |
| `beam` p95 | 46.1, 42.9 | 38.6, 34.7 |
| `generate` p95 | 40.8, 26.3 | 34.7, 10.9 |
| nodes a cast: `sample` | 1,120,897 | 1,130,461 |
| nodes a cast: `beam` | 318,922 | 320,848 |
| nodes a cast: `miss` | 265,428 | 239,126 |
| `sample`, ns a node | 111 | 111 |

- **`sample` did not move**, at 207 words, still past the synthetic's 175.
- `beam` should not have moved and fell 15–20%, so the frame's gain is not all the
  stack's. `miss`, also a walk with no line, may have gained; `sample` did not.
- It draws on both, and the S23 makes its pipelines. The picture was not compared.
- A Mac delta is not given: the two Mac runs were on different displays (a device
  pixel ratio of 2 at 120 Hz, then 1 at 60 Hz), and `beam` moved there too.

## The real page's answers

Whether the page does what "What the S23 did to its table" did, at `d25f5b631`,
through a wrapper of `createBuffer` that finds the pyramid by its size and reads it
back between frames (`tools/pyramid.mjs`, `tools/cmp3.py`, `reports/pyramid-*.json`).

**Each kept entry against the code.** Its slot in the bucket its zoom and node hash
to, a node one under the world's roots, a kind that exists. At the default view, the
circle, and the drive with the shadows on, 7 reads a run: about 54 thousand kept on
the Mac, 52 thousand on the S23, none broken on either.

**The same node's answer, device against device.** Kind and asset, of each node the
two runs both kept, under one key of its zoom, its index and the zooms asked:

| | Nodes in both | Answers differ |
| --- | --- | --- |
| At rest, the Mac / the S23 | 6,009 | 0 |
| At rest, the S23 / the S23 | 6,011 | 2 |
| The circle, the Mac / the Mac | 54,749 | 4 (0.01%) |
| The circle, the S23 / the S23 | 33,758 | 2,283 (6.76%) |
| The circle, the Mac / the S23 | 51,154–65,374 | 1,203–1,225 (1.9–2.4%) |

At rest the S23 answers as the Mac does. Moving, it differs from itself by more than
from the Mac, at the finest zooms: a slow phone's kept answers are older, of what had
arrived by then. So no sign that the page suffers the table's fault. This reads what
the pyramid keeps, at one place, not the pass's other buffers, and only the fault's
effect on what is kept; `S23 / S23` lost its last reads to a run that stopped early.

## The hand kernel

`749560da1` on `walk-visit`: `sample_hit.wgsl`, 367 lines by hand, a pass before
`sample_cs` over the pyramid alone, read-only, no atomics, no stack, no `world::node`.
What it cannot end it leaves to `sample_cs`, which walks it from the start. Live words
(`tools/livehand.py`): peak 47 in its entry, its callees' own not counted, against
`sample`'s 301.

The matched circle on the S23, two windows each (`reports/s23-hand-kernel.json`):

| | `d25f5b631` | `749560da1` |
| --- | --- | --- |
| fps | 6.9, 7.0 | 14.6, 14.0 |
| GPU p50, ms | 133.5, 129.2 | 48.3, 51.5 |
| GPU p95, ms | 244.0, 241.3 | 145.1, 171.5 |
| `sample_hit` p95 | | 2.2, 2.4 |
| `sample` p95 | 121.8, 120.6 | 47.3, 54.2 |
| `miss` p95 | 67.6, 68.7 | 56.6, 57.5 |
| `beam` p95 | 46.1, 42.9 | 34.1, 35.6 |
| `generate` p95 | 40.8, 26.3 | 13.3, 14.4 |

- **It makes its pipelines and draws**, on the S23 and the Mac.
- **The hand pass takes about 2.4 ns a node** on the S23, against the old walk's 111:
  p95 2.4 ms over the 1.02 million nodes it would leave ended, the baseline's count
  less what is left, so an estimate. The Mac's `sample_hit` is 1.2 ms over about 1.21
  million, about 1.0 ns. The ratio a node falls from 62 to about 2, where the passes
  that walk no tree stand. A reading: the cost was the shape, not the phone. It walks
  hits alone, so a miss's cost is not in that figure.
- **What stays is what remains in the big shader.** `ray_counts`' `sample` lane now
  counts the left ones' walks alone: 103,494 nodes a cast on the S23, 9.2% of the
  baseline, and 11,851 on the Mac, 1.0%. Why nine times as many are left on the
  phone is open. 54.2 ms over those is 524 ns a node, far past the 111, so `sample`
  either pays to be entered, by all 84,413 threads though most return at once, or
  pays more on a left ray. These were not told apart.
- **Auto** settles at a scale of 0.50 and a lattice of 2: 21–22 fps, `sample` 61–68,
  `beam` 18, `miss` 28.
- **The drive is untouched**: 1.9 fps, `light_walk` p95 1,028 ms. The light's walks
  are by the old shader.
- **The picture** on the Mac at rest, sky fixed, against `d25f5b631`: by eye the same;
  58% of pixels differ, 8.7% by over 2 of 255, 0.38% by over 8 outside the companion,
  0.0001% by over 32. A frame against itself in one run differs only on the companion.
  What of that is run to run was not told: a second run opened on another display.

Not checked: that the hand walk answers as `cast::walk` does, ray by ray. The walk is
written twice and only this picture holds them.

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
- A grant by `Browser.grantPermissions` ends with the session that made it: closed,
  the page sits on "Locating…", `stood`, `pivot -`, with no error. Keep it open.
- The S23 loses its device on a pass too long, with no shader error. `visit.mjs`
  sizes each variant's pass to about 50 ms from its first run.
- The S23's GPU clock climbs under load: one dispatch from idle read up to 3 times
  the warmed time. Warm it first.

## Static reach

`tools/scip.py`, `tools/reach.py` and `tools/block.py` read rust-analyzer's SCIP
index of the main and the shader workspaces (`rust-analyzer scip`) into what the
GPU and the CPU reach of pxl-engine and pxl-content, the figures in the minutes'
"What one source is for". They match trait impls by name and take every non-test
function of a client or server crate as a root, so the lines are a static bound and
no run-time count.

## Not measured

- The whole of the walk under the cliff: `sample_hit` is the samples' hits alone.
- The real page's other buffers, against what the synthetic did to its table.
- `sample`'s cost apart: being entered, and walking a left ray.
- The picture of step 1a against `d25f5b631`.
- What of a node's time is the driver and Chrome's path.
- The same WGSL in the S23's native app.
- A hand-written walk.
- The distribution of a ray's nodes: only the mean a cast.
- Nodes at a lattice of 2, or in the Mac's drive.
- Another Adreno or Mali.
