# The walk on a phone

**2026-11-03.** Why the page runs at 6–7 fps on a Galaxy S23 as the view moves,
at `d25f5b631`, the deployed commit: what sets how often `beam` and `sample` run,
what a lattice of 2 buys, and how many nodes the walks visit against what each
takes on the S23 and on a Mac. What this means for the shaders is in
[the walk on a phone, and who owns it](../../minutes/20261103-the-walk-on-a-phone-and-who-owns-it.md).

Closed. The walks measured here are rust-gpu's and the hand passes of 11-03 and 11-04.
rust-gpu went on 2026-11-05 ([taking rust-gpu out](../20261104-taking-rust-gpu-out/README.md)),
the walks run as the holes' queue since 11-06 (`9f116e5f34`), and a ray ends in its
frame since 11-07 ([a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md)).

A commit named below that `git log --all` no longer shows was of the work in progress
of 11-03 and 11-04. The same change is on `original`, rebased, under its own subject;
main holds the hand walks squashed in `6c196144d4`, `f9f66682e` and `e8d616ba4` in
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

The shell scripts in `tools/` serve each commit's page from `ts/app/web`, the page's
tree of the time, in worktrees of their own; the scratchpad and the cargo target they
name are the session's that ran them (`572d46ae`), both gone.

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

The short stack went at `94d6e9284`, once `sample` walked only what the hand pass left
("What stays in `sample`" below).

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
  phone is partly the camera: below. 54.2 ms over those is 524 ns a node, far past
  the 111: `sample` pays to be entered and pays more on a left ray, both below.
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

### What stays in `sample`

`94d6e9284` runs `sample_cs` by an indirect dispatch over only what `sample_hit` left,
so against `749560da1` the change is the cost of being entered. The left rule is the
same (`tools/run6.mjs`, `reports/s23-left-only.json`, `reports/mac-left-by-speed.json`).
A turn of the circle in seconds; the Mac at `749560da1`, the S23 at `94d6e9284`:

| | Turn | fps | Left, nodes a cast | `sample` p95 | `miss` p95 | `beam` p95 |
| --- | --- | --- | --- | --- | --- | --- |
| Mac | 6 | 60 | 10,654 | 1.4–1.7 | 1.7 | 2.2 |
| Mac | 1.5 | 60 | 38,743 | 1.7–1.8 | 1.6 | 2.8 |
| S23 | 6 | 13.3–13.9 | 90,082 | 33.5–39.6 | 58.9–69.7 | 39.5–40.2 |
| S23 | 24 | 12.4–17.0 | 47,471 | 10.8–15.9 | 37.6–60.3 | 31.6–38.4 |

- **Being entered costs about 14 ms of the 47–54.** Every thread out of `sample_cs`
  took it to 33.5–39.6, and the frame gained no fps: 13.9 against 14.0–14.6.
- **What is left is walked at 230–440 ns a node**, past the old walk's 111: `sample`'s
  p95 over the left lane, a comparison. A left ray asks what no slot keeps, so it
  pays `world::node`, where the old figure was mostly hits.
- **A faster camera leaves more, and that is about half of it.** A quarter of the
  turn leaves 3.6 times as many on the Mac, 1.9 on the S23. The S23 at 14 fps and a
  6 s turn moves as far a frame as the Mac at 60 and 1.5 s, and leaves 2.3 times as
  many; at 17 fps and 24 s it moves about as the Mac at 6 s, and leaves 4.5 times.
  The rest was not traced.

### `miss` by hand

`e7757099f`: `miss_hit_cs` ahead of `miss_list_cs`, over the same marked list, the
pyramid read-only, atomics only on its own 8-word buffer; `walk.wgsl` holds the walk
it shares with `sample_hit`. The span `miss` covers both passes, and `ray_counts`' lane
what is left (`reports/s23-miss-by-hand.json`). The S23, a 6 s turn, four windows:

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

- **It makes its pipelines and draws.** At rest on the S23, 45 s after the world, the
  picture is the ground and trees as on the Mac, by eye. No flat ground was seen; none
  was looked for before 45 s.
- **A tenth of what `miss` walked is left**, and `miss` falls to a third.
- **The frame is still bimodal**: an interval's p50 is 20–21 ms and its p95 over
  217. `beam` and `sample` hold the frames that run them.

### `beam` by hand

`8ac7315b1`: `beam_hit_cs` over the pyramid read-only, each node's `Inheritance` written
straight to its line, no stack. What it leaves `beam_cs` walks by indirect dispatch,
with the 33 entries, the atomics and every write to the pyramid. The span `beam` covers
both, and `ray_counts`' lane what is left (`reports/s23-beam-by-hand.json`). The S23, a
6 s turn, four windows:

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

- **The median frame gains, the worst does not.** GPU p50 falls by a third, p95 stands.
  `beam`'s p95 rose: 39–48% of its nodes are left, every write to the pyramid is in that
  pass, and a frame that meets many new nodes pays both passes.
- **`face` read 4.6 before and 13 here**: the run, as a later run of this code shows.
- **It draws**; no view moved unasked in these windows (16 u, polar 60).

The picture against `d25f5b631`, on the S23, at rest, sky fixed, two runs of each
(`tools/shotcmp.py`, the companion left out):

| | Differ | Over 2 | Over 8 | Over 32 | Mean |
| --- | --- | --- | --- | --- | --- |
| One run, two frames | 0.027% | 0.027% | 0.025% | 0.019% | 0.023 |
| `8ac7315b1`, run a / run b | 37.6% | 1.86% | 0.15% | 0.010% | 0.48 |
| `d25f5b631` b / `8ac7315b1` b | 36.2% | 1.90% | 0.10% | 0.013% | 0.48 |
| `d25f5b631` a / `d25f5b631` b | 33.4% | 4.47% | 3.78% | 3.05% | 3.89 |
| `d25f5b631` a / `8ac7315b1` a | 42.0% | 5.90% | 3.83% | 3.04% | 4.02 |

`d25f5b631`'s run b differs from the hand build as little as the hand build's two runs
differ from each other. Its run a differs from everything by the same amount, over the
top of the picture: not the hand walk's, and not traced. At one place, shadows off.
Ray by ray is still unchecked.

### Why rays are left

`8ac7315b1`, instrumented in a scratch tree (`tools/left-instrument.patch`, none of it
on the branch): at the node a ray first stopped on, the hand pass records the node and
why no slot served it. `tools/left.mjs` reads that and the pyramid on two consecutive
frames, while the camera circles: 10 frames on the S23, 6 on the Mac
(`reports/left-*.json`). The copies are 25 MB and slow what they read; only counts are
taken from them.

| | Left rays | Distinct first stops, summed by frame | Rays a node | Other zooms | Absent, room in the bucket | Absent, bucket full |
| --- | --- | --- | --- | --- | --- | --- |
| S23 `beam` | 10,495 | 366 | 28.7 | 5,281 | 5,207 | 7 |
| S23 `sample` | 9,341 | 958 | 9.8 | 444 | 8,790 | 107 |
| S23 `miss` | 1,894 | 131 | 14.5 | 72 | 1,802 | 20 |
| Mac `beam` | 1,547 | 144 | 10.7 | 1,365 | 182 | 0 |
| Mac `sample` | 1,239 | 181 | 6.8 | 51 | 1,183 | 5 |
| Mac `miss` | 617 | 44 | 14.0 | 24 | 593 | 0 |

- **Few nodes hold many rays**: 10 to 29 a node. Asking each once is worth weighing.
  These are first stops only: a ray served at one may stop again deeper, so this bounds
  one round of a queue, not all of them.
- **`beam` is half zooms changed.** The node is kept, but at other asked zooms: half of
  the left beams on the S23, 88% on the Mac, mostly at zooms 22–24, the nodes near the
  eye. `sample` and `miss` are nearly all nodes never asked.
- Eviction is small: 134 of 15,933 absent met a full bucket. Forgetting read small here too,
  98 held the frame before, but this read missed the frames that forget: see "What stops a
  whole frame".
- **The rays come in bursts.** On the S23 one frame of the ten left 5,122 beams to 115
  nodes, and two left none. No reasons were read for a frame's GPU time against its
  count, so the tail's tie to these bursts is a reading.

`face` at `8ac7315b1`: `boxes a cast` is 4,216–4,467 against `e7757099f`'s 4,587–5,209,
so its cost of 13 against 4.6 ms is not more boxes. Its hits and asks were not counted.

### A coarser `far` taken for the same `near`

`2a9b235f3`: the hand walk takes a slot held from a coarser `far` to the same `near`;
`beam_cs` keeps the exact key (`reports/s23-rekey.json`). The S23, a 6 s turn, four
windows, against `8ac7315b1`:

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

- **`beam` did not move.** How many of the "other zooms" differ in `far` alone was not
  counted.
- **`face` is back at its old value**, with no fewer left beams; below, that was the
  run and not this change.

The picture, on the S23, at rest, sky fixed, the origin's storage cleared before each
load and the camera logged, at the default view and two views zoomed out (93 u, 541 u),
two runs of each commit. Mean difference of a pixel, the companion left out:

| | Default | 93 u | 541 u |
| --- | --- | --- | --- |
| `d25f5b631`, run a / run b | 1.68 | 4.13 | 2.08 |
| `8ac7315b1`, run a / run b | 6.69 | 5.00 | 4.60 |
| `d25f5b631` a / `2a9b235f3` a | 1.87 | 0.66 | 0.42 |
| `d25f5b631` b / `2a9b235f3` a | 0.66 | 4.06 | 2.14 |
| `8ac7315b1` b / `2a9b235f3` a | 2.27 | 1.13 | 1.31 |

The commits differ no more than a commit differs from itself between runs. That
noise is large, from what is still arriving, the toast and the companion, and would
hide a small change. By eye the three are the same, with no darker or bluer picture.
Not compared: a moving view at a boundary of detail, or `2a9b235f3`'s run b.

`2a9b235f3`'s run b opened at 20.2 u, polar 56.3, azimuth -15.5: a touch on the phone, the
user's, as the Mac's like runs were not the hand walk's. It is left out of the table.

Stored state had spoiled the runs before it: a view is kept by origin, and a run opens
where the last left. Two of those six sat on `pivot -` for 180 s, one each at
`2a9b235f3` and `d25f5b631`; none of the six from cleared storage did.

### What "other zooms" is

`2a9b235f3`, its misses instrumented as before (`tools/left-instrument-split.patch`,
`reports/left-split-*.json`), the left beams' reason split by how the slot held differs
from what was asked. Held minus asked; the walk takes a coarser `far` already, so what
differs in `far` alone here is a finer one held. 14 frames on the S23, 8 on the Mac:

| | Left beams | Distinct first stops | `near` alone | `far` alone | Both | Absent |
| --- | --- | --- | --- | --- | --- | --- |
| S23 | 16,068 | 342 | 3,065 | 429 | 11,349 | 1,225 |
| Mac | 5,171 | 151 | 4,690 | 232 | 27 | 222 |

- **`near` decides.** On the Mac 91% of left beams are held at another `near`, 4,238 of
  them one coarser than asked and `far` the same.
- **The S23's bursts are one node.** 10,540 were held both 2 coarser than asked, in two
  frames of the 14: 6,848 left beams to 7 nodes, 6,366 of them at one node of zoom 21,
  and 4,838 to 109, 4,171 at zoom 22. These are nodes high on nearly every beam's way,
  so one key stops most of a frame. Four frames left none. That this is the frame's
  tail is still a reading: no frame's GPU time was read against its count.
- `sample` and `miss` stay nodes never asked: 23,869 of 25,161 and 3,466 of 3,514 on the
  S23.

### What a frame costs, and how deep a queue goes

`7bf251816`, the passes as at `8ac7315b1`. `face` p95 there is 6.6–7.0, where the same
code took 12.9–14.1 at `8ac7315b1` and `2a9b235f3` 4.6–5.6. `world` moved too, 8.7 at
one and 11.8 at the other, on the same code. So those were no finding of `face`: on this
phone a pass's p95 moves up to twice between runs, and only windows of one run compare.

**A frame against what it left** (`reports/s23-frames.json`, `tools/tail.py`): the
profiler's rows, each frame's GPU time and spans beside its `ray_counts` lanes, 149
frames with both, a 6 s turn, `ray_counts` on.

| Left nodes, all passes | Frames | GPU busy, ms | `beam` | `sample` | `miss` |
| --- | --- | --- | --- | --- | --- |
| under 20 thousand | 18 | 31.1 | 4.8 | 0.6 | 1.3 |
| 20–100 thousand | 33 | 40.4 | 11.2 | 1.9 | 2.6 |
| 100–250 thousand | 24 | 51.8 | 24.6 | 1.2 | 1.8 |
| 250–500 thousand | 43 | 88.9 | 43.6 | 9.1 | 7.1 |
| over 500 thousand | 31 | 157.2 | 57.1 | 40.2 | 28.7 |

- **The tail is the left nodes.** GPU busy on all left nodes: 32.4 ms plus 150 ns a node,
  correlation 0.96. By pass, at no node left and a left node's cost: `beam` 4.1 ms and
  139 ns, 0.96; `sample` 1.8 and 113, 0.99; `miss` 2.9 and 213, 0.94.
- **The floor read 32 ms with `ray_counts` on**, which is high: below.

**How deep.** `tools/left.mjs`, `MODE=depth`, no shader change: a frame's entries the
pyramid tags with that frame and pass, each counted one deeper than its parent if the
parent is new too. The depth is the round of a queue that asks one layer a round
(`reports/s23-depth.json`). 12 frames on the S23:

| | Nodes in `beam` | Rounds | Nodes by round |
| --- | --- | --- | --- |
| Most frames | 5–892 | 1–8 | most in the first two or three |
| Two frames of the 12 | 2,113, 2,228 | 19 | 100–128 in the first, a few each to the 14th, then 118, ~550, ~340, ~480, ~470 |

`sample` is at most 4 rounds, `miss` 1 or 2 where read. What is kept alone is counted, so
an opaque cell a beam does not keep is not among them, and these bound from below.

**The floor** (`reports/s23-frames-all-spans*.json`), every span of each frame, in one
sitting, mean ms:

| | Frames | Busy | `world` | `beam` | `px` | `face` | `finish` | `sraster` | `generate` | `resolve` | `miss` | `sample_hit` | `sample` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Counts on, under 20 thousand left | 23 | 28.7 | 8.98 | 4.74 | 4.14 | 2.45 | 1.50 | 1.35 | 1.40 | 1.04 | 1.31 | 0.85 | 0.53 |
| Counts off, the fastest quarter that cast | 146 | 20.4 | 5.67 | 4.36 | 1.32 | 1.96 | 1.46 | 1.31 | 1.31 | 0.97 | 0.84 | 0.77 | 0.06 |
| Counts off, every frame | 631 | 50.6 | 5.31 | 17.79 | 1.37 | 4.58 | 1.46 | 1.42 | 1.47 | 0.94 | 7.91 | 1.16 | 6.65 |
| Counts off, frames that did not cast | 50 | 7.9 | 2.59 | | 1.64 | | 1.30 | 0.95 | | 0.50 | 0.79 | | |

A frame that casts with nothing left is about 20 ms, not 32: `ray_counts` itself costs
`px` 3 ms and more. The spans sum to the busy time, so no GPU work is unaccounted. Every
frame's p50 is 31.4 and its mean 50.6.

### A load that never draws

Seen on the S23 on `7bf251816` as on `d25f5b631`: the world never comes, the toast stays
on "공간을 불러오고 있습니다", "위치 찾는 중…" (`reports/s23-failed-load.json`). When caught:
`navigator.geolocation.getCurrentPosition` at 5 s gave error 3, timeout, with permission
granted, where the same call answered at the page's first moment; `pivot -`, frames `plain`
and `stood`, no failed resource. So the page was waiting on a position that never came.
The position here is the DevTools override, set again every 5 s by the script, so this may
be the harness's. Whether a real device's silence holds the page the same way was not
tried.

### Two rounds, and a Split at other zooms

Each against its parent in one sitting, by turns (`tools/ab2.sh`, `tools/ab3.sh`).

**A queue**, `3e7ba241d` against `7bf251816`: each left beam's first stop asked once, then
the hand walk again. It left 115 thousand nodes a cast against 155–166, and took `beam` from
44–60 to 88–99 and GPU p95 from 108–167 to 175–217. fps 13.0–17.2 against 15.8–23.4.
Reverted. `ask.gen.wgsl` peaks at 95 live words, so the cliff is not why. A queue that
asks each node once came back on 11-06, the holes' queue (`9f116e5f34`), its rays parked
within their frame since 11-07 ([a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md)).

**The node that stops a frame.** `7bf251816`, a first stop's distances against its side
(`reports/left-near-s23.json`). Two frames of 16 left 6,848 beams, every one of the grid's
corners, to one node: zoom 17, side 128 u, the eye inside it (near 0), its far corner at 192
u, asked [23, 27] and held [22, 26]. Other bursts are alike: a node of the chain over the
eye, near or far on a boundary of `zoom_at` (12, 24, 48, 96, 192 u).

**A Split held at other zooms passed through**, `2f7d18b37` against `1af780b72`:

| | `1af780b72` | `2f7d18b37` |
| --- | --- | --- |
| fps | 15.1–17.4 | 15.2–17.2 |
| GPU p95, ms | 141–179 | 147–180 |
| `beam` p95 | 56.5–64.9 | 59.7–66.1 |
| `beam` left, mean a cast | 150–206 thousand | 109–122 thousand |
| frames left over 250 thousand | 32–43% | 14–20% |
| most left in a frame | 439–450 thousand | 430–432 thousand |
| frame busy p50, p95, ms (with counts) | 32.5–54.0, 128–158 | 33.0–35.3, 130–161 |

It halves the frames that leave most, and the worst frame and the tail stand. Not read: what
those frames stopped on now.

The picture at rest, sky fixed, two runs each: `1af780b72` a / `2f7d18b37` a differ 8.0%,
0.04% over 8, mean 0.10, `1af780b72` a / `2f7d18b37` b mean 0.41, and the child's two runs
0.37. The parent's run b differs from all at 8.9: three near trees absent and the toast
"동행을 준비하고 있습니다" still up at 70 s. So a slow load, and no commit's. The "moved" view
turned 0.1 degrees, so a moving view at a boundary of detail remains uncompared here.

**Noise.** `world` and `face` trade by load, not commit: in one parent's load `world` 5.7–6.4
and `face` 15.7–18.2, in the next 9.2–9.4 and 6.6–7.0. Not traced. Thermal status reached 1 at
AP 48.7 °C after about an hour of this and again after each long round.

### What stops a whole frame: forgetting

The reads above of "Why rays are left" and "How deep" took the second of two frames after a
stall of a 25 MB copy, and missed most of this. Read instead without that copy, the record
zones as fast as they come, one distinct frame in about 3.5, and each frame's head beside them
(`tools/left4.mjs`, `MODE=scan`, `tools/left-why.patch`, `reports/scan-s23.json`), `2f7d18b37`
on the S23, a 6 s turn, 328 frames:

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
- The bricks are few: 3 to 40 bits a frame. No box or write was involved.
- The other whole-grid frames stopped on a node holding the eye, held as a Cell at coarser zooms
  (z22, side 4, asked [27, 27], held [25, 26]).
- Rays leave by no slot almost alone: 695,752 against 169 narrow, none at a Lacks under a blind
  split or by a reach.
- `sample` and `miss` rise with `beam` (correlation 0.64, 0.56), on nodes absent, not re-keyed.
- Beam rays left a frame: p50 1,263, p90 6,386, p95 6,848.

### A first load, and a moved brick that leaves a split

On the rebase over `main` at `430cdc831`, which reads `division` 9: runs of the evening at `pivot -`
were a build of format 8 refused its source, not the page, and with the harness's own faults (below)
they are no evidence. `tools/load.mjs` wraps the page's calls before its first script
(`reports/load-*.json`), seconds from navigation on the S23:

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
- The hand passes add 3 pipelines and no time. 10 `light_*` and 7 decor are made with shadows off.
- The slowest are `decor_cs`, `decor_footing_cs`, `face_cs`, `light_*` and `miss`, at 12–16 s, side by side, so
  a pipeline's own cost is not read here. Modules take 21–25 ms of calls for 5.1 MB.
- From the first compute pass to the toast gone: 0.3–1.0 s, up to 8 s in some. Not split.
- A minute was not seen, on either build, with the page's location answered or left unanswered.

**`b7457102e`'s fix on this rebase**, `9029f35dc`: on a moved brick a Split's key is left as held by no
zooms, not 0. Tip `981947702` against the tip with only the forget pass of before, two loads each, counts
off, no storage but the camera's cleared:

| | Without | With |
| --- | --- | --- |
| fps | 20.9, 21.4 | 22.6, 20.5 |
| GPU p50, ms | 24.3, 25.0 | 31.7, 27.2 |
| GPU p90, ms | 97.3, 103.4 | 74.3, 105.3 |
| GPU p95, ms | 149.5, 141.8 | 97.6, 140.8 |
| GPU most, ms | 377, 315 | 233, 227 |
| interval p95, ms | 174, 187 | 174, 198 |

Frame by frame with it (`reports/scan-forget-s23.json`, 153 frames): 1 whole-grid frame, where 30 of 328 were
before the fix on the earlier base. Of the 31 frames that moved a brick, 13 left few rays and 18 part
of the grid, none all. The parent's own scan did not draw, so that comparison is across bases. What
stays: 71 frames over 1,500 beam rays, nearly all stopped on a Cell near the eye at other zooms (zoom 20–23).

**What a harness must keep clear of, learned here.** Leaving a tab per run piled 32 on the phone and
took an hour's numbers low. A wait of 90 s failed a cold load, and clearing every store made each load cold.
A DevTools geolocation override alone can leave the page waiting: stub `navigator.geolocation`. And
`main` renames what a harness imports: a run that cannot turn the profiler on reads as "never drew".

**The picture of that fix** at rest, on the S23, sky fixed, one run each: mean difference 0.54 at the default view, 0.54 at 93 u, 0.30 at 541 u, 0.01–0.02% of pixels over 32. No more than a commit differs from itself between runs.

### What a Cell held at other zooms answers when asked again

`981947702` instrumented (`tools/left-cells.patch`, `tools/left5.mjs`, `reports/cells-s23.json`): for each node a beam ray stopped on, kept at other zooms, its slot's kind and asset before `beam_cs`, then the slot the frame wrote at the zooms asked. 285 frames, read whole, so each follows a stall of a 25 MB copy. 8,220 nodes held as a Cell and asked again, on 520,417 rays:

| Held near, against asked | Held | Nodes | Rays | Nodes changed | Rays on them |
| --- | --- | --- | --- | --- | --- |
| coarser | air | 1,830 | 249,014 | 17 | 199 |
| coarser | filled | 546 | 11,911 | 13 | 22 |
| the same | air | 2,214 | 167,410 | 2 | 46 |
| the same | filled | 865 | 12,872 | 0 | 0 |
| finer | air | 2,178 | 70,531 | 6 | 16 |
| finer | filled | 587 | 8,679 | 0 | 0 |

- 99.5% of nodes answer the same words: air to air 6,197 nodes, 486,694 rays, 94% of those compared; filled to filled 1,985.
- 38 nodes, 283 rays, changed: held `near` one coarser, air to a split 17, filled to a split 10, filled to air 3; held `near` one finer, air to filled 6; `far` one finer with `near` the same, air to a split 2.
- So asked finer can turn air into a split. Those nodes sit on a boundary of `zoom_at`, near 96 or 192 u in 33 of 38, at zooms 21–23 in 36.
- 118 more held air had no slot at the asked zooms when read, not compared.

Since 11-06 the node cache keeps a cell's answer once, whatever zooms it is asked at,
and a split's at the pair asked (`42d3340754`).

### The app against the page

The Android app at `e8d616ba4` on the S23, Release, against the page at the same commit, one load each, the same place and motion. The app runs wgpu and naga on Vulkan, with no browser. `tools/app-probe.patch` is what the scratch build changed: the probe, a fixed position, the kernel's and the shell's dev settings, and the surface's alpha below. `reports/app-s23.txt`.

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

- The app is not faster, so the browser is not what makes the phone slow. The app draws 19% more pixels and its frames were paced at about 25 ms, against the page's 16.6.
- **The app does not start on this phone without a change.** `xpute/rust/gpu/src/host/surface.rs:147` asks for `Opaque` where the S23's Vulkan surface offers only `Inherit`, and `Surface::configure` stops the kernel, `ENOTRECOVERABLE`. Since `432295c304` a surface that offers only `Inherit` configures.
- The app's picture 3 s after the circle shows the place with no tree in view, where the page has trees at about the same view. Not followed.
- The world showed at 24 s, the page's at 16 s. The app's was its first run after an install.

### The light's missed slots by hand

`e8d616ba4` against `355b65140`, alternating, one sitting, shadows on, a 25 s circle at noon, caches kept (`tools/w355b.sh`, `reports/light-miss-by-hand-s23.txt`).

| Run | fps | GPU p50 / p95, ms | `light_walk` p50 / p95, ms | Light `miss` a cast | Eye walked |
| --- | --- | --- | --- | --- | --- |
| `e8d616ba4` | 7.2 | 107 / 363 | 27.3 / 134.1 | 85,535 | 0.49% |
| `355b65140` | 8.8 | 102 / 289 | 6.8 / 28.6 | 96,657 | 0.45% |
| `e8d616ba4` | 7.2 | 114 / 332 | 25.2 / 102.2 | 106,691 | 0.52% |
| `355b65140` | 8.0 | 116 / 279 | 7.8 / 33.6 | 115,293 | 0.49% |

- About the same nodes take about 3.5 times less, so about 70% of the old `light_walk` was the generated shader. Roughly 285 ns a node against 75, from p50 and a cast's nodes in different frames.
- The eye's `sample`, `beam` and `miss` overlap across the commits. At rest the commits differ by a mean of 0.46 and 0.25, each from itself by 0.30 and 0.26. By eye, no wrong color, at phone size.
- Shadows off, a fresh load each, in that order: `e8d616ba4` 22.7 fps, GPU 30.7 / 100.7; `355b65140` 18.8, 38.2 / 133.9. Not told apart from a sitting's drift.
- Cold first load: 91 pipelines made, none failed, no console or GPU error. `light_miss_cs` from 397 KB and 12.5 s to 50 KB and 6.9 s; the wall stays 13–14 s.

### The light keeps its cells: what the eye paid

Alternating, one sitting, shadows on, a 25 s circle at noon, caches kept (`tools/pair.sh`, `reports/light-keeps-s23.txt`). Runs 1 and 3 are `f9f66682e`, 2 and 4 `e8d616ba4`.

| Run | fps | GPU p50 / p95, ms | `light_walk` p50 / p95, ms | Eye walked | Eye `miss` a cast |
| --- | --- | --- | --- | --- | --- |
| 1 | 2.6 | 239 / 1,054 | 283 / 831 | 0.64% | 83,933 |
| 2 | 7.0 | 124 / 371 | 20.5 / 105 | 0.49% | 45,525 |
| 3 | 2.2 | 331 / 1,172 | 536 / 918 | 0.60% | 82,128 |
| 4 | 6.6 | 132 / 346 | 24.9 / 111 | 0.42% | 39,459 |

- The eye does not pay in this view: it walks less with the light's cells kept.
- At rest, the two commits differ by a mean of 0.56, and each differs from itself by 0.50 and 0.59. By eye, no wrong color at rest or right after stopping, at phone size.
- Not run: a long drive, where the pyramid's slots run out.

### One sitting drifts

Shadows never on, a 25 s circle, in order (`tools/four.sh`): `06a46035c` 24.1 fps, `6e9a7cb83` 22.1, `e8d616ba4` 19.8, `06a46035c` 19.4, thermal status 0 throughout, every pass rising together. A run late in a sitting reads about 20% below an early one. With shadows off the light's walks do not run, so this says nothing of `e8d616ba4`'s cost to the eye, only that `6e9a7cb83`'s `Aim` shows nothing above the drift.

### Trees cast at noon

`e8d616ba4`, noon, shadows off, on, off, no camera move, 10 s each (`tools/tree.mjs`): on against off is a mean of 1.80, 5.55% of pixels over 8; off against off 0.40, 0.14%. The crowns darken the ground and water, not only walls. An earlier shot at `6e9a7cb83`, with no control, had been read as no shadow.

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

On this, `auto` took a third knob on 2026-11-07, the rate turns go in at
([a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md)).

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

The 90 s were later found to be the data's format and the harness
([the journal](../20261104-taking-rust-gpu-out/journal.md), "Did not work"): 19–29 s to
the world in "A first load, and a moved brick that leaves a split" above.

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
- A touch moves the camera: `Input.setIgnoreInputEvents` holds real input off a run's tab,
  and in-page events still pass.
- Clear the origin's storage before a load and log the camera: a view is kept by
  origin. Compare pictures only where the views match.
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

The hand walks and the native app were measured after this list was written: "The hand
kernel" and "The app against the page" above.

- The whole of the walk under the cliff: `sample_hit` is the samples' hits alone.
- The real page's other buffers, against what the synthetic did to its table.
- Why the S23 leaves 2–4 times what the Mac does at the same motion a frame.
- The picture of step 1a against `d25f5b631`.
- What of a node's time is the driver and Chrome's path.
- The same WGSL in the S23's native app.
- A hand-written walk.
- The distribution of a ray's nodes: only the mean a cast.
- Nodes at a lattice of 2, or in the Mac's drive.
- Another Adreno or Mali.
