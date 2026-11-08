# The frame on the S23

**2026-11-03 to 2026-11-08.** This record follows the page's frame on a Galaxy S23 from
6–7 fps under rust-gpu's generated shaders, through the walks written by hand, to main
measured against the deployed build: what each pass costs on the phone and why, what was
changed for it, and what each change gave. It closed on 11-08 with main against the
deployed build `e317a3644b` as follows: the Mac's gate passes with its GPU faster; on the
S23 at 719 MHz main's GPU time is 6.1 times shorter at p50, and at p95 1.7 to 2 times
shorter than the deployed build's best run; the tick's tail, about 1 ms over the deployed
build's, is accepted as the cost of filling the bricks on the CPU. What the S23's p95 still
holds is the light's walks, which the light's list did not undercut
([the deployed build is the bar](../../minutes/20261107-the-deployed-build-is-the-bar.md)).

What the measurements of 11-03 meant for the shaders is in
[the walk on a phone, and who owns it](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md).
The passes by hand are in [taking rust-gpu out](../20261103-the-shaders-by-hand/README.md);
rust-gpu went on 2026-11-05. The decisions the later runs served are
[the frame is what it must draw](../../minutes/20261106-the-frame-is-what-it-must-draw.md),
[a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md),
[the deployed build is the bar](../../minutes/20261107-the-deployed-build-is-the-bar.md) and
[the light's list](../../minutes/20261108-ray-raster-ray.md).

## How the runs were taken

**The devices.** The S23 is an SM-S911N with an Adreno 740. Its Chrome 156 runs at a
viewport of 411×750 and a device pixel ratio of 2.625, a canvas of 1080×1969. Its GPU's
clock is capped at 719 MHz until heat steps the cap down; 680, 615, 550, 348 and 295 MHz
were logged. The Mac is an M1 Max with 32 GPU cores. On 11-03 its Chrome 156 ran at a
viewport of 540×984 at a ratio of 2, a canvas of 1080×1968, so that `Grid::of` laid the
same grid on both devices: a span of 53×96 and 217×389 samples, computed from the grid, not
read back from a page. Later Mac runs say when they are at 4K. Safari is Safari 27.0.1 on
the Mac.

**11-03 and 11-04.** Both devices ran the same dev build of a clean worktree of
`d25f5b631`, the commit then deployed (`watch` wasm profile, delivery data), each at the
same place (38.1193, 128.4656), driven over the DevTools protocol (`tool/walk/run4.js`,
`tool/walk/cdp.js`). The profiler is `pxl.perf`, which only a dev build installs. The
shell scripts in [`tool/walk/`](tool/walk/) serve each commit's page from `ts/app/web`,
the page's tree of the time, in worktrees of their own; the scratchpad and the cargo target
they name were those of the session that ran them (`572d46ae`), and both are gone.

**Where these runs stood.** A rename on 10-29 (`98ed13185d`) stored the leaf tile's corner
in place of the GPS fix, until 11-08 (`bb12471c6e`). So every run in this journal, and every
`compare-perf`, asked for the ridge at 38.1193, 128.4656 and stood about 300 m away, at
38.11727, 128.46313. The numbers below are that corner's. A drive from the corner and one
from the ridge were not the same measurement, and what differed was the camera, not the
ground. The drive sweeps the zoom by relative wheel steps, and under the default `climb` the
terrain stops a zoom out that would put the eye in the ground, so the steps it stopped are
lost. On 11-08, on the Mac, the eye's median distance from the pivot was 11 u from the
corner, where it sat at its nearest, 2 u, for much of the window, and 630 to 740 u from the
ridge. Tasks took 0.1 ms a turn against 9 ms, terrain tile reads were about 800 against
1,200, and the GPU's p50 was 19.8 ms against 23.5 ms. With `orbit_obstacle` at `through`,
the ridge, the corner, 38.1150, 128.4700 and 38.1300, 128.5000 all had a median distance of
1,560 to 1,880 u, 9.8 to 10.0 ms of tasks, 1,296 to 1,369 tile reads and a GPU p50 of 22.4
to 23.2 ms. The drive holds the gate at `through` from that day on.
[`report/`](report/) holds each window's whole report, and `tool/walk/show4.py` prints
them. The scenes were these:

- **The circle**: fixed resolution, shadows off, the default view (distance 16 u, polar 60),
  a right-drag circle of radius 100 CSS px turning every 6 s, timed in windows of 25 s, and
  windows of 15 s with `ray_counts` on for nodes. A "6 s turn" or a "24 s turn" below is this
  circle at that period.
- **The drive**: `pxl.perf.drive(25)`, which turns the shadows on.
- **Auto**: the S23 alone, the circle for seven windows of 25 s.
- **Resolution**: `report/s23-by-resolution.json`, the S23 alone, an earlier run of the same
  circle on the same commit, with the shadows at their default, off.

A span's figure in these runs is its p95 in ms over the frames a read timed that ran it, as
the profiler reports it. A node's time is that p95 over the mean nodes a cast, so it compares
one device with the other and is no mean cost.

**11-05 to 11-08.** The page is the main checkout's, its guest built (`pnpm build:wasm`)
and served by its own Metro as a dev bundle, with the tiles and assets from the local shelf
(`PXL_DEV_SOURCE=shelf`) instead of plei.me unless a run says otherwise.
`tool/s23/s23.js` loads a page on the S23's Chrome over adb, settles, and runs the scenes
asked; `tool/s23/ab.sh` alternates two trees, a fresh load each, and `tool/s23/show.py` and
`tool/s23/trace.py` read what the runs saved. S23 figures are ms p50 / p95 over fresh loads
that alternate base and change in one sitting, two loads a side unless a run says otherwise.
From the visit A/B on, a load is timed only once the ground's textures have landed;
before, whether they had was a confound. Data and shots went under `out/s23/`, which is not
kept.

**From 11-07, against the deployed build.** The bar is the deployed build: `e317a3644b`
with `e2b019f1b0` and `b6dac91e78`, so that its drive is not pinned, served by Vite from its
own tree (`.claude/worktrees/deployed`) on port 8291. The change is main with its working
tree, served by Metro on 8298. Both are dev builds, both off the shelf on 3001. The Mac's
figures come from the gate, `script/compare-perf.py`, which runs base and change by turns
for the rounds named; `tool/s23/cmp-prof.py` takes one drive's `pxl.perf.report()` on the
Mac, and `tool/s23/cmp-rest.py` takes 8 s of the drive, then 65 s untouched, and shots. The
S23's figures are `pxl.perf.drive(120)` and then 65 s untouched, fresh loads by turns, with
the GPU's clock read every 5 s (`tool/s23/cmp-hot.js`, run by turns by `cmp-s23.sh` and
tabulated with the clocks by `cmp-s23tab.py`). Each tool runs under the GPU lock, and the
data went under `out/cmp/`, not kept. Safari's runs are under "Memory, submits and Safari".
[tool/s23/README.md](tool/s23/README.md) gives the commands.

**The drive was pinned until a fix made before the hot runs of 11-06.** A held drag and the
wheel past the horizon left the camera 2 u under the level, where every step but a vertical
drag was refused, and a user could hit it. `collision.rs` was fixed with a test, and
`drive.ts` now tilts to just over the horizon. Every drive figure the runs took on main before
the fix is about 6 s of casts and about 14 s of one standing close-up; below, such figures are
marked "pinned". Their cast frames' and passes' tails compare only with each other, their fps
and gated share mean nothing, and a `mark` of 6.2 ms in them is the character's fragments up
close. The honest
drive's baseline, from one run: 326 frames, all cast, 16.4 fps, busy 52 / 115 ms,
`light_walk` p95 39, `miss` 27, `light_sample` 18.5, `sample` 17.

**Static reach.** `tool/walk/scip.py`, `tool/walk/reach.py` and `tool/walk/block.py` read
rust-analyzer's SCIP index of the main and the shader workspaces (`rust-analyzer scip`) into
what the GPU and the CPU reach of pxl-engine and pxl-content: the figures in the minutes'
"What one source is for". They match trait impls by name and take every non-test function of
a client or server crate as a root, so their line counts are a static bound, not a run-time
count.

**Commit names.** A commit of 11-03 and 11-04 named here that `git log --all` no longer
shows was of the work in progress. The same change is on `original`, rebased, under its own
subject. Main holds the hand walks squashed in `6c196144d4`, `f9f66682e` and `e8d616ba4` in
`47d238aab4` (`e8d616ba4`'s tree), and `355b65140` in `4cd32d38ef` (the same tree).

| Named here | On `original` |
| --- | --- |
| `05e8ae4a5` | `5e908d68e8` |
| `749560da1` | `a6165918dc` |
| `94d6e9284` | `e00353e76d` |
| `e7757099f` | `76b40dff10` |
| `8ac7315b1` | `8d0f501633` |
| `2a9b235f3` | `067b76a6d1` |
| `7bf251816` | `1eef3ee8c6` |
| `3e7ba241d` | `20fd63c0e5` |
| `1af780b72` | `3f1fd41f2a` |
| `2f7d18b37` | `8650432b78` |
| `981947702` | `e93ecb5a8c` |
| `b7457102e`, `9029f35dc` | `25d2136ca7` |
| `430cdc831` | `251dd052ed`, the same tree |
| `69ff22b9a` | `27d811459a`, the same tree |

## What a run must keep clear of

**The phone.**

- A phone left alone locks, and a locked page draws nothing and reports nothing amiss: the
  canvas is 300×150 and no frame comes. Hold a wake lock from the first moment, not once the
  world is shown.
- The S23 loses its device on a pass that runs too long, with no shader error.
  `tool/walk/visit.js` sizes each variant's pass to about 50 ms from its first run.
- The S23's GPU clock climbs under load: one dispatch from idle read up to 3 times the warmed
  time. Warm it first.
- A tab left open for each run piled 32 tabs on the phone and took an hour's numbers low.

**The page's state.**

- A view is kept by origin, and a run opens where the last one left. Clear the camera's
  storage before a load, log the camera, and compare pictures only where the views match.
  Stored state spoiled the six runs before those of `2a9b235f3`'s picture: two of them sat on
  `pivot -` for 180 s, one at `2a9b235f3` and one at `d25f5b631`, and none of the six from
  cleared storage did. Clearing every store makes each load cold.
- A touch moves the camera. `Input.setIgnoreInputEvents` holds real input off a run's tab,
  and in-page events still pass.
- The strata's textures land at their own time and change `world` by several ms. Settle
  before timing and look at the `-settled.png` shot.
- A DevTools geolocation override alone can leave the page waiting on a position ("A load
  that never draws", under First load): stub `navigator.geolocation`. A grant by
  `Browser.grantPermissions` ends with the session that made it; once that session closes,
  the page sits on "Locating…", `stood`, `pivot -`, with no error. Keep it open.

**The harness.**

- `Emulation.setDeviceMetricsOverride` with a scale factor did not reach the canvas on the
  Mac: 411×750 at 2.625 stayed 822×1500, while 540×984 at a factor of 0 gives 1080×1968. A
  device-scale override changes `devicePixelRatio`, not the canvas. Read `canvas.width`.
- A window with no GPU read is no measurement, whatever its fps: `stood`, `draws 0`,
  `no timestamps`. `tool/walk/run4.js` stops where no read landed.
- The toast the script waits on is Korean, so Chrome on the Mac starts with `--lang=ko`.
- A wait of 90 s failed a cold load.
- `main` renames what a harness imports, and a run that cannot turn the profiler on reads as
  "never drew". At `69ff22b9a`, past `d25f5b631`, the Mac's page drew nothing, its
  `FS_MOUNT` refused with `EINVAL`; that commit changes `spec/spec/bake/publication.json`, so
  the delivery data may lag it, and the cause was not traced. Since `2651534165` main names
  `src/services/share.ts` `src/service/share.ts` and no longer exports `perf_frames` from
  `src/port/perf.ts`, so `tool/s23/s23.js`, which reaches the page's modules by path, does
  not run against main.
- `pxl.perf.drive` sets `res` to fixed itself. The `perf` setting does not stop the
  timestamps' read; it only shows the panel.

**Noise.**

- One sitting drifts as the phone warms. Four 25 s circles in order with the shadows never
  on (`tool/walk/four.sh`) read `06a46035c` 24.1 fps, `6e9a7cb83` 22.1, `e8d616ba4` 19.8 and
  `06a46035c` again 19.4, with thermal status 0 throughout and every pass rising together: a
  run late in a sitting reads about 20% below an early one. With the shadows off the light's
  walks do not run, so these runs say nothing of `e8d616ba4`'s cost to the eye; they say only
  that `6e9a7cb83`'s `Aim` shows nothing above the drift. Compare runs within an alternation.
  The `therm` lines of a run's log give the thermal status and the AP's temperature. Thermal
  status reached 1 at an AP of 48.7 °C after about an hour of runs, and again after each long
  round.
- The first run of a pair runs faster. The atomics' first round read −3 fps from that alone.
- On the S23 a pass's p95 moves up to twice between runs of the same code, so only windows of
  one run compare. `7bf251816` runs the passes of `8ac7315b1`, and its `face` p95 read 6.6–7.0
  ms where `8ac7315b1` read 12.9–14.1 (and `2a9b235f3` 4.6–5.6); `world` read 8.7 at the one
  and 11.8 at the other. `world` and `face` trade by load, not by commit: one load of a parent
  read `world` 5.7–6.4 and `face` 15.7–18.2, the next 9.2–9.4 and 6.6–7.0. The cause was not
  traced.
- A baseline takes two cool runs and one sustained run (`DRIVE_S=300 FRAMES=1`, its last
  120 s), the phone warm as a user's.
- Mac runs on different displays do not compare. Step 1a's two Mac runs were at a device
  pixel ratio of 2 at 120 Hz, then of 1 at 60 Hz, and `beam` moved there too.
- Two runs of one commit differ in their pictures by as much as two commits do ("The
  picture").

## What a node visit costs on the S23

At `d25f5b631`, under rust-gpu's generated shaders, the page ran at 6–7 fps on the S23 as the
view moved. The S23 visits as many nodes as the Mac and pays 26 to 62 times as much for each.
The synthetic pass below ties that cost to the words a shader keeps live across its walk, and
the hand kernel later took the ratio a node from 62 to about 2, which reads as the cost being
the shader's shape, not the phone.

### The same work, not the same time

Nodes a cast over the circle, with `ray_counts` on:

| | S23 (29 casts) | Mac (444 casts) |
| --- | --- | --- |
| `beam` | 318,922 | 337,602 |
| `sample` | 1,120,897 | 1,223,913 |
| `miss` | 265,428 | 347,364 |
| nodes a sample | 13.3 | 14.5 |
| pixels that took the visible sample | 99.54% | 99.44% |

The S23 visits no more than the Mac does. Over the same circle and window, a node's time is
another matter:

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

The S23's frame ran at 6.9 fps against the Mac's 60, with GPU p50 133.5 ms against 7.7 and
p95 244.0 against 10.0. On the S23 a turn takes under 8 ms and none is long, while 24.4 s over
169 frames is 144 ms a frame, the GPU's time, so the GPU sets the pace. Thermal status stayed
0, with the AP at 44–47 °C once running. At 1.7 million nodes a cast, a frame of 16.7 ms
leaves under 10 ns a node with nothing else in the frame; the S23 takes 111–260 ns.

On the Mac a `sample` node costs a third of a `beam` node; on the S23 it costs four fifths.
`sample` reads what `beam` already wrote to the pyramid, so its visits mostly hit. The reading
was that the S23 pays for what a visit costs before it asks anything, which a hit does not
avoid, and the synthetic pass measured that apart.

Not measured: how a ray's nodes are distributed (only the mean a cast was read), what part of
a node's time is the driver's and Chrome's path, and another Adreno or a Mali.

### What a visit pays

A synthetic pass (`tool/walk/visit.js`, a page of its own with no renderer) takes the real
pass's size: workgroup 8×8, dispatch 28×49, 14 steps, 1.23 million visits over a table of
2^19 entries of 12 words (25 MB), read at a hash of the coarse pixel and the step so that
neighbours share nodes. A visit hits at its first slot in 13 loads, as `sample.gen.wgsl`
does, where every read of the pyramid is an `atomicLoad`, and each thread's hits are read
back. Each variant is warmed for a second and timed by the pass's timestamps over 3 rounds of
12, in ns a visit; the wall clock agrees (`report/visit-*.json`).

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

A second set puts locals that never run behind a uniform that is 0, each assigned in the loop
and read only after it, on a dispatch of 7×12; the full dispatch lost the S23's device at
2,000 of them.

| Locals | S23 | Mac |
| --- | --- | --- |
| 0 | 0.61 | 0.26 |
| 125 | 1.09 | 0.30 |
| 500 | 434 | 0.61 |
| 1,000 | 610 | 1.09 |
| 2,000 | 4,998 | 2.13 |

- **The 33-entry stack costs 20 ns a visit, and 8 entries cost none.** The indexing costs, not
  the fill.
- **All-atomic loads take 6 to 19 times as long as keys-only loads.** The figure moved between
  runs: 8.85 here, 28.63 and, unwarmed, 10.3 in the runs before
  (`report/visit-s23-first.json`). Only its order holds. The two rows of random addresses are
  of that earlier run, the rest of the last.
- **The costs do not add.** Atomic loads with the 33-entry stack took 17.4 ns where plain loads
  with it took 21.4.

### Live words, and the cliff

The locals above are live across the loop, each assigned in it and read after it; `sample`'s
are not. `tool/walk/live.py`, a structured backward liveness over the generated WGSL, counts
function_1 at `d25f5b631`:

| | Vars | At the walk's head | Peak, vars and lets |
| --- | --- | --- | --- |
| `sample` | 1,584 | 16 vars, 150 words (132 the stack), 83 lets | 301 words, line 8461 |
| `miss` | 1,601 | 16 vars, 150 words, 86 lets | 304 words |
| `beam` | 1,460 | 17 vars, 151 words, 85 lets | 327 words |

A let counts as one word here, whatever its type, so the words are a lower bound.

The synthetic pass again, on a dispatch of 7×12, in ns a visit
(`report/visit-*-ladder.json`), with locals live across the loop and, beside them, locals of
`sample`'s shape: declared at the function's top, assigned and read inside the block that
never runs, so that none is live across anything.

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

- **The count of locals costs nothing; live words do.** 6,000 scoped locals cost the S23
  nothing. With words live across the loop, the S23 holds its time up to 150, rises at 175,
  refuses the pipeline at 200 and takes 80–240 ns past it. The reading is that this is what
  fits in registers.
- **`sample`'s peak of 301 words lies past that cliff, and the synthetic pass at 300 takes
  102.5 ns against the real pass's 111.** This is a likeness, not a proof: the peak is one point
  in the code, not the whole loop, and a let may hold more than a word.
- **The refusal is the shaders' driver error.** `VK_ERROR_UNKNOWN` in
  `vkCreateComputePipelines` at 200 live words is what
  [the shaders on two phones](../20261103-the-shaders-by-hand/README.md) met, there without
  a cause in the WGSL.

### What the S23 did to its table

With every load atomic on a read-write binding and 250 or more locals live, the S23's pass left
the table changed, though the shader only loads. The whole table read back after
`atomic_rw live 250` has 279 words of 6.3 million that differ from what was written, none of
them 0, the first at word 103,352. The Mac's table is untouched.

From that pass on, in that device, every pass answers as if the table were empty, the plain and
read-only ones too: every thread's hits are 0 and its `a` is 0. 279 words cannot do that, and
the copy of the table still reads the rest whole. A word written by the queue reads back, so the
read is sound. No error came, the device was not lost, and the timestamps ran as before. It took
a new device to come back.

| | S23 | Mac |
| --- | --- | --- |
| Atomic, 125 live | right | right |
| Atomic, 250, 500 or 1,000 live | wrong, and everything after | right |
| Plain read-only, up to 500 live | right | right |
| Atomic, up to 6,000 scoped | not read apart: they ran after the fault | right |

Each of the 6 runs that reached it showed this. A plain read-write binding past the cliff was not
tried. `sample` is an atomic read-write binding with a peak past that count, and that the page
draws says no more than that its reads are not all lost. The fault was not reported to anyone.

Whether the real page does the same was read at `d25f5b631` through a wrapper of
`createBuffer` that finds the pyramid by its size and reads it back between frames
(`tool/walk/pyramid.js`, `tool/walk/cmp3.py`, `report/pyramid-*.json`). Each kept entry was
checked against the code: its slot in the bucket its zoom and node hash to, a node one under the
world's roots, a kind that exists. At the default view, over the circle, and in the drive with the
shadows on, 7 reads a run kept about 54 thousand entries on the Mac and 52 thousand on the S23,
none broken on either. The same node's answer, kind and asset, was compared device against device
for each node two runs both kept, under one key of its zoom, its index and the zooms asked:

| | Nodes in both | Answers differ |
| --- | --- | --- |
| At rest, the Mac / the S23 | 6,009 | 0 |
| At rest, the S23 / the S23 | 6,011 | 2 |
| The circle, the Mac / the Mac | 54,749 | 4 (0.01%) |
| The circle, the S23 / the S23 | 33,758 | 2,283 (6.76%) |
| The circle, the Mac / the S23 | 51,154–65,374 | 1,203–1,225 (1.9–2.4%) |

At rest the S23 answers as the Mac does. Moving, it differs from itself by more than from the
Mac, at the finest zooms: a slow phone's kept answers are older, of what had arrived by then. So
no sign shows that the page suffers the table's fault. This reads what the pyramid keeps, at one
place, and only the fault's effect on what is kept; the run marked S23 / S23 lost its last reads
to a run that stopped early. The page's other buffers were not read.

### The app against the page

The Android app at `e8d616ba4` on the S23, a Release build, ran against the page at the same
commit, one load each, at the same place with the same motion. The app runs wgpu and naga on
Vulkan, with no browser. `tool/walk/app-probe.patch` is what the scratch build changed: the
probe, a fixed position, the kernel's and the shell's dev settings, and the surface's alpha
below (`report/app-s23.txt`).

| Shadows off, 25 s circle | Page | App |
| --- | --- | --- |
| fps | 16.4 | 14.8 |
| GPU p50 / p95, ms | 45.0 / 146.7 | 31.5 / 229.2 |
| `sample` p95 | 42.9 | 95.6 |
| `beam` p95 | 42.3 | 60.0 |
| `miss` p95 | 36.8 | 51.6 |
| `generate` p95 | 24.8 | 26.1 |
| `world` p95 | 13.1 | 8.7 |
| Drawn | 2.1 Mpx | 2.5 Mpx |

| Shadows on | Page | App |
| --- | --- | --- |
| 25 s circle, fps | 5.0 | 4.3 |
| GPU p50 / p95, ms | 171 / 450 | 212 / 465 |
| Drive, casts a second | 2.5 | 1.7 |

- **The app is not faster, so the browser is not what makes the phone slow.** The app draws 19%
  more pixels, and its frames were paced at about 25 ms against the page's 16.6.
- **The app did not start on this phone without a change.**
  `xpute/rust/gpu/src/host/surface.rs:147` asked for `Opaque` where the S23's Vulkan surface
  offers only `Inherit`, and `Surface::configure` stopped the kernel with `ENOTRECOVERABLE`.
  Since `432295c304` a surface that offers only `Inherit` configures.
- The app's picture 3 s after the circle shows the place with no tree in view, where the page
  has trees at about the same view. This was not followed.
- The app's world showed at 24 s and the page's at 16 s; it was the app's first run after an
  install.

## The eye's walks by hand

From 11-03 the eye's walks were written by hand, a pass at a time, on the branch `walk-visit`.
Each hand pass reads the pyramid alone, read-only, and the generated pass walks only what the
hand pass leaves. The runs are the S23's over the circle, `ray_counts`' lanes counting what is
left, each against the commit before.

### The short stack

`05e8ae4a5`, step 1a: a walk with no line holds 8 entries where it held 33. In
`sample.gen.wgsl` that makes 2,201 `var`s and a peak of 207 live words where it was 301, with
58 words at the walk's head where there were 150; `beam`, which keeps its 33 entries, peaks at
311. The matched circle ran on the same dev build's profile, the phone cooler at the start
(`report/s23-step-1a.json` against `report/s23-matched.json`):

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

- **`sample` did not move.** At 207 words it still lies past the synthetic pass's 175.
- `beam` should not have moved and fell 15–20%, so the frame's gain is not all the stack's.
  `miss`, also a walk with no line, may have gained.
- It draws on both devices, and the S23 makes its pipelines. Its picture was not compared with
  `d25f5b631`'s, and no Mac delta is given (the displays differed).

The short stack went at `94d6e9284`, once `sample` walked only what the hand pass left.

### `sample_hit`

`749560da1`: `sample_hit.wgsl`, 367 lines by hand, runs before `sample_cs` over the pyramid
alone, read-only, with no atomics, no stack and no `world::node`. What it cannot end it leaves
to `sample_cs`, which walks it from the start. Its live words (`tool/walk/livehand.py`) peak at
47 in its entry, its callees' own not counted, against `sample`'s 301. The matched circle, two
windows each (`report/s23-hand-kernel.json`):

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
- **The hand pass takes about 2.4 ns a node on the S23**, against the old walk's 111: p95 2.4 ms
  over the 1.02 million nodes it would leave ended, which is the baseline's count less what is
  left, so an estimate. The Mac's `sample_hit` takes 1.2 ms over about 1.21 million, about
  1.0 ns. The ratio a node falls from 62 to about 2, where the passes that walk no tree stand.
  The pass walks hits alone, so a miss's cost is not in that figure, and the whole walk under
  the cliff was not measured.
- **What stays is what remains in the big shader.** `ray_counts`' `sample` lane now counts the
  left rays' walks alone: 103,494 nodes a cast on the S23, 9.2% of the baseline, and 11,851 on
  the Mac, 1.0%. 54.2 ms over those is 524 ns a node, far past the 111: `sample` pays to be
  entered and pays more on a left ray (the next part).
- **The picture** on the Mac at rest, sky fixed, against `d25f5b631`, is by eye the same: 58% of
  pixels differ, 8.7% by over 2 of 255, 0.38% by over 8 outside the companion, 0.0001% by over
  32. A frame against itself in one run differs only on the companion. How much of that is run
  to run was not told: a second run opened on another display.

`auto` and the drive at this commit are under "Pacing and rest" and "The light". On 11-03
nothing checked that the hand walk answers as `cast::walk` does, ray by ray: the walk was
written twice, and only this picture held the two together.

### What `sample` pays to be entered

`94d6e9284` runs `sample_cs` by an indirect dispatch over only what `sample_hit` left, so that
against `749560da1` the change is the cost of being entered; the rule for what is left is the
same (`tool/walk/run6.js`, `report/s23-left-only.json`, `report/mac-left-by-speed.json`). A
turn of the circle in seconds, the Mac at `749560da1`, the S23 at `94d6e9284`:

| | Turn | fps | Left, nodes a cast | `sample` p95 | `miss` p95 | `beam` p95 |
| --- | --- | --- | --- | --- | --- | --- |
| Mac | 6 | 60 | 10,654 | 1.4–1.7 | 1.7 | 2.2 |
| Mac | 1.5 | 60 | 38,743 | 1.7–1.8 | 1.6 | 2.8 |
| S23 | 6 | 13.3–13.9 | 90,082 | 33.5–39.6 | 58.9–69.7 | 39.5–40.2 |
| S23 | 24 | 12.4–17.0 | 47,471 | 10.8–15.9 | 37.6–60.3 | 31.6–38.4 |

- **Being entered costs about 14 ms of the 47–54.** Every thread kept out of `sample_cs` took
  it to 33.5–39.6, and the frame gained no fps: 13.9 against 14.0–14.6.
- **What is left is walked at 230–440 ns a node**, past the old walk's 111, as `sample`'s p95
  over the left lane, a comparison. A left ray asks what no slot keeps, so it pays
  `world::node`, where the old figure was mostly hits.
- **A faster camera leaves more, and that is about half of why the S23 leaves 2–4 times what
  the Mac does.** A quarter of the turn leaves 3.6 times as many on the Mac and 1.9 times on the
  S23. The S23 at 14 fps and a 6 s turn moves as far a frame as the Mac at 60 and 1.5 s, and
  leaves 2.3 times as many; at 17 fps and 24 s it moves about as the Mac at 6 s, and leaves 4.5
  times. The rest was not traced.

### `miss` by hand

`e7757099f`: `miss_hit_cs` runs ahead of `miss_list_cs` over the same marked list, the pyramid
read-only, with atomics only on its own 8-word buffer; `walk.wgsl` holds the walk it shares with
`sample_hit`. The span `miss` covers both passes, and `ray_counts`' lane what is left
(`report/s23-miss-by-hand.json`). The S23, a 6 s turn, four windows:

| | `94d6e9284` | `e7757099f` |
| --- | --- | --- |
| fps | 13.3–13.9 | 15.5–16.7 |
| GPU p50, ms | 51.9, 69.7 | 50.6–55.7 |
| GPU p95, ms | 151 | 107–129 |
| interval p95, ms | not read | 217–257 |
| `miss` p95 | 58.9–69.7 | 21.2–23.4 |
| `miss` left, nodes a cast | 243,433 | 26,541–26,630 |
| `sample` p95 | 33.5–39.6 | 29.0–39.1 |
| `beam` p95 | 39.5–40.2 | 38.9–41.4 |
| `generate` p95 | 1.0–12.2 | 8.3–13.8 |

- **It makes its pipelines and draws.** At rest on the S23, 45 s after the world, the picture is
  the ground and trees as on the Mac, by eye. No flat ground was seen; none was looked for before
  45 s.
- **A tenth of what `miss` walked is left**, and `miss` falls to a third.
- **The frame is still bimodal**: an interval's p50 is 20–21 ms and its p95 over 217. `beam` and
  `sample` hold the frames that run them.

### `beam` by hand

`8ac7315b1`: `beam_hit_cs` runs over the pyramid read-only and writes each node's `Inheritance`
straight to its line, with no stack. What it leaves, `beam_cs` walks by indirect dispatch, with
the 33 entries, the atomics and every write to the pyramid. The span `beam` covers both, and
`ray_counts`' lane what is left (`report/s23-beam-by-hand.json`). The S23, a 6 s turn, four
windows:

| | `e7757099f` | `8ac7315b1` |
| --- | --- | --- |
| fps | 15.5–16.7 | 18.2–21.5 |
| interval p50, ms | 20–21 | 17–18 |
| interval p95, ms | 217–257 | 186–233 |
| GPU p50, ms | 50.6–55.7 | 29.8–39.1 |
| GPU p95, ms | 107–129 | 107–129 |
| `beam` p95 | 38.9–41.4 | 43.3–46.9 |
| `beam` left, nodes a cast | 308,592–319,008 (all) | 123,299–154,405 |
| `sample` p95 | 29.0–39.1 | 26.5–33.8 |
| `sample` left | 74,777–80,726 | 52,815–65,051 |
| `miss` p95 | 21.2–23.4 | 22.9–29.3 |
| `face` p95 | 4.5–4.7 | 12.9–14.1 |

- **The median frame gains, and the worst does not.** GPU p50 falls by a third, and p95 stands.
  `beam`'s p95 rose: 39–48% of its nodes are left, every write to the pyramid is in the second
  pass, and a frame that meets many new nodes pays both passes.
- **`face` read 4.6 before and 13 here** because of the run, as a later run of this code shows
  (under Noise).
- **It draws.** No view moved unasked in these windows (16 u, polar 60).

The picture against `d25f5b631`, on the S23 at rest, sky fixed, two runs of each
(`tool/walk/shotcmp.py`, the companion left out):

| | Differ | Over 2 | Over 8 | Over 32 | Mean |
| --- | --- | --- | --- | --- | --- |
| One run, two frames | 0.027% | 0.027% | 0.025% | 0.019% | 0.023 |
| `8ac7315b1`, run a / run b | 37.6% | 1.86% | 0.15% | 0.010% | 0.48 |
| `d25f5b631` b / `8ac7315b1` b | 36.2% | 1.90% | 0.10% | 0.013% | 0.48 |
| `d25f5b631` a / `d25f5b631` b | 33.4% | 4.47% | 3.78% | 3.05% | 3.89 |
| `d25f5b631` a / `8ac7315b1` a | 42.0% | 5.90% | 3.83% | 3.04% | 4.02 |

`d25f5b631`'s run b differs from the hand build as little as the hand build's two runs differ
from each other. Its run a differs from everything by the same amount, over the top of the
picture; that is not the hand walk's doing, and it was not traced. This holds at one place, with
the shadows off, and ray by ray is still unchecked.

`face` at `8ac7315b1` drew 4,216–4,467 boxes a cast against `e7757099f`'s 4,587–5,209, so its
cost of 13 against 4.6 ms is not more boxes. Its hits and asks were not counted.

### A ray resumes where it was left

These ran on main after rust-gpu went, each a fresh load alternated with its base.

- **A ray left by the cache walk resumes where it was left** (`a9d6d92d37`, kept): `beam_cs`
  4.5–5.3 to 1.8–2.6 ms. The `beam` span had held three passes, so its ns a node read no better
  than rust-gpu's; split, it showed `beam_cs` walking every left ray again from the root. The
  frame's busy fell less than the pass did, by 0.5 to 2 ms.
- **Every asking walk resumes** (`2135d70883`, kept): `sample` nodes 47–56k to 25–27k (p50 2.1
  to 1.2 ms), `miss` 17–21k to 5–9k. In the first build `beam` rose 40%, since its blind walks
  went back to the root. Resuming a blind walk is exact, since a split's box joins all that its
  descendants answer, so the restart went and `beam`'s visits are main's.
- **The cold start after a view jump stays the refill itself.** With the asking resume, one
  fresh-load drive a build with ray counts from the start: the first casts' `miss` peak
  fell from 74 to 26 ms, nodes a walked pixel from 11–28 to 6–8, and 1.1 M to 0.5 M nodes a
  cast. What is left is the refill, about 70k pixels and about 250k nodes asked a frame for
  about 0.5 s, spread over `miss`, `sample` and `beam`.

**A static read of every pass, ranked by the S23's time** (three readers over `a9d6d92d37`,
the top items checked in the code), listed 16 items. Of them the bucket scan, the taps (both
under "What scales with the pixels"), the asking walks' resume and the light's footprint landed.
It also showed that `px_ray` does not walk, which the bisection had misread.

## Why rays are left, and what the cache forgets

A left ray is one the hand pass cannot end from the pyramid. Reading why found nodes never
asked, nodes held at other zooms than the ones asked, and the pyramid forgetting whole chains
over a moved brick.

### Where a left ray first stops

`8ac7315b1`, instrumented in a scratch tree (`tool/walk/left-instrument.patch`, none of it on
the branch): at the node a ray first stopped on, the hand pass records the node and why no slot
served it. `tool/walk/left.js` reads that and the pyramid on two consecutive frames while the
camera circles, 10 frames on the S23 and 6 on the Mac (`report/left-*.json`). The copies are
25 MB and slow what they read, so only counts are taken from them.

| | Left rays | Distinct first stops, summed by frame | Rays a node | Other zooms | Absent, room in the bucket | Absent, bucket full |
| --- | --- | --- | --- | --- | --- | --- |
| S23 `beam` | 10,495 | 366 | 28.7 | 5,281 | 5,207 | 7 |
| S23 `sample` | 9,341 | 958 | 9.8 | 444 | 8,790 | 107 |
| S23 `miss` | 1,894 | 131 | 14.5 | 72 | 1,802 | 20 |
| Mac `beam` | 1,547 | 144 | 10.7 | 1,365 | 182 | 0 |
| Mac `sample` | 1,239 | 181 | 6.8 | 51 | 1,183 | 5 |
| Mac `miss` | 617 | 44 | 14.0 | 24 | 593 | 0 |

- **Few nodes hold many rays**, 10 to 29 a node, so asking each node once is worth weighing.
  These are first stops only: a ray served at one may stop again deeper, so this bounds one
  round of a queue, not all of them.
- **Half of `beam`'s left rays are zooms changed.** The node is kept, but at other asked zooms:
  half of the left beams on the S23 and 88% on the Mac, mostly at zooms 22–24, the nodes near the
  eye. `sample`'s and `miss`'s are nearly all nodes never asked.
- Eviction is small: 134 of 15,933 absent nodes met a full bucket. Forgetting also read small
  here, 98 of them held the frame before, but this read missed the frames that forget
  ("Forgetting stops a whole frame" below).
- **The left rays come in bursts.** On the S23 one frame of the ten left 5,122 beams to 115
  nodes, and two left none.

### What "other zooms" is

`2a9b235f3`: the hand walk takes a slot held from a coarser `far` for the same `near`, and
`beam_cs` keeps the exact key (`report/s23-rekey.json`). The S23, a 6 s turn, four windows,
against `8ac7315b1`:

| | `8ac7315b1` | `2a9b235f3` |
| --- | --- | --- |
| `beam` left, nodes a cast | 123,299–154,405 | 121,105–135,362 |
| `beam` p95 | 43.3–46.9 | 43.2–48.8 |
| `face` p95 | 12.9–14.1 | 4.6–5.6 |
| fps | 18.2–21.5 | 18.4–21.2 |
| GPU p50, ms | 29.8–39.1 | 32.4–40.3 |
| GPU p95, ms | 107–129 | 95–129 |
| interval p95, ms | 186–233 | 175–225 |
| `world` p95 | 8.7 | 10.0–11.5 |

`beam` did not move. `face` came back to its old value with no fewer left beams, and that was the
run, not this change (under Noise). How
many of the "other zooms" differ in `far` alone was then counted with the misses instrumented as
before (`tool/walk/left-instrument-split.patch`, `report/left-split-*.json`), the left beams'
reason split by how the slot held differs from what was asked, as held minus asked. The walk
takes a coarser `far` already, so what differs in `far` alone here is a finer one held. 14 frames
on the S23, 8 on the Mac:

| | Left beams | Distinct first stops | `near` alone | `far` alone | Both | Absent |
| --- | --- | --- | --- | --- | --- | --- |
| S23 | 16,068 | 342 | 3,065 | 429 | 11,349 | 1,225 |
| Mac | 5,171 | 151 | 4,690 | 232 | 27 | 222 |

- **`near` decides.** On the Mac 91% of left beams are held at another `near`, 4,238 of them one
  coarser than asked with `far` the same.
- **The S23's bursts are one node.** 10,540 were held with both 2 coarser than asked, in two
  frames of the 14: 6,848 left beams to 7 nodes, 6,366 of them at one node of zoom 21, and 4,838
  to 109 nodes, 4,171 at zoom 22. These nodes lie high on nearly every beam's way, so one key
  stops most of a frame. Four frames left none.
- `sample` and `miss` stay nodes never asked: 23,869 of 25,161 and 3,466 of 3,514 on the S23.

The node that stops a frame was then read at `7bf251816`, a first stop's distances against its
side (`report/left-near-s23.json`). Two frames of 16 left 6,848 beams, every one of the grid's
corners, to one node: zoom 17, side 128 u, the eye inside it (near 0), its far corner at 192 u,
asked [23, 27] and held [22, 26]. Other bursts are alike: a node of the chain over the eye, with
near or far on a boundary of `zoom_at` (12, 24, 48, 96 or 192 u).

The picture of `2a9b235f3`, on the S23 at rest, sky fixed, the origin's storage cleared before
each load and the camera logged, at the default view and two views zoomed out (93 u, 541 u), two
runs of each commit, as the mean difference of a pixel with the companion left out:

| | Default | 93 u | 541 u |
| --- | --- | --- | --- |
| `d25f5b631`, run a / run b | 1.68 | 4.13 | 2.08 |
| `8ac7315b1`, run a / run b | 6.69 | 5.00 | 4.60 |
| `d25f5b631` a / `2a9b235f3` a | 1.87 | 0.66 | 0.42 |
| `d25f5b631` b / `2a9b235f3` a | 0.66 | 4.06 | 2.14 |
| `8ac7315b1` b / `2a9b235f3` a | 2.27 | 1.13 | 1.31 |

The commits differ no more than a commit differs from itself between runs. That noise is large,
from what is still arriving, the toast and the companion, and it would hide a small change. By
eye the three are the same, with no darker or bluer picture. A moving view at a boundary of
detail was not compared, nor `2a9b235f3`'s run b, which opened at 20.2 u, polar 56.3, azimuth
−15.5 after the user touched the phone; the Mac's like runs were not the hand walk's.

### A Split held at other zooms

`2f7d18b37` passes through a Split held at other zooms; it ran against `1af780b72` in one
sitting, by turns (`tool/walk/ab2.sh`, `tool/walk/ab3.sh`):

| | `1af780b72` | `2f7d18b37` |
| --- | --- | --- |
| fps | 15.1–17.4 | 15.2–17.2 |
| GPU p95, ms | 141–179 | 147–180 |
| `beam` p95 | 56.5–64.9 | 59.7–66.1 |
| `beam` left, mean a cast | 150–206 thousand | 109–122 thousand |
| frames left over 250 thousand | 32–43% | 14–20% |
| most left in a frame | 439–450 thousand | 430–432 thousand |
| frame busy p50, p95, ms (with counts) | 32.5–54.0, 128–158 | 33.0–35.3, 130–161 |

It halves the frames that leave most, and the worst frame and the tail stand. What those frames
stopped on afterwards was not read.

The picture at rest, sky fixed, two runs each: `1af780b72` a against `2f7d18b37` a differ in 8.0%
of pixels, 0.04% over 8, mean 0.10; `1af780b72` a against `2f7d18b37` b at a mean of 0.41, and the
child's two runs at 0.37. The parent's run b differs from all at 8.9: three near trees were absent
and the toast "동행을 준비하고 있습니다" was still up at 70 s, so a slow load, no commit's. The
"moved" view turned only 0.1 degrees, so a moving view at a boundary of detail remains uncompared.

### Forgetting stops a whole frame

The reads of "Where a left ray first stops" and "How deep a queue goes" took the second of two
frames after the stall of a 25 MB copy, and missed most of this. It was read instead without that
copy, the record zones as fast as they come, one distinct frame in about 3.5, with each frame's
head beside them (`tool/walk/left4.js` with `MODE=scan`, `tool/walk/left-why.patch`,
`report/scan-s23.json`): `2f7d18b37` on the S23, a 6 s turn, 328 frames.

| Forgets | Moved bricks | Boxes, writes | Beam rays left | Frames |
| --- | --- | --- | --- | --- |
| `FORGETS_MOVED` | yes | none | all 6,848 | 29 |
| none | none | none | all 6,848 | 1 |
| none | none | none | over 1,500 | 116 |
| none | none | none | up to 1,500 | 182 |

- **Every frame that moved a brick left the whole grid**, and all but one whole-grid frame moved
  a brick. In the earlier scan 44 of 47 whole-grid frames stopped on one node, the root, absent:
  `pyramid::forgets` drops every node over a moved brick, and a node as wide as a clip level
  "reads every entry of the clip level", so any brick takes the root and the chain.
- The bricks are few, 3 to 40 bits a frame, and no box or write was involved.
- The other whole-grid frames stopped on a node holding the eye, held as a Cell at coarser zooms
  (z22, side 4, asked [27, 27], held [25, 26]).
- Rays leave by finding no slot almost alone: 695,752 against 169 narrow, and none at a Lacks
  under a blind split or by a reach.
- `sample` and `miss` rise with `beam` (correlation 0.64 and 0.56), on nodes absent, not
  re-keyed.
- Beam rays left a frame: p50 1,263, p90 6,386, p95 6,848.

**A moved brick leaves a split** (`9029f35dc`, `b7457102e`'s fix on the rebase over main at
`430cdc831`): on a moved brick a Split's key is left as held by no zooms, not 0. The tip
`981947702` ran against the tip with only the forget pass of before, two loads each, counts off,
no storage cleared but the camera's:

| | Without | With |
| --- | --- | --- |
| fps | 20.9, 21.4 | 22.6, 20.5 |
| GPU p50, ms | 24.3, 25.0 | 31.7, 27.2 |
| GPU p90, ms | 97.3, 103.4 | 74.3, 105.3 |
| GPU p95, ms | 149.5, 141.8 | 97.6, 140.8 |
| GPU most, ms | 377, 315 | 233, 227 |
| interval p95, ms | 174, 187 | 174, 198 |

Frame by frame with it (`report/scan-forget-s23.json`, 153 frames), 1 frame left the whole grid,
where 30 of 328 had before the fix on the earlier base. Of the 31 frames that moved a brick, 13
left few rays and 18 part of the grid, none all. The parent's own scan did not draw, so that
comparison is across bases. What stays is 71 frames over 1,500 beam rays, nearly all stopped on a
Cell near the eye at other zooms (zoom 20–23). The picture of the fix at rest on the S23, sky
fixed, one run each, differs by a mean of 0.54 at the default view, 0.54 at 93 u and 0.30 at
541 u, 0.01–0.02% of pixels over 32: no more than a commit differs from itself between runs.

### What a Cell held at other zooms answers when asked again

`981947702`, instrumented (`tool/walk/left-cell.patch`, `tool/walk/left5.js`,
`report/cell-s23.json`), records for each node a beam ray stopped on that was kept at other
zooms its slot's kind and asset before `beam_cs`, and then the slot the frame wrote at the zooms
asked. 285 frames were read whole, so each follows the stall of a 25 MB copy: 8,220 nodes held as
a Cell and asked again, on 520,417 rays.

| Held near, against asked | Held | Nodes | Rays | Nodes changed | Rays on them |
| --- | --- | --- | --- | --- | --- |
| coarser | air | 1,830 | 249,014 | 17 | 199 |
| coarser | filled | 546 | 11,911 | 13 | 22 |
| the same | air | 2,214 | 167,410 | 2 | 46 |
| the same | filled | 865 | 12,872 | 0 | 0 |
| finer | air | 2,178 | 70,531 | 6 | 16 |
| finer | filled | 587 | 8,679 | 0 | 0 |

- 99.5% of nodes answer the same words: air to air 6,197 nodes on 486,694 rays, 94% of those
  compared, and filled to filled 1,985.
- 38 nodes on 283 rays changed. Held `near` one coarser: air to a split 17, filled to a split 10,
  filled to air 3. Held `near` one finer: air to filled 6. `far` one finer with `near` the same:
  air to a split 2.
- So a node asked finer can turn air into a split. Those nodes sit on a boundary of `zoom_at`,
  near 96 or 192 u in 33 of the 38, at zooms 21–23 in 36.
- 118 more held air had no slot at the asked zooms when read, and were not compared.

Since 11-06 the node cache keeps a cell's answer once, whatever zooms it is asked at, and a
split's at the pair asked (`42d3340754`).

### Far views: the cache forgot what aliased it

With `forget-bricks.patch` (held first for the Mac's GPU tests and on main by `3d9521137f`;
the S23 at dawn, shadows on, a 6 s circle at fixed zooms, two runs a side), far views' cost
was found to be the cache forgetting. A node's bricks were tested by entry bit modulo `ACROSS`,
so a moving fine level dropped nodes that alias it anywhere in the world. A cache 4 times larger
changed nothing, and no forgetting at all took the nodes asked from 215k to 3k. Each level now
hands the box its moved bricks lie in. At 11,761 u the nodes asked fell from 215k to 22–37k,
`sample` from 15 to 2 ms and busy from 58 to 41–47; at 131,072 u from 90k to 10k, busy 40 to
29–33. The default view is unchanged.

The tests hold the cache's requirement, not its route (`b26607770f`, −7445 lines): what pinned
visits, floors, the cache's slots, claims, layouts and pass splits went, and an edited brick that
is never answered as it was is held end to end; that test fails with forgetting off.

## What is left: the queue, the holes and a ray that ends in its frame

### What a frame pays for what it leaves

`7bf251816` runs the passes of `8ac7315b1`. The profiler's rows put each frame's GPU time and
spans beside its `ray_counts` lanes (`report/s23-frame.json`, `tool/walk/tail.py`): 149
frames with both, a 6 s turn, `ray_counts` on.

| Left nodes, all passes | Frames | GPU busy, ms | `beam` | `sample` | `miss` |
| --- | --- | --- | --- | --- | --- |
| under 20 thousand | 18 | 31.1 | 4.8 | 0.6 | 1.3 |
| 20–100 thousand | 33 | 40.4 | 11.2 | 1.9 | 2.6 |
| 100–250 thousand | 24 | 51.8 | 24.6 | 1.2 | 1.8 |
| 250–500 thousand | 43 | 88.9 | 43.6 | 9.1 | 7.1 |
| over 500 thousand | 31 | 157.2 | 57.1 | 40.2 | 28.7 |

**The tail is the left nodes.** GPU busy fits all left nodes as 32.4 ms plus 150 ns a node, at a
correlation of 0.96. By pass, at no node left and a left node's cost: `beam` 4.1 ms and 139 ns
(0.96), `sample` 1.8 and 113 (0.99), `miss` 2.9 and 213 (0.94). This ties the tail to the bursts
of left rays above, which the first reads had left as a reading.

The floor of 32 ms is high because `ray_counts` costs. Every span of each frame, in one sitting,
mean ms (`report/s23-frames-all-spans*.json`):

| | Frames | Busy | `world` | `beam` | `px` | `face` | `finish` | `sraster` | `generate` | `resolve` | `miss` | `sample_hit` | `sample` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Counts on, under 20 thousand left | 23 | 28.7 | 8.98 | 4.74 | 4.14 | 2.45 | 1.50 | 1.35 | 1.40 | 1.04 | 1.31 | 0.85 | 0.53 |
| Counts off, the fastest quarter that cast | 146 | 20.4 | 5.67 | 4.36 | 1.32 | 1.96 | 1.46 | 1.31 | 1.31 | 0.97 | 0.84 | 0.77 | 0.06 |
| Counts off, every frame | 631 | 50.6 | 5.31 | 17.79 | 1.37 | 4.58 | 1.46 | 1.42 | 1.47 | 0.94 | 7.91 | 1.16 | 6.65 |
| Counts off, frames that did not cast | 50 | 7.9 | 2.59 | | 1.64 | | 1.30 | 0.95 | | 0.50 | 0.79 | | |

A frame that casts with nothing left takes about 20 ms, not 32: `ray_counts` itself costs `px`
3 ms and more. The spans sum to the busy time, so no GPU work is unaccounted. Over every frame
the p50 is 31.4 ms and the mean 50.6.

### How deep a queue goes

`tool/walk/left.js` with `MODE=depth`, with no shader change, counts a frame's entries that the
pyramid tags with that frame and pass, each one deeper than its parent if the parent is new too.
The depth is the round of a queue that asks one layer a round (`report/s23-depth.json`). 12
frames on the S23:

| | Nodes in `beam` | Rounds | Nodes by round |
| --- | --- | --- | --- |
| Most frames | 5–892 | 1–8 | most in the first two or three |
| Two frames of the 12 | 2,113, 2,228 | 19 | 100–128 in the first, a few each to the 14th, then 118, ~550, ~340, ~480, ~470 |

`sample` goes at most 4 rounds, and `miss` 1 or 2 where read. Only what is kept is counted, so an
opaque cell that a beam does not keep is not among them, and these figures bound from below.

### A queue

`3e7ba241d` ran against `7bf251816` in one sitting by turns (`tool/walk/ab2.sh`,
`tool/walk/ab3.sh`): each left beam's first stop is asked
once, and then the hand walk runs again. It left 115 thousand nodes a cast against 155–166
thousand, but it took `beam` from 44–60 ms to 88–99 and GPU p95 from 108–167 to 175–217, at
13.0–17.2 fps against 15.8–23.4. It was reverted. `ask.gen.wgsl` peaks at 95 live words, so the
cliff is not why.

A queue that asks each node once came back on 11-06 as the holes' queue (`9f116e5f34`).

### The batch of 11-06

The batch of 11-06 drew the kept voxel records every frame, asked through one budgeted holes'
queue, woke owed work by readback and rolled the lattice. It followed an agents' plan, not a
decision, which lay under `out/` until [a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md)
moved it beside these runs; its decisions are
[the frame is what it must draw](../../minutes/20261106-the-frame-is-what-it-must-draw.md). In
that plan a change landed on its contract argument and the static gates, the GPU suite ran under
the lock, and the device ran hot once a batch. Its work in flight on the evening of 11-06:

| work | owner |
| --- | --- |
| stale entries as holes; eyed stale box not drawn; the horizon line's cause; dark squares | stale agent |
| light: own spans, incremental stand/propagate, receivers by stood tiles, pending ≠ owed, seen store once, counts per observer | rest agent |
| whole-frame harness under validation | harness agent |
| declared access + build-time hazard check | access agent |
| Phase 2: drift a frame word (first), draws as changes, bind once a pass | pixelet-b9 |

Its next steps were: (1) one table, with the band out of the key, one record and the drawable
state in the node cache's entries, waiting on the stale and rest batches; (2) frame kinds and
owed and rest as a pure state machine explored whole; (3) the rolled lattice's beams and samples
into the queue, with per-region discovery done; (4) the S23 hot at the clock floor, and Safari
with shadows on; (5) the push.

By 11-07 these were done: the stale entries and the eyed stale box (`08bb506031`), the light's
items (`c3da6cc3bf`), the whole-frame harness (`45284116f3`), declared access and the build-time
hazard check (`29095723c6`), Phase 2 (`d5a5ebdb2f`, `22292a6d0c`), Next 1 (`00834fb890`,
`42d3340754`), Next 2 (`65892d6015`), the hot S23 at its clock floor (hot2 under "On a hot
phone", 348 MHz) and the push. Next 3 was built (`ecddf34256`) and then replaced: the rolled
lattice and the visit budget went on 11-07. The horizon line's cause and the dark squares stayed
open ("The picture"), and so did Safari with shadows on until 11-08 ("Memory, submits and
Safari").

On the hot phone (hot3, `034ddad063`, 550 MHz, ray counts on, the drive's last minute) the batch
counted, per cast:

| | beam | sample | miss | asked of the world |
| --- | --- | --- | --- | --- |
| eye | 220k | 278k | 21k | 25k |
| light | 177k | 191k | 193k | 31k |

The eye reused 74% and walked 5%; the light cast again only 5% of its rays. The frames at or
past the interval's p95 (315 ms) walked 125k eye nodes on average against 15k for the rest, and
asked fewer, 1.3k against 5.6k. The light's asks follow busy at a correlation of 0.43. The
profiler's frames give no owed count, so the owed queue itself was not visible. In the earlier
hot runs at `ef8d1621b7` ray counts were off in the drive, so no spike was tied to the queue's
asks.

### Every ray ends in its run

On 11-07 ([a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md)) these
went: the visit budget and its caps, the rolled lattice, every ray parked across frames and the
owed words, bits and pools it took, the readback that woke frames for them, and the light's
claim budget, places, rings and carried rays. Every ray ends in its run, and a ray with no room
or asks left ends as a lack. `Counter` keeps only the `ray_counts` setting's copy. The S23 and
the gate against the deployed build are under "Against the deployed build".

- **The eye's holes, one lane a word, were the cost; they became a list, a lane a hole** (kept).
  It was found in the code first: a lane took a word of hole bits and walked its up to 32 holes
  one after another, so holes that lay together kept a 32nd of the lanes busy, about 5 times
  short at 100k holes. A list of holes, as the light's, took `miss` p95 from 29 to 6 ms on the
  Mac, GPU from 27.5 / 56.8 to 19.3 / 36.8 ms, and casts in 20 s from 599 to 872, one run each
  side. The list takes a word a drawn pixel, so the most drawn falls to about 14.7 Mpx
  (`DRAWN_AREA`); an 8K canvas is drawn smaller and brought up.
- **The asks a frame takes are 2^21.** At 2^17 a run of the light's holes over a fresh node cache
  still asked past it after 64 frames, and four GPU tests failed on it.
- **The walk's budget atomics cost 0.3–8 ms on the S23** by the count of rounds; a reader had
  first estimated 10–30 ms. The S23's bandwidth had been taken as 25–30 GB/s unchecked; its peak
  is about 67 GB/s.
- **The stops' room, 2^18 against the deployed build's 2^19, closed on 11-08.** It differs only
  in a frame of 2^18 to 2^19 holes. Three 4K drives held 3 such frames of 1107, and 174 passed
  2^19, past either room.

The dark band along the horizon came from the parked beam rays and the holes they left; it is
under "The picture".

## The node cache: atomics, claims and layout

On the S23 an atomic load costs what a plain one does not ("What a visit pays": all-atomic loads
take 6–19 times keys-only). How many of the node cache's reads can be plain was tried in rounds.

- **The first round, dropped**: entries apart from keys, plain tallies, a name column, +2 MiB.
  The cache walks rose 0.2–0.5 ms, and plain tallies read 2–15% of main's counts on Adreno,
  which makes them useless. A person decides by those counts, so they stay atomic. The round's
  −3 fps was run order.
- **The node cache without claims or atomics, dropped**: stamp-checked, plain loads. With
  shadows on, fps fell from 15.8 to 11.7 and busy p95 rose from 112–131 to 212–230 ms. Taking an
  entry only at the zooms asked, which is needed once entries can tear, left light misses at 70
  times the nodes; the atomics saved less than that. Main's claims never tear, so its other-zoom
  shortcuts answer right.
- **One atomic a workgroup for list appends, dropped**: the eye's passes stayed even or rose
  0.1–0.8 ms, and its `sample_hit` rewrite (flags, a barrier) raised `light_sample` 2–5 ms.
  Adreno's per-lane appends were not the cost.
- **The second round, held**: a slot's key atomic beside a plain entry, the tallies atomic and
  gated, the light list, ways, decor and stands plain. With shadows on, `light_beam` fell from
  3.2 to 1.6 ms, `light_walk` p95 from 2.4 to 1.6, busy p95 from 50 to 43, and fps rose from 25
  to 27.6. With shadows off, over three cool pairs, fps went from 43.9 to 44.0, even. Stacked on
  forget-bricks and px-tiles it answers wrong on the Mac: `hand_light_beam_rays_answer_as_cast_walk`
  failed 4 of 18 runs, on the next frame only, with one light beam ray ending one node over;
  without the round, 10 of 10 pass. The plain reads under the claims had been assumed safe, and
  as built they are not. The round is held, and its plain groups were being narrowed one by one.
- **Only the light list and the stands plain** (`atomics-light.patch`, held for the Mac's GPU
  tests): at equal `world`, `light_beam` fell from 7.2 to 4.5 ms throttled and from 3.7 to 2.6
  cool, busy from 62 to 59–60, and nothing else rose. The candidate with the ways, decor and
  exposure plain too read `world` 0.8 ms higher, which reads as warmth; it is unsettled.

**The keys and states apart from the entries** (11-07, kept): dense arrays, with the entry at
44 B where it was 80 B. A walk's hit reads about as many lines as before (2.5 to 2.4) and a miss
1 where it read 4. A pass over every slot reads 17–43 MB where it read 84 MB, two or three such
passes a cast. The expected gain was under 0.5 ms on the Mac at 400 GB/s and 2–5 ms on the S23 at
its peak of 67 GB/s or below. The Mac's gate passed, within the Mac's noise as expected ("Against
the deployed build").

## The light

The light walks the world from the sun with passes of its own, which run only with shadows on.

### The light's walks under rust-gpu and by hand

**The drive at `d25f5b631` is not one cost.** On the S23 two runs gave GPU p50 415.6 and
229.1 ms, the view at 2.0 u and 525.5 u as each was read, and `light_walk` p95 324.3 and
951.3 ms. Asked at its end, the S23's light walked 1,828,794 `miss` nodes a cast at 1,030.6 ms
p95, about 560 ns a node. The Mac's frames at that view stood (`gated`), so the Mac counted none,
and its nodes in the drive were not counted. On the Mac 264 of the drive's 1,491 timed frames
cast; on the S23 all 38 did, so the slow phone does again what the Mac's frames skip. At
`749560da1`, with the eye's `sample` by hand, the drive was untouched at 1.9 fps and `light_walk`
p95 1,028 ms, since the light's walks were still the old shader's.

**The light's missed slots by hand** (`355b65140` against `e8d616ba4`, alternating in one
sitting, shadows on, a 25 s circle at noon, caches kept; `tool/walk/w355b.sh`,
`report/light-miss-by-hand-s23.txt`):

| Run | fps | GPU p50 / p95, ms | `light_walk` p50 / p95, ms | Light `miss` a cast | Eye walked |
| --- | --- | --- | --- | --- | --- |
| `e8d616ba4` | 7.2 | 107 / 363 | 27.3 / 134.1 | 85,535 | 0.49% |
| `355b65140` | 8.8 | 102 / 289 | 6.8 / 28.6 | 96,657 | 0.45% |
| `e8d616ba4` | 7.2 | 114 / 332 | 25.2 / 102.2 | 106,691 | 0.52% |
| `355b65140` | 8.0 | 116 / 279 | 7.8 / 33.6 | 115,293 | 0.49% |

- About the same nodes take about 3.5 times less time, so about 70% of the old `light_walk` was
  the generated shader: roughly 285 ns a node against 75, from p50 and a cast's nodes in
  different frames.
- The eye's `sample`, `beam` and `miss` overlap across the commits. At rest the commits differ
  by a mean of 0.46 and 0.25, and each from itself by 0.30 and 0.26. By eye no color is wrong, at
  phone size.
- With shadows off, a fresh load each in this order, `e8d616ba4` ran 22.7 fps at GPU
  30.7 / 100.7 ms and `355b65140` 18.8 at 38.2 / 133.9, which is not told apart from a sitting's
  drift.
- A cold first load made 91 pipelines, none failed, with no console or GPU error.
  `light_miss_cs` went from 397 KB and 12.5 s to 50 KB and 6.9 s; the wall stays at 13–14 s.

**The light keeps its cells, and the eye does not pay** (`f9f66682e` in runs 1 and 3,
`e8d616ba4`, which keeps them, in runs 2 and 4; alternating in one sitting, shadows on, a 25 s
circle at noon, caches kept; `tool/walk/pair.sh`, `report/light-keeps-s23.txt`):

| Run | fps | GPU p50 / p95, ms | `light_walk` p50 / p95, ms | Eye walked | Eye `miss` a cast |
| --- | --- | --- | --- | --- | --- |
| 1 | 2.6 | 239 / 1,054 | 283 / 831 | 0.64% | 83,933 |
| 2 | 7.0 | 124 / 371 | 20.5 / 105 | 0.49% | 45,525 |
| 3 | 2.2 | 331 / 1,172 | 536 / 918 | 0.60% | 82,128 |
| 4 | 6.6 | 132 / 346 | 24.9 / 111 | 0.42% | 39,459 |

In this view the eye walks less with the light's cells kept. At rest the two commits differ by a
mean of 0.56, and each differs from itself by 0.50 and 0.59. By eye no color is wrong at rest or
right after stopping, at phone size. A long drive, where the pyramid's slots run out, was not run.

**Trees cast at noon.** At `e8d616ba4`, noon, with shadows off, on and off again, no camera
move, 10 s each (`tool/walk/tree.js`), on against off differs by a mean of 1.80 with 5.55% of
pixels over 8, and off against off by 0.40 with 0.14%. The crowns darken the ground and water,
not only walls. An earlier shot at `6e9a7cb83`, taken with no control, had been read as showing
no shadow.

### The light's walks, resumed and split

- **The light's asking walk takes only the slots left** (`d261b5e0ca`, kept): `light_walk` p95
  from 19–20 to 4.5–6.2 ms circling and from 93–103 to 50–54 driving (pinned), for 16 MiB of list
  room at a 2048 map. The resume from the cell alone kept a sixth of the gain; the list was it.
  Tried alone, the misses resumed from their cell took the drive's `light_walk` from 116–125 to
  101, where the build with a list of the slots left reached 11–14; the list was kept.
- **The light's beam split as the eye's** (`880b154d54`, kept): `light_beam` from 7.2 / 13 to
  4.3 / 6.3 ms, busy p95 7 ms lower, and the drive's cast frames' p95 (pinned) from 200 to 141.
  One `beam_hit` and `beam` serve both observers, each its own part.
- **A cell's window band in three evaluations** (`514cda20b3`, kept): the `light_beam` span,
  where `light_cell` runs, fell 1.5 to 2 ms at p50. The moon's fill is gated where its air
  reaches; its direct light is kept, since the bound proves the fill only.

### The receivers

- **`light_receivers` is the stands' stores.** Bisected with scratch variants of `light_mark`:
  the footprint off and a store without its read change nothing, and no stand gives 1.05 ms of
  7–8. Every drawn pixel's thread stores into a few hundred tile words, 2.1 M atomic stores
  serialized.
- **The receivers' stands once a tile a workgroup** (`repeats.patch` on `c83fd6f67b`, the shelf,
  the S23 hot, two runs alternating): `light_receivers` from 14.4 to 4.4 ms with a character
  walking, busy 10 ms lower, shadow edges and exposure the same. A root line shared by every beam
  ray bought nothing (`beam_hit` even) and goes.
- **The stands in workgroup memory, dropped** (`light_propagate`): `light_receivers` stayed
  about 7 ms either way on the S23, for 1 KB and barriers.
- **Unsettled as last written:** `light_receivers` at 6.9–8.1 ms every frame with a character
  walking was the largest light cost, and `receivers.patch` was being measured.
- **The light map drawn through its gate on receivers-only casts, dropped:** `shadow` took
  1.6–2.0 ms in all three builds. The camera follows the character, so the runs with "the
  character walking, the camera still" were full casts, and the gate's lack of gain on
  receivers-only casts is unproven either way.

### What makes the light cast again

**A world change casts again only the light tiles it reaches** (`aa2ee5efef`, kept):
`light_hit` p95 from 17 to 5.2–5.4 ms, `light_walk` from 25–28 to 11.5–15, busy p95 from 112 to
80–88, with a character walking under the circle. It replaces the 11-02 minute by the user's
word. No stale or missing tile was seen at phone size.

**A brick's columns are read by the bands that read it** (11-08, `light::Column`,
`brick_columns`, kept). A tile answers at its band, off the finest clip level at or over the band
that holds the cell present (`clipmap::step`). So a brick at a zoom moves the tiles of its band,
and of the bands finer only where the next zoom's bricks are not present; a coarser band reads
none of it. Before, every band over the brick's column was cast again. The light's GPU tests hold
it: `a_world_change_keeps_the_tiles_it_does_not_reach` with a brick over the leaf's, kept slots'
walks alike, and `hand_slide_forgets_what_a_world_change_reaches` with a column one band reads.
In two runs after it against the one before, the columns' light ms went from 956 to 601 and 847,
and the frames that cast the map whole from 1375 to 833 and 1247 light ms; run to run differs as
much, so the gain is not separated from it. A column now carries its bands, and the head holds
two vec4s a column. A column past the head's 64 is taken into the last, as the draws' stands are,
where before it cast the map whole: one branch and one cause of a whole cast fewer.

**The light map is recast whole on a depth step.** On main at `e50bf2d5ec`, with a depth tag and
two time-swept drives, the light frame's key changes by its depth cell alone (planes snapped by
`DEPTH_STEP`, 0.2 of the box's depth) as the eye moves along a low sun: 45% of light casts start
a new frame so, at 60–64 ms against 49–56, and they take 48–51% of the drive's busy. A filter of
the changed columns to those reaching the map changed nothing and was dropped.

**The depth step slid, held** (`light-depth-slide.patch`, 33 files and a contract change): the
depth step slides as a tile shift does, the ray kept and its window moved, so that a kept texel
equals a fresh cast. At 178 u whole recasts fell from 9–13 to 1–2 a window, but a stepped cast
costs as much as the whole recast it replaces (82.5 ms against 62 unstepped; the base's whole
cast 69–90), so the tail stays. In the time-swept drive whole casts fell from 116 and 109 to 85
and 85, most of the rest cheap far re-anchors past the window (51 ms, walk 0.4), with busy p95
4 and 16 ms lower and fps 0.4 and 2.0 higher. The dear casts left change the light frame's key
without the sun moving (`cast_from` or `half_width`), 34 a drive, all of them `cast_from`: the
sky re-aims the sun from the eye's place as it travels tens of km at the sweep's far end
(`environment.rs` `sky_anchor`), each re-aim easing through several light frames. That is left as
it is, since a user rarely travels so far with shadows on.

**With the claim budget gone on 11-07, an epoch casts every tile it reads in one cast**, and the
S23's p95 became the light's ("Against the deployed build").

**Why the light casts again, 11-08.** Main at `497bdf6062`, the Mac at 4K, `pxl.perf.drive(20)`
with `ray_counts` on, one fresh page a run, each frame's `light_moved` read off the trace. A probe,
not kept, split the light's frame (`Shadow::generation`) into its key's parts and the world's
whole change from its columns. The light's spans, `light_beam_hit` to `light_walk`, summed by why
the frame cast:

| why | frames | light ms | share |
| --- | --- | --- | --- |
| the box's depth stepped (the eye along the light) | 302 | 1129 | 43% |
| bricks and edits, by their columns | 144 | 956 | 37% |
| the box slid, or the eye's zooms stepped | 53 | 321 | 12% |
| tiles read anew | 217 | 192 | 7% |

The line of 11-07 had put the p95 on the epoch with the sun's and the zoom sweep's moves among
its causes. The drive holds the sky at 07:00, the box's width is a constant (`EFFECT_REACH`), and
a step of the eye's zooms is a slide, not an epoch: of 288 epochs in a second run, 261 were the
depth step alone and 27 the depth step with the sun.

**Keeping tiles across a depth step was weighed and dropped.** It was first left open, to be
proposed apart: a slot's word is a share of the box's depth from its near plane, so a box stepped
along the light makes every word another depth and every ray another start, and keeping them
across a step changes "a texel kept is the same ray". Weighed, a kept slot's ray would start a
step from a fresh one's, half the slots end as sky and would owe the strip the step added, and the
tile would have to carry which plane it was cast under.

**What a whole cast walks.** On the Mac's drive, a cast of the 1.9 M slots the receivers read took
8% through the holes' walk (150–170k), 42–45% off the samples' raster and 46–51% as sky. So
`light_hit`, the largest span of the S23's p95, is the holes' first walk over about 150k rays.

On the hot phone, once every submit was valid (hot3, `034ddad063`), the light's beam, sample, walk
and hit passes took about 100 ms of a cast frame, `light_beam` about 80 ms of it ("On a hot
phone"). At rest after that drive, 51 frames of rest2 were light casts that asked zero nodes, with
33k boxes a cast; at rest in the earlier hot runs `light_moved` stayed at about 10 a frame.

### The light's list and its room

**The light's box room, tried and dropped.** `record::LIGHT_CELLS_MOST`, 4096, is the size of
the claim budget that went on 11-07; `LIGHT_DRAWS` is 65,536 boxes off it, and past them a slot
walks. On the Mac's drive 11 of 71 casts over ground reached it, and those walked 220–290k slots
against the usual 170k. With room for every tile, one S23 drive at 719 MHz went from GPU p95 205.6
to 220.7 and `light_hit` from 79.8 to 82.6: no gain, so it was not kept. The stale number stands,
with the node cache's sizing that reads it.

**What the light's list would hold**, from the counts of those casts. The 71 casts over ground
drew 30–38k boxes at the median and 65,536 at the room. They asked the world 110–290k nodes in the
beam, 80k in the samples and 50–80k in the holes, up to 2 M, and of 1.9 M slots took 790–840k off
the raster, walked 146k and left 890k as sky. The boxes are what the sun sees, so a list holds at
least that many; the cells they hide are not counted. At the reach of 96 units and a leaf of 1
unit the read disc is about 29k columns, and the sunward sweep more. That puts a list near 100k
cells here, a tenth of the 1 M that 11-06 assumed. Its time on the S23 was not measured.

**The light's list, measured on the CPU and set aside** (11-08). Each tile's cells at its band,
found by a descent of the query over its texels' sweep, came out exact in 13 of 14 cases, at 5 to
400 times the nodes that the tile's 64 walks ask: the ground under the surface is listed as deep
as the sweep runs ([the light's list](../../minutes/20261108-ray-raster-ray.md)).

**A split's air children (ESVO's child masks), measured and dropped.** What a light's ray asks on
the real-world test data, walked whole from the root on the CPU, 81 rays a case over 14 cases, is
17–50 nodes a ray: 73–95% of them splits, 0–12% air (25% in the case on what is placed) and 0–7%
cells. A mask skips the air alone, past what the split's box already skips, so at most about a
tenth of the visits. It needs 8 bits where a split's words have 4 left, a coarser box for them,
and each split's children asked to fill it. The visits are the descent, not the air. The eye's
rays were not counted, and sibling loads and subgroups were not tried.

## What scales with the pixels: the world pass, `px` and decor

**What runs by what.** `beam` and `sample` dispatch on `Grid::corners` and `Grid::samples`
(`gpu/ray/cast.rs` `dispatch`), from the screen's tangents and the lattice (pxl-engine `beam.rs`
`Grid::of`): an angle, `CELL` = 1/96 rad and `SAMPLE` = 4, whatever the pixels. That is the
contract (`ARCHITECTURE.md`, Drawing). On the S23 at `d25f5b631`, over the circle:

| Scale | Mpx | fps | GPU p50 | `sample` | `beam` | `miss` |
| --- | --- | --- | --- | --- | --- | --- |
| 1.00 | 2.1 | 5.9 | 154.7 | 133.2 | 55.4 | 92.1 |
| 0.71, Auto | 1.1 | 6.2 | 152.1 | 147.3 | 55.5 | 62.9 |
| 0.38, Low | 0.3 | 6.8 | 139.2 | 138.0 | 53.2 | 19.3 |

Seven times fewer pixels took 10% from the frame, and `miss` alone followed them.

**The frame became pixel-bound at full resolution** (main `82efe02944`, `res` fixed at
1080×1969 against low, 0.38 a side, the phone hot at thermal 1, two runs alternating). `world`
fell from 13.7 to 4.5 ms and `light_receivers` from 11.5 to 2.0, and `px`, `sraster`, `resolve`
and `miss` fall with the pixels; `beam_hit`, `beam`, `sample_hit`, `sample` and the light's
walks, which follow the angle lattice, do not. Busy halves, and fps went from 21.6 to 36.4 with
shadows off and from 12.1 to 21.2 with them on. Of a hot shadows-on frame's ~80 ms, ~38 are per
pixel and ~25 the lattice's. No scale between the two can be had without code.

**The air before a point is one trilinear read** (`7f88f6676a`, kept): `world` from 9.6–10.8 to
8.4–8.5 ms. It was bisected on the S23 from `world` to `screen.draw` (85%), to the lighting, to
the air, to the 8 B-spline taps a pixel. The pictures are alike at noon, dawn and dusk, high and
mid, at the horizon: base against change within load-to-load noise, with no band in
contrast-stretched crops. The B-spline came with `aed85fbdd7`'s fix for bands seen from high up;
those bands were the per-froxel march, which stays fixed.

**A bucket scanned once, one texture tap** (`36bd3f9a7f`, kept, from the static read of the
shaders, which found four identical taps in paint): `world` from 9.0–9.1 to 7.1–7.3 ms,
`beam_hit` from 2.3–2.4 to 1.8–2.0, and fps from 31.5–33.2 to 33.3–36.8 with shadows off. The
textured ground is alike.

**A span a draw group in the world pass, dropped.** WebGPU writes timestamps only at a pass's
ends, and a split pass stores and reloads the tiles every frame. The world pass was attributed by
subtraction instead: `px_ray` and `hit_of` take about 0.9 ms, not the 5–6 first read.

**`px` over the claimed tiles** (`px-tiles.patch`, held first for the Mac's GPU tests and on main
by `3d9521137f`): at rest `px` went from 2.29 to 0.07 ms and busy from 10.4 to 8.3; moving frames
are even. On the hot phone of 11-06, before the light ran (hot1 and hot2), `px` was the largest
pass at 37–39% of a cast frame's p50, where it had been about 1–2 ms; it and `miss` follow a
frame's busy (correlation 0.70 and 0.68), and `sample` and `beam` do not (0.14, −0.01). The light's
queue passes took under 1 ms there.

**Decor's model floor** (a diagnostic at rest): models drawn down to 1 camera cell (about 11 px,
"set by eye") cost much of the 3.3 ms; `world` went from 5.24 to 3.41 ms at 2 cells and to 2.69
at 4.

**Lighting not run under decor, not built.** A depth-only screen draw would let decor clear a
stencil bit. The 8 stencil bits are the gate's numbers, so a coverage bit touches every
world-pass pipeline, for an unmeasured 0.3–0.8 ms; the share of screen pixels that decor covers
is to be measured first.

## Pacing and rest

### `auto`: the scale, the lattice and the rate

**A lattice of 2 bought nothing on 11-03.** `auto` on the S23 at `d25f5b631`, each window 25 s, as
the judge moved:

| Window | Scale | Lattice | fps | GPU p50 | `sample` | `beam` | `miss` | `generate` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 0.71 | 1 | 7.6 | 120.5 | 119.5 | 44.4 | 52.5 | 20.9 |
| 1 | 0.50 | 1 | 7.9 | 114.4 | 121.1 | 44.7 | 24.4 | 24.6 |
| 2 | 0.50 | 2 | 7.7 | 107.3 | 170.1 | 22.0 | 26.0 | 30.0 |
| 3 | 0.50 | 2 | 6.9 | 124.6 | 171.1 | 22.0 | 30.5 | 30.3 |
| 4 | 0.50 | 2 | 6.5 | 145.4 | 182.0 | 23.2 | 34.3 | 35.4 |
| 5 | 0.50 | 2 | 6.2 | 144.6 | 189.4 | 24.8 | 27.7 | 37.0 |
| 6 | 0.50 | 2 | 6.2 | 144.4 | 188.8 | 24.7 | 30.7 | 36.2 |

`beam` halves, `sample` grows by 50–70 ms, and the frame gains nothing. The judge, with both
knobs at their floor, holds 6–8 fps. The windows' views differ as the circle turns, and the later
ones ran hot for longer; thermal status stayed 0. Nodes were not counted at a lattice of 2. At
`749560da1`, with `sample` by hand, `auto` settled at a scale of 0.50 and a lattice of 2 at
21–22 fps, `sample` 61–68 ms, `beam` 18, `miss` 28.

**`auto` on the S23 after rust-gpu** (main `97f694bb41`, the shelf, holds of 60 s, one load, thermal
0): with shadows off it ran 54–56 fps at lattice 1; with shadows on 45–48 fps, settled at 0.5 and
lattice 2 within 10 s, softer but clean. With shadows off it swung between 0.84 and 0.71 every
~3 s, 18 changes in 60 s, against the judge's stated contract: the timed branch lacked the untimed
branch's guard. **`auto` now tries a step up on one rule** (`6a9f3a2070`, kept): scale changes
fell from 18–20 a minute to 0–1, fps rose from 54–55 to 56–58, and the interval's p95 fell from
33–36 to 23–30 ms; it settles at 0.5 warm.

On 11-07 `auto` took a third knob, the rate that turns go in at
([a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md)).

**`auto`'s lattice was removed on 11-08.** The backlog's condition was that it goes if it earns
nothing. A coarser lattice moves the descent that a beam ray shares onto the samples under it,
four to a cell or more, so it pays each level on more rays. The S23 at 719 MHz both, one 60 s
drive each at `fixed` with the lattice forced:

| lattice | GPU p50 / p95 | `beam` | `sample` |
| --- | --- | --- | --- |
| 1 | 57.7 / 205.6 | 8.5 | 7.8 |
| 2 | 60.3 / 218.4 | 4.2 | 13.8 |

The sign is that of 11-03. The 4K Mac went the other way in one run each, from 26.3 / 50.9 to
22.5 / 49.3, but the Mac does not reach `auto`'s floor, where the knob moved. The deployed build
has the knob, so it goes as a knob that buys nothing, not as a regression. Gone with it:
`LATTICE_MAX`, `Grid::lattice` and `s`, three of the grid's words, the judge's G (`cell_ms`,
`per_cell`), `gpu_lattice` and the perf frame's `lattice`. `fixed` and `low` drew at lattice 1
before and draw the same now; `auto` keeps the rate and the scale.

**`auto`'s rate on the S23, judged on 11-08: the floor stands.** One 60 s drive under `auto` at
719 MHz, against the same at `fixed`:

| | rate, scale | frames a second | interval p50 / p95 | missed | GPU p50 / p95 |
| --- | --- | --- | --- | --- | --- |
| `fixed` | 59.9 Hz, 1.00 | 10.8 | 48 / 282 | 87% | 57.7 / 205.6 |
| `auto` | 29.9 Hz, 0.50 | 11.2 | 35 / 267 | 45% | 50.7 / 195.4 |

`auto` runs to both floors. The median frame meets the 33 ms interval, so 30 Hz is what the frames
can hold, and a lower floor would slow the median for nothing. Half the pixels each way buy 12% of
the GPU's p50 and 5% of its p95: the frame is the light's and the cells', which the scale does not
reach.

### Why frames cast, standing frames and held turns

**Why frames cast** (main `97f694bb41`, per-frame reason bits, and a variant with the world query
doubled). Circling or walking, 97–99% of frames cast, all for the eye and the jitter, and every
cast was earned. At rest none cast, yet the GPU is busy 10.8 ms a frame at 60 Hz (`world` 4.6).
Driving (pinned), gated frames carry 10.2 s of GPU against the cast frames' 2.0, with `mark` at
6.2 ms. One world query takes about 0.1–0.2 ms at p50, 15–25% of an asking pass's p95. So neither
the kernel's cast rate nor the world's rules are the next lever.

**No rest on 11-06.** In the hot runs every frame of each rest asked the next (`wakes next` = n),
with no decay over 15 s windows; the frames at rest are of the kinds held, `px` and gated. hot1's
rest ran 47 fps with the GPU busy 36 ms a timed frame (`px` p95 20). hot2's rest, the camera stopped
0.5 m from a grass wall, ran 22–24 fps with the GPU at 130–170 ms a frame and `px` p95 125–147 ms. On
`034ddad063` every frame of both rests still asked the next. rest2 had no motion or cast for 20 s
before it and still ran 18 fps at 151 ms of GPU a timed frame (`px` p95 80.5, `light_beam` p95 71);
it had not come to rest 35 s after the drive. During the drive half the frames were held, with no
GPU read (C h C h …), and they count in the fps; long intervals mostly fall on the held frame after
a cast.

**Standing on 11-07.** On the Mac, 8 s of the drive and then 60 s standing gave 3258 frames gated
and 342 stood, GPU 6.0 / 9.4 ms, the panel idle; the deployed build gave 3598 gated, never resting,
at GPU 9.2 / 13.1 ms. A held turn first stopped drawing the next frame whole where its lists moved
nothing; `frame_task`, below, then removed the held turn. The kernel's frame test still finds the
view resting 18 to 28 turns after it stops, against its bound of `JITTERS + FRAME_LATENCY`, and that
test fails. On the S23 main at first drew three times the frames the deployed build does while
standing, casting the pixels again (main-1 and main-2 under "Against the deployed build").

**A frame turn the GPU cannot take is not run** (11-07, the kernel's `frame_task`, kept): its
grant's time is added to the next. The renderer's held turn, which drew the next frame whole and so
started the jitter's sequence again, is gone. The Mac's gate passed ("Against the deployed build").
The Mac standing 60 s drew 1116 frames stood and 2484 gated, none casting the pixels, GPU
5.8 / 8.9 ms. On the S23 main-3, with the phone cooled since main-2, drew the drive as main-1 did,
and standing it cast the pixels none of the times that main-2 had cast them (1059); its row is in
the table under "Against the deployed build". Main draws the scene's animation at its own rate, more
often than the deployed build's standing window did. The profiler's interval now spans the turns
not run.

**A standing view, judged on 11-08: no change.** Standing 60 s on the S23, main drew 2605 frames
and the deployed build 840. The asks are not what differs. Since `4a4441d55b`, which the deployed
build lacks, an actor's clip steps where its pose moves a device pixel, where the deployed build's
stepped at the clip's fastest point throughout. A gated frame asks no frame after it
(`Resolve::asks`) and costs less than the deployed build's: GPU p50 18.0 against 30.0 ms, `world`
p95 10.1 against 13.2, `mark` 6.0 against 7.1. The deployed build drew fewer frames because each
took longer. Fewer frames would take a cap on the animation's rate, a knob, and none is added.

## The CPU's share: turns, readbacks and the tick

**The app's own profiler's trace** (a Perfetto export): the S23's gaps of 100–500 ms follow
runs of frames whose GPU work adds up to them, with script and render at 15–18 ms in each. The
gaps are GPU saturation and Chrome's queue, not turns, loads or readbacks. **Readbacks were ruled
out**: with the timestamps off, `map_async`s went from 1.2 to 0.2–0.4 a frame, and the gaps did not
change.

**The tick after the CPU fill** (11-08). The Mac's tick p95 went from 2.1 to 3.2 ms with the bricks
filled on the CPU. The fill's own stages run past the frame, in the share of the turn's tasks. What
a landing runs in the turn is `clipmap::cut`, which spread the whole tile's palette indices into a
scratch of 1 MiB to read a few bricks of them. Read a cell at a time where it is cut, the demand
step's most fell from 5.4 to 3.0 ms, the tick's most from 7.5 to 4.5–5.8, and the fixed ranges by
1 MiB; the p95 stayed at 3.2. Folding only the clip levels a change reaches was tried, moved
neither, and was dropped. The gate passed, and what held the p95 was not found until the next
reading.

**The tick's tail is the tiles landing.** The demand step, split for a run and not kept, showed that
what spikes is the arrivals' pass (`admit_arrived`): 0.2 ms a turn and 3.0 at the most, where the
deployed build's whole demand step was 0.8 at the most. The pass takes as many tiles as the turn's
budget leaves, so a burst of landings lands in one turn, and each now cuts its bricks' palette
indices on the CPU. With the cut written a row at a time, its cells' order hoisted, and one loop for
each way a packet holds them, the arrivals' most fell from 3.0 to 1.3–2.1 ms and the tick's p95 from
3.2 to 2.6–3.2 over four runs. The Mac's gate, three rounds against the deployed build, the Mac
under load, passed with tick p95 3.1 to 4.2 ms. The tick's p50 is 0.2 ms higher too, so what is
left is spread over the turn, not one step, and no turn ran past its budget. The user accepted the
tail as the CPU fill's cost, and holding packets past their turn to move the cut out is not taken
([the deployed build is the bar](../../minutes/20261107-the-deployed-build-is-the-bar.md)).

**The save at a suspend, closed** (11-08). A hidden page saves at once (`door/life.rs`), and an
edit goes with its turn. What is left is a hide in a turn with no write left, which nothing has
shown.

## Memory, submits and Safari

**Safari loads again** (`e0b0fadcdc`). `FACE_NONE`, `i32::MIN` folded, reached Metal as a bare
`long`; with no `select` of it and an int compiled, three pipelines were refused and the page stood
at its loading. The head word's own none (i16's lowest) is used instead, and `build.rs` refuses an
`i32::MIN`. It was found from the dev console relay.

**The page's GPU memory, read from the code** (main `417494fe4a`): 437 MiB on the Mac (428 on the
S23) with shadows never on, 565 once they are on (none of it given back), and ~780 at every cap with
load transients. Five loops or GPU-written counts had no hard cap, `way_free.wgsl:31` the worst by
reading: a one-lane walk of the ways pool that leaves only at depth 0. After the runs' pages the Mac's GPU
stayed at 100% with Finder, the Dock and the menu bar frozen, and the Mac was rebooted; the next day
quitting Safari alone freed it, so the reboot was not shown to be needed. Safari's GPU process died
at 1.4–1.5 GB after several runs left pages open; a fresh page stays well under that.

**The shadow resources follow the setting, and the loops and counts that read as unbounded are
capped** (the caps were not shown to fix anything; see the hung submit). Shadows toggled six times
a run in Safari, three runs, every allocation counted: off went back to 431.19 MB and on rose to
603.40 MB each time, a swing of 164 MiB with no leak, the same shadows on each turn-on, the GPU never
pinned. A "t must advance" stop on the capped walks was not taken, since a push, a corner crossing
or a node under t's ulp keeps t.

**A fresh Safari page holds 412 MiB, flat** (main `97f694bb41`), at the default and the far zoom,
with no climb. So the device losses read as memory on 11-05 were not shown to be memory. **Safari's
page memory measured in every phase** (`af434aad6e`, the shelf, each pass in its own submit, the far
zoom for 10 min): default 410 MiB, far 412, back 412, shadows on 577, flat, with no climb, no stuck
pass, no device loss and no pin, utilization under 71%. The 0.6–0.7 GB gap of the code's inventory
does not exist on a fresh page: the 1.4–1.5 GB belonged to one GPU process serving several hung
pages. The hang did not recur in that run, but the face cap and the split submits had changed
together.

**The hung submit at Safari's far zoom.** On `97f694bb41` one GPU submit never finished 80 s into
the far zoom, near a sky step: in flight stuck at 1, GPU 100%, the GPU process uninterruptible, the
canvas off. The caps in `9d55c0c66d` did not stop it, so `way_free`, which that commit named
likeliest, was not the cause. Force-quitting Safari freed the GPU at once. On `af434aad6e` unsplit,
with the face cap, the GPU pinned about 2 min into the far zoom with memory flat at 412 MiB; with
each pass in its own submit the same phases ran 10 min clean, passes in the tens of ms. So the face
cap is not the fix. The suspect is one large command buffer stalling in WebKit or Metal, not a
shader that never ends; that rests on two runs against one, and a split run could be luck. Reading
found every loop in that frame ending on data the code can produce. On main `3d9521137f`
(forget-bricks and px-tiles, one submit a frame) it pinned again about 50 s into the far zoom,
memory flat at 412 MiB, so the cache's aliasing was not the cause, which puts the weight on the
single large command buffer. The next step named was the frame in several submits, in the app,
tested in Safari. **The frame in stages, 4–6 submits** (`submits.patch`, held for the Mac's GPU
tests) costs nothing on the S23, shadows-on busy p95 53 to 46 ms; its Safari run waited for a
force-quit, and whether it stops the hang is untested.

**Safari with shadows on, rechecked on 11-08** (Safari 27.0.1, `tool/s23/measure.js`, a watcher to
force-quit it): no pin. With shadows off through 120 s of the far zoom and on for 90 s at the
default, the GPU reached 79% at most, the Mac's other work about 40–50%. With shadows on from the
start through 180 s of the far zoom, the sky's time moving, it reached 72% at most, the GPU process
at 310 MB, with no page error. The page's live bytes read 0: the proxy's counters do not reach the
worker's device. No shot was taken in Safari, so that it drew is read from the load's end and from
the utilization moving by phase.

`tool/s23/measure.js` loads a page in the Mac's Safari through safaridriver (`safaridriver -p
4444`) and runs its phases: the default zoom, the farthest zoom with the sky's time moving, back, and
shadows on. Every 5 s it logs the page's live GPU bytes and object count, the GPU process's footprint
and the GPU's utilization to `<label>.jsonl`, and it stops if the GPU pins at 100% or the footprint
passes 1.2 GB. The page is served behind `tool/s23/mem-proxy.js`, which counts every buffer and
texture from before the first frame, or `tool/s23/pass-proxy.js`, which also gives each pass its
own submit and logs the passes still outstanding. A pin holds the GPU until Safari is force-quit.

**Safari over `http://localhost` asks no zstd** and gets the 1 MB tile form; over https Safari takes
zstd since Safari 26.3. This is left to time.

The command buffer that was invalid every frame on the S23 until `034ddad063` is under "On a hot
phone".

## First load

**On 11-03 a cold first load took about 94 s to the world on plei.me**, with one restart of the page
near 60 s that logcat does not explain, and 91 s on the dev build with none; warm, it took 15 s.
Moving on plei.me over `requestAnimationFrame`, the page ran 7.5 fps, p50 33.9 and p95 550.1 ms, with
no long task (`tool/walk/raf.js`). The 90 s were later found to be the data's format and the
harness ([taking rust-gpu out, the journal](../20261103-the-shaders-by-hand/README.md), "Did not
work"). On the rebase over main at `430cdc831`, which reads `division` 9, the evening's runs at
`pivot -` had been a build of format 8 refused its source, not the page, and with the harness's own
faults they are no evidence. `tool/walk/load.js` wraps the page's calls before its first script
(`report/load-*.json`); seconds from navigation on the S23:

| | First pipeline | All made (of) | Wall | World |
| --- | --- | --- | --- | --- |
| `981947702`, cold | 4.3 | 18.9 (90) | 14.6 | 19.4 |
| `981947702`, warm | 5.1 | 21.1 (90) | 16.0 | 28.8 |
| `430cdc831`, cold | 3.6 | 19.1 (87) | 15.5 | 19.5 |
| `430cdc831`, warm | 4.7 | 20.3 (87) | 15.6 | 20.6 |
| `981947702`, the phone's own location, cold | 4.4 | 19.5 (90) | 15.1 | 28.0 |
| `981947702`, the phone's own location, warm | 3.5 | 21.0 (90) | 17.5 | 22.8 |
| `981947702`, a location prompt left unanswered | 5.0 | 21.3 (90) | 16.3 | 26.9 |

- A reload makes the pipelines no faster than a first load: about 15 s each time.
- The hand passes add 3 pipelines and no time. 10 `light_*` pipelines and 7 of decor are made with
  shadows off.
- The slowest are `decor_cs`, `decor_footing_cs`, `face_cs`, `light_*` and `miss`, at 12–16 s, side
  by side, so a pipeline's own cost is not read here. Modules take 21–25 ms of calls for 5.1 MB.
- From the first compute pass to the toast gone takes 0.3–1.0 s, and up to 8 s in some loads; this
  was not split.
- No load took a minute, on either build, with the page's location answered or left unanswered.

**One module for `miss` and `miss_list`** (`c83fd6f67b`): 84 modules of 977 KB, where 85 were
1,036 KB. The Tint test and the probe now make all 88 pipelines, where they made only a module's
first entry. The inlining of 11-03 had gone with rust-gpu already.

**Pictures before models** (`2297b60d29`): the Earth's sphere waited behind the whole model catalog,
and Safari's far corner showed the sky's black until 57 s. It fills once it lands, at 70 s on the
dev link.

**plei.me answers Korea from LAX** at 60–110 KB/s; this is left as it is.

**A load that never draws**, seen on the S23 on `7bf251816` as on `d25f5b631`: the world never comes,
and the toast stays on "공간을 불러오고 있습니다", "위치 찾는 중…" (`report/s23-failed-load.json`).
When caught, `navigator.geolocation.getCurrentPosition` at 5 s gave error 3, timeout, with permission
granted, where the same call answered at the page's first moment; the page showed `pivot -`, frames
`plain` and `stood`, and no failed resource. So the page was waiting on a position that never came.
The position here was the DevTools override, set again every 5 s by the script, so this may be the
harness's. Whether a real device's silence holds the page the same way was not tried.

## The picture

**How pictures are compared.** A picture is shot at rest with the sky fixed, the origin's storage
cleared before each load and the camera logged, and compared with the companion left out
(`tool/walk/shotcmp.py`). Two frames of one run differ only on the companion, but two runs of one
commit differ by a mean of up to 6.69, from what is still arriving, the toast and the companion,
which hides a small change ("What 'other zooms' is"). Each change's own comparison is with the
change.

**What the hot phone showed on 11-06** (`ef8d1621b7`, hot1 and hot2, and `034ddad063`, hot3; shots
every 10 or 20 s under `out/s23/hot*-shots/`, not kept):

- From about 100 m out the ground breaks into thin vertical slivers and fragments over a flat
  untextured green (hot2 s-t0086, s-t0096, s-t0136, s-t0166). With the indirect scope fixed, at 2 km
  (hot3 s-t0111) and 200 m (s-t0211) the ground is still slivers, scattered boxes and dark squares
  over flat green.
- At 10–20 km a bright horizontal line crosses the horizon, and the far ground ends at a hard
  horizontal edge in haze (hot1 s-t0176, s-t0291; hot2 s-t0151). The line still crosses it at 20 km
  in hot3 (s-t0101).
- Inside the ground the camera met faces against solid: at 2 m three rock faces in a corner (hot2
  s-t0221), at 100 m a full screen of one brown texture (hot1 s-t0336), and in hot3 at 2 m a grey
  rock (s-t0141) and at 20 m the whole screen one grey-violet rock (s-t0156).
- At rest after the drive the mid-distance hillside is flat green with faint box edges, and a thin
  vertical sliver runs at the left edge (hot1 s-t0356). At rest in hot3 a grass wall fills the
  screen (s-t0246, s-t0261, s-t0276).
- No stale shadow was seen after a turn. The sun is low at the drive's dawn, so few shadows are on
  screen.

The plan of 11-06 left the horizon line's cause and the dark squares open on 11-07 ("The batch of
11-06"), and no later run reports on them.

**The dark band along the horizon was not found again** (11-08). Its cause was the parked beam rays
and the holes they left (`56e388dc33`), and neither exists since a ray ends in its frame. Two 4K
drives on main, a shot every 75 ms at a quarter of the size, gave 687 shots, 152 of them at the far
end, and none has a band. Dark under the horizon there is the clouds' underside or the space over
the limb, in the shots before and after alike. At the earlier count of 5–9 a drive, about one frame
in three shot, some would have shown. The near end's dark shots are the inside of a grass block the
camera passes through.

**"Fewer rocks at the far zoom" was the clouds.** On 11-07 the drive's shots showed fewer rocks at
the far zoom than the deployed build's, and this was left open; the shots had been taken at
different zooms. Held at the far end, with the same input on both, at 4K, main and the deployed
build differ at rest and a second into a turn in 0.8–0.9% of the pixels, all of them the white
lumps, the panel's text and the compass. The lumps are the cumulus, the same ones shifted, which
drift by the wall clock, and the two runs were a minute and a half apart. The ground is alike.

**The character preview shows** (11-08). The scene's work found it culled by the world's eye under
the sea (`8671645700`). In Chrome the drawer's Character tab draws the person and the pet.

## The whole frame on the S23

### Where it started, and a day's work

On 11-05 main at `a449637a35` ran 35–40 fps on the S23 with shadows off, where rust-gpu's page had
run 5.9 on 11-03, with GPU p50 21–24 ms and `sample` p95 5.6–7.0 against 133. Three runs in a row
warmed the phone from 43 to 48 °C with fps drifting down, so this is a reference, not a fixed number.

**Morning against night.** `a449637a35` against `c3847db899` on the S23, both off the shelf,
alternating twice, the phone warming to thermal 1 alike on both sides (data `out/s23/day*`):

| | morning | night |
| --- | --- | --- |
| shadows off, fps | 33 / 30 | 42 / 38 |
| shadows off, busy p95 | 40–46 ms | 29–32 ms |
| shadows on, fps | 16 / 15.5 | 24 / 24 |
| shadows on, busy p95 | 110–115 ms | 50–52 ms |
| drive (pinned), cast frames' busy p95 | 175–240 ms | 122–136 ms |
| first load to the world | 9.9 / 5.2 s | 6.9 / 5.7 s |

The largest moves: the tails of `light_hit` and `light_walk` −90%, `light_receivers` −55%, `sample`
−65%, `world` −25–30%; the eye's nodes a cast fell from 57k to 12–15k in `sample` and from 20k to
2–4k in `miss`, and the light's beam from 114–153k to 7k.

### On a hot phone

The batch of 11-06 on main at `ef8d1621b7` ran on the user's drive, shadows on, `res fixed`, off the
shelf. Two fresh loads ran with `tool/s23/s23.js`: hot1 with `SCENES=drive,rest`, `DRIVE_S=300
FRAMES=1`, and hot2, after 3 min off, with `SCENES=drive,rest,rest`, `DRIVE_S=180`. Beside each, every
5 s, the GPU's clock cap (kgsl `max_freq`, `thermal_pwrlevel`) and the AP's and the skin's
temperatures were logged, with an adb screencap every 20 s and every 10 s; beside hot2, a second
DevTools client kept every console error. The profiler keeps the last 60 s, so each figure is the
drive's last minute (data, shots and clock logs `out/s23/hot1*`, `out/s23/hot2*`).

| | hot1, still throttling | hot2, settled hot |
| --- | --- | --- |
| GPU clock cap in the window | 615 then 550 MHz (level 2–3), of 719 | 348 MHz (level 6) from 50 s on, 295 for 15 s |
| thermal status, skin | 0 to 1, 39.5–40.5 °C | 1 at the end, 39–40 °C |
| fps | 22.3 | 12.4 |
| a frame's fps p50 / p5 | 38.5 / 7.6 | 22.7 / 4.3 |
| interval p50 / p95 / max | 26 / 133 / 388 ms | 44 / 231 / 521 ms |
| cast frames' busy p50 / p95 | 77 / 130 ms | 135 / 248 ms |
| px | 29.6 / 67.4 | 50.5 / 126.6 |
| miss | 4.0 / 18.0 | 8.1 / 36.4 |
| sample | 8.3 / 16.9 | 16.5 / 33.4 |
| beam | 7.9 / 15.1 | 14.2 / 27.6 |
| world | 6.9 / 8.3 | 11.6 / 15.1 |
| cast | 6.2 / 8.1 | 9.5 / 14.0 |
| sample_box | 3.4 / 4.9 | 5.4 / 8.1 |
| light_raster | 2.3 / 2.8 | 3.8 / 4.7 |
| every other light pass, p95 | ≤ 0.5 (light_receivers 4.1) | ≤ 0.9 |

Spans are ms p50 / p95. hot2 is the hot level: the clock held at its floor for two minutes, and its
two 30 s halves read 12.6 and 12.2 fps; hot1's halves read 23.1 and 21.8 while the cap was still
stepping down. Against the honest drive's baseline of 16.4 fps and busy 52 / 115 ms, and a deduced
hot frame of about 80 ms, a hot cast frame took 135 ms at p50 and the drive ran 12.4 fps.

**A command buffer was invalid every frame, from load.** The console read `[Buffer (unlabeled)]
usage (Indirect|Storage(read-write)) includes writable usage and another usage in the same
synchronization scope. While validating compute pass usage`, then `[Invalid CommandBuffer] …
Submit`: 7,490 pairs in hot2's 263 s (25–50 a second), through the load, the drive and the rest.
The pass was not identified. A submit's work was dropped each frame, so the figures above time only
the submits that ran. The fix
(`034ddad063`): an indirect call whose args are bound writable is refused as it is encoded, and the
four dispatches' counts sit in buffers of their own. In Mac Chrome the same page, loaded for 45 s
in a fresh profile, showed no GPU validation error and no `ENOTRECOVERABLE`; four React Native
"touch end without a touch start" warnings came from the pointer over the window.

Two loads on the S23 at `034ddad063` ran as before: hot3w, a warm-up (`SCENES=drive`, `DRIVE_S=300`,
ray counts off), and hot3 right after it (`SCENES=set:ray_counts=1,drive,rest,rest`, `DRIVE_S=180`).
Both logged the clock cap and took shots every 10 s, and hot3 also had the console listener. Neither
page logged a console error (`errs []`, and the listener wrote nothing). The clock cap fell to
550 MHz (level 3) 81 s into hot3 and stayed there to the end, status 1, skin 40.1–40.5 °C. It did not
reach hot2's 348 MHz, so hot3 is set against hot1, which had the same cap (615 to 550 MHz). Data:
`out/s23/hot3*`. Pass times are ms p50 / p95 over the drive's last minute.

| | hot1, ef8d1621b7, 615→550 MHz | hot3w, counts off, 615 MHz | hot3, counts on, 550 MHz |
| --- | --- | --- | --- |
| fps | 22.3 | 13.4 | 10.7 |
| a frame's fps p50 / p5 | 38.5 / 7.6 | 32.3 / 3.9 | 24.4 / 3.2 |
| interval p50 / p95 / max | 26 / 133 / 388 | 31 / 256 / 405 | 41 / 315 / 452 |
| cast frames' busy p50 / p95 | 77 / 130 | 166 / 251 | 194 / 303 |
| light_beam | 0.1 / 0.5 | 73.0 / 82.7 | 81.5 / 92.3 |
| light_sample | 0.1 / 0.5 | 11.1 / 26.0 | 12.2 / 28.4 |
| light_walk | 0.1 / 0.5 | 2.0 / 34.5 | 4.1 / 40.0 |
| light_hit | 0.1 / 0.2 | 1.8 / 19.9 | 4.8 / 20.2 |
| light_raster | 2.3 / 2.8 | 2.6 / 3.6 | 2.6 / 3.5 |
| px | 29.6 / 67.4 | 17.2 / 57.0 | 26.0 / 74.4 |
| miss | 4.0 / 18.0 | 4.8 / 19.7 | 5.6 / 24.1 |
| sample | 8.3 / 16.9 | 8.7 / 16.7 | 11.1 / 19.6 |
| beam | 7.9 / 15.1 | 6.9 / 12.8 | 8.3 / 16.6 |

**The light's work now runs, and the figures of `ef8d1621b7` timed a frame without it.** On
`ef8d1621b7` the light's beam, sample, walk and hit passes read near zero. With every submit valid
they take about 100 ms of a cast frame, `light_beam` about 80 ms of it, and they follow a frame's
busy most closely (`light_beam` 0.86, `light_sample` 0.79, `light_hit` 0.73, `px` 0.69). That the
dropped command buffer held those passes is read off these timings, not traced. The cost of counting
stays under the clock's share: hot3w and hot3 differ by 17% in busy, and the cap by 12%. The rays these
runs counted are under "The batch of 11-06", their rests under "Pacing and rest" and their shots
under "The picture".

## Against the deployed build

From 11-07 the bar is the deployed build, `e317a3644b`
([the deployed build is the bar](../../minutes/20261107-the-deployed-build-is-the-bar.md)); how it
and main are served is under "How the runs were taken".

**The Mac's gate**, `script/compare-perf.py`, the deployed build to main; every gate passed:

| Main with | Rounds | GPU p50 / p95, ms | First load's longest frame, ms | Tick, ms |
| --- | --- | --- | --- | --- |
| the eye's holes as a list (11-07) | 3 | 19.60 / 37.36 to 19.46 / 37.36 | 878 to 175 | 1.5 / 2.2 to 1.5 / 2.1 |
| the node cache's keys apart (11-07) | 2 | 19.79 / 37.22 to 18.87 / 39.19 | | |
| `frame_task` (11-07) | 2 | 20.77 / 38.99 to 16.78 / 34.80 | | |
| main at `6b9c4549ab` (11-08) | 2 | 19.27 / 34.28 to 16.12 / 31.39 | 1041 to 178 | p95 2.20 to 3.00 |
| the cut a row at a time (11-08), the Mac under load | 3 | 27.0 / 47.8 to 20.4 / 36.9 | | p95 3.1 to 4.2 |

**The S23 on 11-07**: main at `b034c0bf54` with the working tree, the drive and then 65 s untouched,
fresh loads by turns, the GPU's clock as logged. main-3 is main with `frame_task`, the phone cooled
since main-2.

| run | clock, MHz | frames drawn a second | GPU p50 / p95 | standing 60 s: frames, GPU p50 |
| --- | --- | --- | --- | --- |
| main-1 | 719 | 9.5 | 62.5 / 238.7 | 3554 (951 px, 960 held), 18.0 |
| base-1 | 615–680 | 2.5 | 304.4 / 1282.1 | 1004 (969 gated), 20.3 |
| base-2 | 550–719, most 550 | 1.5 | 300.2 / 437.6 | 840 (731 gated), 30.0 |
| main-2 | 550, two reads 615 | 6.8 | 82.3 / 359.7 | 3289 (1059 px, 1109 held), 23.3 |
| main-3 | 680–719 | 9.1 | 63.2 / 249.2 | 2604 (gated, none casting the pixels), 18.0 |

At 550 MHz (main-2 against base-2) main's GPU p50 is 3.6 times better than the deployed build's and
its p95 1.2 times, and main draws 4.5 times the frames. The 2× bar held at p50 and not at p95. The p95 is the
light's now: `light_hit` 152, `light_sample` 73 and `light_walk` 48 ms, together more than the
deployed build's light (`light_walk` 174, `light_sample` 64, `light_beam` 34), where the eye's `miss`
takes 13 against 272. With the claim budget gone, an epoch casts every tile it reads in one cast.

**The S23 on 11-08**: main at `6b9c4549ab`, fixed resolution, 120 s drives by turns, the report's last
60 s:

| run | clock, MHz | frames a second | GPU p50 / p95 |
| --- | --- | --- | --- |
| main-1 | 719 | 11.7 | 53.7 / 220.2 |
| base-1 | 719 | 2.2 | 326.8 / 1246.6 |
| main-2 | 615, most | 9.9 | 65.9 / 259.1 |

At 719 MHz main's p50 is 6.1 times better than the deployed build's and its p95 5.7 times. The
deployed build's p95 has read 437.6 to 1282.1 ms over its runs, on 60–92 timed frames, so 11-07's
1.2 times at 550 MHz and this 5.7 times are one pair each; no pair at 550 MHz was taken again.
Against the deployed build's best run, 437.6 ms, main's p95 is 1.7 to 2 times better.
