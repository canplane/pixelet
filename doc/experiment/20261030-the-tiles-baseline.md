# The tiles' baseline

**2026-10-30.** Step 1 of [the screen keeps its tiles as the light
does](../minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md):
the numbers the tiles' cost is held to, taken on the line they land on
(`e6b740ab8`, the region with ages by square and the depth's bounds), and
the region split by what it holds (on `a0da7b17c`, which changes no part of
the renderer).

## How

`e6b740ab8` built into the session's own target, served on :5187 (its own
origin), Chrome on an Apple M-series Mac (Metal 3), one tab in the foreground,
a 2400×2250 canvas at a device pixel ratio of 2, the scale 1.00. The view the
page opens at, the companion and the puck in sight, clouds on, edges on,
corners' occlusion (not SSAO). Each setting switched, then 60 s let pass, then
the profiler's report over its 60 s window.

## What was seen

| run | frames by kind | GPU p50 | p95 | widest spans, p95 (ms) |
|---|---|---|---|---|
| noon, shadows | region 3601 of 3601 | 6.1 | 7.7 | finish 2.3, world 2.0, resolve 1.4, miss 1.1 |
| noon, shadows off | region 3593, px 8 | 3.7 | 4.5 | air 1.8, miss 1.1, world 0.9, resolve 0.8, finish 0.7 |
| 7:00, shadows | region 3601 of 3601 | 7.3 | 11.1 | finish 3.7, world 3.1, resolve 1.5 |
| night (00:30, real time), shadows | region 3589, px 11 | 5.9 | 8.1 | light_beam 10.4, finish 2.4, world 2.2 |
| zoomed out, noon, shadows | stands once loaded | — | — | — |

- Every third frame dearer with shadows off, seen on 5553036df, is gone: 8
  frames of 3601 drew whole.
- Zoomed out the companion and the puck fall out of sight; once the world
  had landed the page stood and drew nothing (the window held only the
  frames of the load). The zoomed-out region frame is so not measured here.
- At night the light's cast still runs in some region frames
  (`light_beam` p95 10.4 ms).

## The stencil's format

The adapter here offers `depth32float-stencil8` (with
`core-features-and-limits`, `timestamp-query` and others); the renderer asks
only for `timestamp-query`. So on this machine the stencil needs no change to
the depth's precision, only the feature asked for. The iPhone, the Quest and
Android are still to be checked.

## The region split by what it holds

Each contributor shrunk in turn on a build not kept, its sphere a point so
its draws stay bounded. Taking one out of `region::drawn` instead is no
measure: its draws still change the list, so with no sphere for them every
frame draws whole (the puck left out: px 3600 of 3600, 14.1 ms). A first run
on the shared checkout was cut off when another session's commit reloaded the
page under it; these were taken on a worktree of `a0da7b17c` no one else
edits, at a 2400×2138 canvas, noon, shadows on, every frame a region frame:

| build | GPU p50 | p95 |
|---|---|---|
| as it is | 5.8 | 6.8 |
| the puck's spheres a point | 6.2 | 7.7 |
| the actor's sphere a point (and its shadow with it) | 4.9 | 7.0 |
| the squares still gathering left out | 5.8 | 7.5 |
| the shadow's squares left out | 5.0 | 7.1 |
| the cumulus patches' spheres a point | 6.0 | 7.5 |

Runs of the same build differ by about 0.4 ms at the p50, so only the actor
and its shadow stand out: about 0.9 ms, nearly all of it the squares its
shadow is taken to fall on. The puck, the cumulus at this view and the
squares still gathering add nothing seen. Most of a region frame's 5.8 ms is
what it costs whatever its size.

## Step 2: the stencil carried

The world's surfaces draw into `depth32float-stencil8` where the device
carries it, `depth32float` where not (pxl-render gpu/api.rs
`Gpu::world_depth`), and every pipeline drawing into them is built onto that
depth (gpu/pipeline.rs `Pipelines::build_world`); the light's map, the
raster, the occlusion and the screen's depth keep `depth32float`. The resolve,
the finish and the depth's bounds read the depth through a view of it alone
(`Surface::depth_only_view`). The page and the device app ask for the
feature where the adapter has it (ts/app/web/src/webgpu.ts, the native
bracket's `ASK`). No stencil is written or tested yet.

On :5187 at a 4112×2226 canvas, a 30 Hz display, noon, shadows on, the view
the page opens at, built with the stencil and built with it forced off, in
turn, the same window:

| build | frames | GPU p50 | p95 |
|---|---|---|---|
| the stencil carried | region 1743, px 57 | 5.8 | 9.2 |
| the stencil carried, again | region 1801 of 1801 | 5.5 | 6.9 |
| forced to `depth32float` | region 1800 of 1800 | 5.7 | 8.0 |

No cost seen; the 57 whole frames of the first run fell in its settling.
The picture as before, and no WebGPU error in the page's console.

## Step 3, in parts (under way)

Parts 1–4 built (the words marked part by part; the resolve, the write-back
and the finish as the tiles' quads; the world's pass through the gate's
stencil, and the rays' screen pass as the tiles' quads). On :5187 at a
3728×2098 canvas, the resolution fixed (`고정`), noon, shadows on, the view
the page opens at; every run region 3592–3601 of about 3600:

| build | GPU p50 | p95 | `world` p95 | `finish` p95 |
|---|---|---|---|---|
| step 2 (`c4cbb4b64`) | 4.6 | 5.3 | 1.4 | 1.4 |
| parts 1–2 | 4.0, 4.1 | 4.3, 5.0 | 1.4 | 1.1 |
| parts 1–3 | 3.9 | 5.0 | 1.2 | 1.0 |
| parts 1–4, the rays' pass whole under the stencil | 14.9 | 17.6 | 8.2 | 7.9 |
| parts 1–4, the rays' pass as the tiles' quads | 5.6, 5.3 | 6.6, 6.2 | 2.3, 2.4 | 1.6 |
| the same, the claimed tiles' rectangle as a scissor too | 5.2 | 6.0 | 2.2 | 1.5 |
| the same, the draws testing no stencil | 5.2 | 6.0 | 2.2 | 1.5 |

- The rays' screen pass writes its depth and discards, so a stencil cannot
  reject its fragments early: drawn whole under it, it shaded the whole
  screen. As the tiles' quads it draws the claimed tiles alone.
- Part 4 still costs about 1.2 ms over parts 1–3, and neither the stencil
  test nor rasterizing past the claimed tiles is it (the two rows above).
  Probes on part 4, each against parts 1–2 at 4.1 ms in the same window:
  no gate and the scissor, 4.9 ms; that and the rays' pass one triangle
  again, 4.8; that and the stencil loaded rather than cleared, 3.9, but in
  a run whose frames moved 174 times against about 2270 in every other, so
  not comparable. The gate writing each gated frame's number into the
  stencil, kept between frames and cleared only where the numbers come
  round (as a light tile's word names its cast), 5.5. Not yet found.
- With the resolution not 1 (`자동` falling to 0.84 under load, or
  `저해상도`), and shadows on, every frame is drawn whole on step 2 as on
  parts 1–2 (px 3600 of 3600; with shadows off, region 3600 of 3600): the
  depth's bounds never land (`DepthBounds::landed`), from the line before.
  Reported to that line's session; step 5 replaces the path.

Part 5's first half (the pixels' cast dispatched whole, each workgroup
returning on an unclaimed tile, its first pixel gone from the eye's words;
the fallback without a stencil): region 3602 of 3602, GPU p50 5.6 ms, p95
6.6, the same window, against step 2 at 4.7 ms in that window. Runs where the
frames moved only 21–70 times against about 2300 drew every frame whole
(px), on step 2 as on this; their frame times are no measure of the change.
A resize, twice, first stopped the guest (the rays' screen pass's group made
over the rays the resize let go); fixed, both resizes then drew right.

**The hour.** A rebuild reloads the page, and a reload sets the hour back
to the real time. From the second build measured this afternoon until part
5.1, the hour was not set again: the rows of part 5's second half, part 6
and step 3 against step 2 below were taken at the real time, between 14:20
and 15:55 KST, not at noon, each comparison's runs alike in it. From part
5.1 on, noon is set after each reload.

Part 5's second half, the samples' raster, which drew within the region's
rectangle (a scissor). Its span (`sraster`) in the region frames, the same
setup, each row a build not kept but the last, from the per-frame record
(`profiler_frames`):

| build | `sraster` p50 | p95 | GPU p50 |
|---|---|---|---|
| within the rectangle (part 5's first half) | 0.79 | 2.49 | 5.8 |
| drawn whole, no scissor | — | 2.8 | 7.4 |
| through the stencil, gated as the world's pass | — | 4.3 | 8.3 |
| the stencil's depth and test, no gate | — | 2.9 | 7.9 |
| drawn whole, the boxes about the eye left out | — | 3.0 | 7.8 |
| drawn whole, every fragment discarded at once | — | 0.3 | — |
| each fragment returning on an unclaimed tile | 2.36 | 3.21 | 8.2 |
| through a gate of depth (kept) | 0.59 | 0.66 | 7.0 |

- Drawn whole it costs its fragments' shading, not the rasterizing: every
  fragment discarded at once, it is 0.3 ms; the boxes about the eye, drawn
  over the whole screen, are not it.
- The stencil is no gate here: the boxes' fragment shader discards and the
  depth is written, so the stencil is tested after the shading, and the
  gate's writes add to it.
- A fragment returning on an unclaimed tile reads the tile's age first, and
  the boxes overlap many times over: 2.4 ms, though the claimed tiles were
  about a tenth of the screen (the pixels' cast, `px`, 0.13 ms in the
  region frames against 1.31 drawn whole). Discarding every fragment where
  the frame is not whole, a uniform's word, gave 0.33 at the p50, 0.39 at
  the p95: the instrument read the region frames.
- A gate of depth, the raster's depth cleared to the nearest and the gate
  setting the claimed tiles to the far plane, rejects them early here: 0.59
  at the p50, under the rectangle's 0.79 in a window that claimed less (its
  `px` at 0.26 against 0.39). The frame's GPU time follows how much is
  claimed in each window, which the wandering companion changes run to
  run, so the rows' GPU times compare loosely; the spans of each run, not
  its total, say what each build does.
- Picture right, and twice resized, no error.

Part 6, `Region`, the scissors, the rectangles and the whole-by-share rule
removed, the frame kind `region` now `gated`, the depth's bounds taking the
claimed tiles: gated 3591, px 10 of 3601; GPU p50 6.1 ms, p95 7.7;
`sraster` p50 0.59, `px` 0.33, `world` 1.84, `resolve` (the depth's bounds
with it) 0.72. Picture right, no error.

Step 3 against step 2 (`aa08395c0`, the line rebased), the same window,
each built in turn on the same checkout, every frame but some ten a region
or gated frame:

| build | GPU p50 | p95 | `world` p50 | `px` p50 | `sraster` p50 | `finish` p95 | `miss` p95 |
|---|---|---|---|---|---|---|---|
| step 3 | 6.1 | 7.7 | 1.84 | 0.33 | 0.59 | 1.9 | 0.7 |
| step 2 | 10.9 | 14.0 | 2.16 | 0.59 | 0.33 | 4.1 | 4.2 |
| step 3 | 6.3 | 7.9 | 1.84 | 0.33 | 0.59 | 2.0 | 0.8 |
| step 2 | 10.9 | 14.3 | 2.16 | 0.59 | 0.33 | 4.3 | 4.3 |

- The afternoon's real hour, its shadows longer, and the scene busier than
  the baseline's noon (step 2 at 10.9 ms against 4.6), the companion and the puck's beacon among cars
  near the view's middle; at the baseline's quieter view, part 4 cost about
  1 ms over parts 1–3, above. Here step 3 is the cheaper by 4.6 ms at the
  p50: step 2's rectangle about everything moving and gathering held most
  of the screen, and its pixels' misses and finish followed it.
- Only the raster is dearer than within the rectangle, by the gate's draw
  and the boxes it still rasterizes; the rest falls.
- Measured at the afternoon's real hour alone, at this view and scale;
  noon, 7:00, night, the clouds moving, zoomed out and scales other than 1
  are not yet taken against step 2.

## Why some runs drew every frame whole

Runs whose frames moved only 21–70 times drew every frame whole (px), on
step 2 as on step 3; they followed a resize or a scale other than 1. A probe
not kept wrote, each frame, which of the frame's conditions held, into a lane
the profiler records (`pack_bytes`), read back from `profiler_frames()`.

- Before a resize: region 2443 of 2456, its bits a caster moving, the lists
  moved, tiles gathering; the depth's bounds landed.
- After resizing the window, every frame px (721 of 721): a caster moving,
  `regions` finding no bound (unbounded), no region; the clouds' sheets not
  among it. The depth's bounds: a map counted as out, nothing copied,
  nothing ever landed. `DepthBounds::take` makes the read buffer and the
  landing anew on a resize but does not clear `mapped`; the map of the
  buffer let go never lands, so `copy` never runs again.
- The frames were woken by the timer (`REDRAW`, the resolve gathering) far
  more than by the kernel's next step, which is what the report counts as
  moving: the few moving frames follow from the whole frames, not the other
  way round.

## Step 5, in parts (under way)

Part 1, the screen's words the GPU's (`changed_at`, the frame each tile last
changed in), the host handing its marks and a pass setting them first in the
frame (screen_mark.wgsl, span `mark`, a span added at the list's end, the
stream's head four words longer). The same setup:

| run | frames | GPU p50 | p95 | `mark` p50 | p95 |
|---|---|---|---|---|---|
| noon | gated 3601 of 3601 | 5.4 | 6.6 | 0.07 | 0.13 |
| 17:00, real time (the page reopened, the time not set) | gated 3589, px 12 | 6.8 | 9.2 | 0.26 | 0.26 |

- At 17:00 the shadows are long and every square they fall on is a mark
  of its own, each tested by every tile's thread; part 3 replaces those
  squares by the reach.
- Picture right, no error. The new compute entry first stopped the kernel
  until it was named in the record's table (record.rs `compute_entry`).

Part 2's first half, the screen's words, uniform and marks made one type
(gpu/tiles.rs `Tiles`), the uniform's flag for whole now `claimed_for`, the
frames a tile stays claimed: noon, gated 3601 of 3601, GPU p50 5.6 ms, p95
6.3, `mark` p50 0.07; as part 1. Picture right, no error.

Part 2's second half, the light's map through its gate: the map's depth with
the stencil, its tiles (`Tiles`, 32 texels, claimed the frame they changed)
marked by each caster that moved, the gate setting them back to the far
plane (light_gate.wgsl), the world laid in as their quads, the casters
through the stencil; the map's rectangle and scissor removed:

| run | frames | GPU p50 | p95 | `shadow` p50 | p95 | `mark` p50 |
|---|---|---|---|---|---|---|
| noon | gated 3600 of 3600 | 6.0 | 6.9 | 0.33 | 0.39 | 0.07 |
| 7:00 | gated 3601 of 3601 | 7.4 | 9.8 | 0.26 | 0.39 | 0.20 |

- Within the rectangle the map's pass was 0.2 ms at the p95 at noon (part
  1's runs); through the gate it is 0.4, the gate's and the lay's quads
  drawn over the map's 4096 tiles.
- Picture right at noon and at 7:00, no error; a caster moving in the open,
  its long shadow followed frame to frame, was not seen in these runs: the
  companion stood behind the car.

Part 3, the reach (screen_reach.wgsl, gpu/reach.rs), the depth's bounds,
`stood` and the light-plane test removed, the host counting a frame it
reached in as one it handed a mark in (`Tiles::mark`'s `reached`). Noon, a
3728×2210 canvas, against part 2 in the same window:

| build | frames, woken by the kernel / the timer | map pass in | GPU p50 | p95 | `mark` p50 |
|---|---|---|---|---|---|
| part 2 (`913627aa4`) | gated 3600; 2308 / 1292 | 2316 frames | 6.0 | 7.0 | 0.07 |
| part 3 | gated 3601; 67 / 3534 | 2986 of 3000 | 10.4 | 14.0 | 0.72 |
| part 3, the reach writing nothing (a probe) | gated 3600; 98 / 3502 | 3598 | 7.9 | 9.8 | 0.98 |

- The frames never stand: a caster moves each turn somewhere in the map
  (the cumulus, which casts and which the wind carries, whether or not its
  shadow falls on the screen), so the reach runs each frame; the host,
  which cannot see what it marked, counts a mark each time, so the tiles
  seem to gather for ever, the timer wakes the next turn (`REDRAW`), and
  that turn's casters move again. The probe shows it is the count, not
  what the reach marks: with no write the loop holds.
- The reach itself costs about 0.9 ms a frame here (`mark`, with the
  marks): every pixel's depth read and taken into the map.
- The minutes left to step 6 how the host knows whether any tile gathers
  once the marks are the GPU's; the reach asks it already.

**Corrected.** The loop above was misread. A probe not kept wrote, each
frame, how many spheres moved, how many cast and the largest radius
(`pack_bytes`): on part 2 as on part 3, every frame of the window had 2 to
6 moved spheres, 2 to 4 of them casters, of radius about 1 (the companion
and the puck, whose draws change each turn). Part 2 too drew every frame
gated, the map's pass in each; the two differ in which wake is counted
first (the kernel's or the timer's), not in how many frames are drawn.
What part 3 costs is the reach and what it claims:

- the reach itself, about 0.7 ms a frame;
- the screen tiles it claims, by the map's tiles of 32 texels: every pixel
  whose taps read a tile a caster touched, the whole of that tile's
  footprint, where part 2 marked the squares the light-plane test found
  (`px` p50 0.52 against 0.33, `resolve` and `finish` p95 3.3 against 1.0
  and 1.6).

Part 3 with the host reading back what the reach marked (`reached`):
gated 3600, GPU p50 10.2 ms, p95 14.1, `mark` 0.72; the same cost.

The map's tiles at 8 texels, the light's cast's tile, by the user's word
once the reach's claim was measured; noon, the same window, against step 2:

| build | frames | GPU p50 | p95 | `px` p50 | `world` p50 | `shadow` p50 | `mark` p50 |
|---|---|---|---|---|---|---|---|
| step 2 (`aa08395c0`) | region 3601 | 4.6 | 5.5 | 0.20 | 1.18 | 0.20 | — |
| part 5.1 | gated 3601 | 5.4 | 6.6 | 0.33 | 1.84 | — | 0.07 |
| part 5.2 | gated 3600 | 6.0 | 7.0 | 0.33 | 1.84 | 0.33 | 0.07 |
| part 5.3, the map's tiles 32 texels | gated 3600 | 10.2 | 14.1 | 0.52 | 2.49 | 0.26 | 0.72 |
| part 5.3, 8 texels | gated 3601 | 8.8 | 12.3 | 0.39 | 1.77 | 0.79 | 1.44 |

(Parts 5.1 and 5.2 from their own windows above, the same view and hour.)

- At noon in this window step 2 is the cheapest: every part of step 5 is
  slower than the baseline, by 0.8 ms (5.1) to 4.2 ms (5.3 at 8 texels).
  The afternoon's comparison, step 3 under step 2 by 4.6 ms, was at the
  real hour, with longer shadows, where step 2's rectangle held most of the
  screen; at noon its rectangle is small.
- At 8 texels the reach claims less (`px`, `world` down) but the map's
  65,536 tiles cost in the marks' pass (a thread a tile) and in the gate's
  and the lay's instanced quads (`shadow` 0.79).

**Part 3's costs, predicted before the change.** At 8 texels, noon:
- the reach, nearly all of `mark`'s 1.44 ms: each pixel taken into the map
  and up to nine of its tiles' words read. Judged a square of 16 pixels at a
  time by the bound of its depths (the 8 corners of the slab between its
  least and most depth, taken into the map, which an affine light keeps
  within their box), the matrices and the words read fall sixteenfold, the
  depths read stay: to about 0.3 ms;
- the marks' pass, a thread every tile of 65,536 for the map: a thread a
  tile of each mark's tiles instead, every tile only where the frame is
  drawn whole, on the screen and the map alike: under 0.05 ms;
- the map's gate and lay, an instance a tile over all 65,536: the map's
  claimed tiles are exactly this frame's marks' (it claims the frame it
  changed alone), so a quad a mark's tiles draws them exactly, every tile
  where the map is drawn whole: `shadow` from 0.79 to about 0.3 ms.

**Part 3's costs, measured after each change** (noon, the same setup):

| change | GPU p50 | p95 | `mark` p50 | `shadow` p50 | `world` p50 | `px` p50 |
|---|---|---|---|---|---|---|
| before (8 texels) | 8.8 | 12.3 | 1.44 | 0.79 | 1.77 | 0.39 |
| the reach a square at a time | 9.0 | 11.1 | 0.72 | 0.92 | 2.03 | 0.39 |
| and the marks by mark, the map's gate and lay a quad a mark | 8.5 | 9.6 | 0.72 | 0.20 | 2.16 | 0.39 |
| and no array of depths in the reach | 8.8 | 10.4 | 0.79 | 0.20 | 2.23 | 0.46 |
| a probe: the reach reading the depths and returning | 7.2 | 8.0 | 0.66 | 0.20 | 1.90 | 0.33 |

- The map's pass came to 0.20 ms as predicted (0.3).
- The marks' pass was not the reach's cost: `mark` stayed at 0.72.
- The reach's cost is its reading of every pixel's depth, 0.66 ms of its
  0.72, not its sums or the words it reads: the prediction of 0.3 took the
  sums for the cost and was wrong. Removing it asks for a depth's bound kept
  a square at a time on the GPU, set again in the tiles drawn, which is
  state the design does not hold yet.
- `world`, `resolve` and `finish` follow what is claimed, which differs
  from window to window with the companion; the square's slab claims a
  little more than each pixel did.

**The depth's bound kept for the reach** (gpu/reach.rs `set_bounds`,
screen_bound.wgsl), set after the world's pass in the tiles it drew, read
by the reach in place of every pixel's depth; predicted `mark` from 0.72
to about 0.1 ms and 0.05 ms more for the bound. Noon, a 3728×2226 canvas
(the window moved back to the display of ratio 2; a first run at ratio 1,
1864×993, and one with the settings not yet set again after a reload are
not counted), main at `127a83946` with the change uncommitted:

| run | frames | GPU p50 | p95 | `mark` p50 | `resolve` p50 | `world` p50 | `px` p50 |
|---|---|---|---|---|---|---|---|
| the bound kept | gated 3598, px 3 | 7.9 | 8.8 | 0.46 | 0.33 | 2.49 | 0.46 |

- `mark` fell 0.26 ms, not 0.6: the prediction missed again. What is left
  is not yet split; a square whose slab spans more than 16 of the map's
  8-texel tiles (a silhouette, far ground) is still asked a pixel at a
  time, off the depth.
- `resolve`, with the bound's pass in it, 0.33.
- The window claimed much (`px` 0.46, `world` 2.49); the claimed area, not
  the reach, is now the larger part of the gap to step 2 (4.6 ms, `world`
  1.18, `px` 0.20).
- Twice resized, gated frames again, no error; picture right at noon, the
  companion in the open.

**What the claimed area is made of** (main at `3bfcdc7d0`, noon, a
3840×2098 canvas, the browser profile `lof-mac`). A probe not kept counted,
each frame, the screen tiles claimed, those changed this frame (the bound's
pass, `tile_age` 0) and those the host's marks cover (on the host); the
reach's share is the changed less the host's.

| run | claimed | changed this frame | the host's marks | GPU p50 | p95 | `world` | `finish` | `px` | `mark` |
|---|---|---|---|---|---|---|---|---|---|
| as committed | 527 | 526 | 360 | 7.3 | 8.5 | 2.23 | — | 0.39 | 0.52 |
| a caster's mark not grown by the taps (kept) | 520 | 519 | 130 | 7.1 | 8.9 | 2.16 | 1.90 | 0.39 | 0.52 |
| and the map's tiles 4 texels (not kept) | 460 | 459 | 130 | 7.5 | 9.1 | 2.03 | 1.77 | 0.39 | 1.38 |
| nothing marked, no reach (a probe) | 0 | 0 | 0 | 2.3 | 3.5 | 1.25 | 0.72 | — | 0.07 |

- The same 520-odd tiles change every frame: the companion and the
  puck's draws change each turn where they stand. Nearly all of the claim
  is fresh, not the gathering's trail.
- A caster's screen mark grown by the receiver's taps was the shadow's
  near the caster, which the reach now finds from the map: not growing it
  takes the host's marks from 360 tiles to 130, and the reach then claims
  nearly the same tiles itself (520): it was work done twice, and is gone.
- The map's texel is 0.125 u, its tile of 8 texels 1 u; at this view 1 u
  is about 100 pixels, three screen tiles. Four spheres of radius 1, where
  they were and are, cover some 500 tiles: the claim is the spheres' size,
  not the map's tiles (4 texels claim 460 and cost the reach 0.9 ms more).
- A gated frame claiming nothing costs 2.3 ms here: `world` 1.25 (every
  draw's vertices, its fragments passed by the stencil) and `finish` 0.72
  (the kept picture shown whole). Claiming 520 tiles adds about 2 ms of the
  cast's passes, which run once any tile is claimed, and 2 to 3 ms of
  shading in the world's pass, the resolve and the finish: what is claimed
  costs well past its share of the screen.

**Another GPU application ran** from about 20:42 to 20:47 KST, the user
said; the runs before it stand. In it fell two probes of step 6.3, its
marks where the actors were left out (4.1 ms) and drawn with the palette
as it is (5.8), which so say nothing. Step 6.3 against the commit before
it, noon, 3728×2226, the companion walking (the kernel waking some 250
frames a minute) and then standing (every frame woken by the timer):

| time | build | frames | GPU p50 | p95 | `world` | `finish` | `resolve` | `px` | `mark` |
|---|---|---|---|---|---|---|---|---|---|
| 20:35, walking | step 6.3 | gated 3601 | 8.3 | 10.0 | 2.29 | 2.10 | 0.72 | 0.39 | 0.66 |
| 20:38, walking | before it (`0fa704375`) | gated 3601 | 6.9 | 9.7 | 2.16 | 1.84 | 0.26 | 0.39 | 0.52 |
| 20:40, walking | step 6.3 | gated 3601 | 7.9 | 9.8 | 2.29 | 2.03 | 0.46 | 0.39 | 0.66 |
| 20:48, standing | step 6.3 | gated 3601 | 4.1 | 6.0 | 1.18 | 1.18 | 0.13 | 0.20 | 0.39 |
| 20:50, standing | before it | gated 3600 | 4.0 | 5.6 | 1.18 | 1.18 | 0.13 | 0.20 | 0.26 |

- With the companion walking, step 6.3 costs about 1 ms more: its marks
  claim more than its sphere did (`resolve`, `finish`, `world` up), not
  less. Why is not yet found; the actors' marks where they were, drawn off
  the palette copied the frame before, are the first suspect, not checked.
- With it standing, the two are alike, 4 ms: little changes.

The probes again, one window, noon, 3728×2226, the GPU free, many draws
changing each turn (`px` 0.4–0.46):

| build | GPU p50 | p95 | `world` | `finish` | `px` | `mark` |
|---|---|---|---|---|---|---|
| step 6.3 | 7.1 | 8.2 | 2.36 | 1.84 | 0.46 | 0.66 |
| step 6.3, no marks where the actors were (a probe) | 7.1 | 8.3 | 2.23 | 1.90 | 0.46 | 0.66 |
| before it (`0fa704375`) | 6.8 | 8.0 | 2.23 | 1.77 | 0.39 | 0.52 |

- The marks where the actors were are not the cost: without them the
  frame costs the same.
- Step 6.3 costs some 0.3 ms over the commit before, its marks' pass
  (`mark` 0.14) and a little more shading; the 1 ms seen at 20:35–20:40
  was in part the windows' scenes differing.
- It claims no less: the claim is the reach's, the shadows found from the
  map's tiles the casters' spheres mark (part 3's record), which step 6.4
  would take from the drawn casters instead.

**Step 6.4, the actors' shadows marking the map's tiles by their drawn
selves**, drawn from the light as where they were and are, through the
same mark, the map's tiles' words set by their fragments; the actors'
spheres' rectangles kept for the map's gate and lay alone (`Tiles::mark`'s
`beside`). The same window, noon, 3728×2226, the companion walking (the
kernel waking some 280 frames a minute):

| build | frames | GPU p50 | p95 | `world` | `finish` | `resolve` | `px` | `mark` |
|---|---|---|---|---|---|---|---|---|
| steps 6.3 and 6.4 | gated 3601 | 6.2 | 8.1 | 1.64 | 1.25 | 0.13 | 0.33 | 0.66 |
| before them (`0fa704375`) | gated 3601 | 8.0 | 10.0 | 2.29 | 2.03 | 0.52 | 0.39 | 0.52 |
| steps 6.3 and 6.4, again (the view moved: cast 138) | gated 3388 | 5.7 | 7.7 | 1.51 | 1.05 | 0.20 | 0.26 | 0.66 |

- About 2 ms under the commit before at the p50: the shadows' claim is
  now where the actors' shadows change, not where their spheres' map
  rectangles reach, and the world's pass, the finish and the resolve fall
  with it. The marks' passes cost 0.14 ms.
- Picture right at noon, no error.

**The baseline again, the GPU free** (21:27–21:35), the step 2 commit
(`aa08395c0`) and main (`fc60c28ca`) alternated in one window, noon,
3728×2338 (the last run 3728×2226):

| build | frames | GPU p50 | p95 | p95 `world` | `resolve` | `finish` | `sraster` | `px` | `mark` |
|---|---|---|---|---|---|---|---|---|---|
| step 2 | region 3548, 2184 timed | 4.8 | 5.7 | 1.5 | 1.1 | 1.7 | 0.7 | 0.7 | — |
| main | gated 3601, all timed | 5.7 | 6.9 | 2.0 | 0.7 | 1.5 | 0.9 | 0.9 | 1.2 |
| step 2 | region 3559, 2190 timed | 4.8 | 6.6 | 1.6 | 1.3 | 2.3 | 1.1 | 1.2 | — |

- Main is about 0.9 ms over the baseline at the p50: the `mark` span
  (the marks, the actors' marks from the eye and the light, the reach)
  and the world's pass.
- The two time different frames: the baseline's reads time some 62 % of
  its frames, main's all of them.

**The map drawn in its claimed tiles alone** (21:40–21:47): the map's
gate and lay draw the actors' spheres' rectangles a tile each, the
unclaimed collapsed (screen_tiles.wgsl `mark_clip`, `Tiles::beside`), in
place of a quad each. One window, noon, 3728×2338, gated frames all:

| build | GPU p50 | p95 | p95 `shadow` | `mark` |
|---|---|---|---|---|
| the change | 5.8 | 7.1 | 0.3 | 1.2 |
| main (`fc60c28ca`) | 5.6 | 6.9 | 0.3 | 1.1 |
| the change, again | 5.6 | 6.6 | 0.3 | 1.0 |

- No cost either way: the actors' rectangles hold a few hundred of the
  map's tiles. The picture at 7 o'clock the same.

**The `mark` span taken apart** (22:10), its passes stamped to spans no
gated frame at noon runs (a probe, reverted), noon, 3728×2226:

| pass | frames | p50 | p95 |
|---|---|---|---|
| the marks (screen and map, compute) | 343 | 0.07 | 0.20 |
| the actors' marks from the eye | 3353 | 0.13 | 0.26 |
| the actors' marks from the light | 3196 | 0.07 | 0.20 |
| the reach | 3377 | 0.46 | 0.66 |

The timestamps step by some 0.066 ms here. Within the reach, by probes:
returning once the square's bound is read leaves nothing of it; dropping
the per-pixel fallback alone takes some 0.26 ms off.

**The reach skips what lies off the marks' box**: the host hands the box
of the map's tiles the frame's marks cover (`Tiles::marks_box`), where no
word changed off it, and each screen tile first takes its whole slab, its
least and most depth gathered from its squares' bounds in the workgroup,
into the map; off the box, the tile is done; each square likewise before
its own box or its pixels. The same window, gated frames, noon:

| build | `mark` p50 | p95 | GPU p50 | p95 |
|---|---|---|---|---|
| the change | 0.26 | 0.59 | 4.8 | 6.6 |
| `c03a389e4` | 0.59 | 0.79 | 5.4 | 6.0 |
| the change, again | 0.33 | 0.72 | 6.2 | 8.1 |

- The reach falls from some 0.4 ms to under 0.1; the rest of `mark` is
  the actors' marks. The frame's total moves with the scene more than by
  this (the last run's `world` and `finish` p95 2.5).
- The picture at 7 o'clock as before, no error.

**The baseline against main after the reach** (22:47–22:58), the step 2
commit (`aa08395c0`) and main (`46f776afa`) alternated in one window, the
GPU free, noon, 3728×2226:

| build | frames, timed | GPU p50 | p95 | p95 `world` | `resolve` | `finish` | `shadow` | `mark` |
|---|---|---|---|---|---|---|---|---|
| main | gated 3591, 3569 | 4.8 | 5.4 | 1.7 | 0.5 | 1.3 | 0.3 | 0.5 |
| step 2 | region 3600, 2553 | 5.3 | 6.7 | 2.1 | 1.6 | 2.4 | 0.8 | — |
| main | gated 3599, 3598 | 4.9 | 5.8 | 1.8 | 0.5 | 1.5 | 0.3 | 0.7 |
| step 2 | region 3601, 1846 | 4.7 | 5.6 | 1.4 | 1.0 | 1.5 | 0.3 | — |

- Main is at the baseline in this window, within what the baseline itself
  moves between runs (4.7–5.3 at the p50), and its p95 no worse.
- The baseline's reads time half to seven tenths of its frames, main's
  all of them: which frames go untimed there is not known, so the
  baseline's numbers may leave some out.

**The actors' marks' passes** (23:02), a probe (reverted): the eye's pass
with its attachment and no draw takes one step of the timestamps (0.066
ms) at the p50, against 0.13 with its draws; the light's, with its draws,
one step. What attaching the world's depth costs is under a step: a
cheaper attachment would save less than 0.07 ms, not worth a pipeline and
a target of its own.

**The clouds' marks** (23:05): zoomed out ten steps from the start, the
clouds on, noon, main: 26 frames in 65 s, the rest none, as nothing the
kernel bounds changed; the clouds marked no frame. With the noon window's
343 frames of 3600 that the marks' pass ran in, the spheres' marks left
(the puck's and the clouds') claim next to nothing in either window:
moving them to their drawn selves would gain nothing measured here. The
frames drawn whole after the zoom cost 18 ms at the p50 (`air` 12.5,
`light_beam` 14.7 at the p95): the casts, not the tiles.

**The camera moving** (23:10–23:28): a wheel event at the canvas's middle
every 16 ms, in for 30 and out for 30 by turns, for some 70 s from the
start view, noon; every frame drawn whole (`cast`). One window:

| build | canvas | frames, timed | GPU p50 | p95 |
|---|---|---|---|---|
| main (`fc7c981c4`), first after a reload | 3728×2338 | 1318, 318 | 56.5 | 73.2 |
| step 2 (`aa08395c0`) | 3728×2226 | 1739, 353 | 45.7 | 52.6 |
| main (`fc7c981c4`) | 3728×2226 | 1779, 356 | 43.6 | 50.3 |

- At the same canvas, main is at the baseline while the camera moves: the
  tiles' work neither costs nor gains there. The first run, a taller
  canvas and the first after a reload, is left out.
- The frame is the casts': at the p95, main's `miss` 9.7, `light_beam`
  8.5, `sample` 6.9, `air` 6.5, `finish` 5.5, `world` 4.2, `beam` 3.7,
  `fill` 2.8, `mark` 1.7 ms (the first pass of a loaded frame, which may
  hold the wait before it).

**The camera orbiting** (23:31), main (`26b828e73`): a pointer held at the
canvas's middle and swung 300 px each way on a sine, an event every 16 ms,
for some 70 s, noon, 3728×2338; frames drawn whole (`cast`) but a few:

| motion | GPU p50 | p95 | p95 `light_beam` | `finish` | `miss` | `world` | `sample` | `air` |
|---|---|---|---|---|---|---|---|---|
| orbit | 34.5 | 36.1 | 9.4 | 6.1 | 5.4 | 4.8 | 2.0 | 1.0 |
| zoom (above, 3728×2226) | 43.6 | 50.3 | 8.5 | 5.5 | 9.7 | 4.2 | 6.9 | 6.5 |

- The light's cast is as costly orbiting as zooming: the detail that
  follows the eye's distance was taken to be what moves it, and an orbit
  keeps the distance. What it casts each frame, and why, is not yet
  counted (the tally, the `ray_counts` setting, counts its slots).
- `finish` costs some 6 ms in a frame drawn whole, against 1.3 at the p95
  through the gate.

**What the light casts while the camera orbits** (23:50), the `ray_counts`
setting on, the same orbit for 30 s, main: of 213 counted frames the light
cast in all 213; its slots that took the visible sample some 632,000 a
frame and those that walked some 11,000, some 15 % of the map's 4,194,304
(`light_rays` is the host's figure, the whole map each frame it casts, not
what was cast). The eye's place is read into what a tile holds
(gpu/ray/cast.rs, `light_content`: `eye::ORIGIN..eye::STEPS`), so a moving
eye begins an epoch each frame and every tile a receiver reads is cast
anew: the visible receivers' tiles, every frame the camera moves.

**The light's tiles held while only the eye moves** (2026-10-31, 00:20–00:36),
`light_detail_cs`: the eye's place and the light's words out of what a
tile holds; the box's corner a whole number of tiles, the reads moved with
it by the host, a tile the box slid onto and one whose slots' zooms the
eye now asks otherwise (`Light::asks`) set to be read anew. The same orbit,
noon, 3728×2338:

| | slots cast a frame (took, walked) | `light_beam` p95 | GPU p50 |
|---|---|---|---|
| main (`26b828e73`, above) | 632,000, 11,000 | 9.4 | 34.5 |
| the change | 87,000, 1,400 | 8.3 | 32.7 |

- The slots cast fall by some 86 %, the frame by some 1.5–2 ms.
- `light_beam` is the receivers' marks, not the cast: split into spans (a
  probe, reverted), orbiting, the marks (`light_mark_cs`,
  `light_mark_drawn_cs`) 7.1 ms at the p50, the beam and the cells 0.8,
  the reach 0.07, `light_detail_cs` 0.07.
- Stopped after an orbit at 7 o'clock, and drawn again from a fresh epoch
  (the shadows off and on): no difference seen.

**The receivers' marks read each pixel once** (00:40–00:50),
`light_mark_cs`: a thread's square of pixels, `light::MARK` a side, read
its pixels and its neighbors' tiles once into an array, 30 a thread (a
column past and a row either side), where each pixel read its own and its
four neighbors' anew, 80; the same marks. The orbit, noon, 3728×2338, one
window:

| build | `light_beam` p50 | p95 | GPU p50 | p95 |
|---|---|---|---|---|
| the change | 5.24 | 6.09 | 31.3 | 35.2 |
| `15944f91a` | 8.45 | 9.31 | 34.1 | 37.6 |

- Some 3.2 ms off `light_beam`, 2.8 ms off the frame, as predicted (the
  marks some 7 ms of it, the reads cut to 3/8).
- At 7 o'clock, shadows as before, no error.
