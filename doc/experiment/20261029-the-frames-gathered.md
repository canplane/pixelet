# The frames gathered

**2026-10-29.** Whether gathering the world's color over frames (a temporal
resolve, TAA) settles the far terrain's shimmer, what it costs, and why it and
not a filter made ahead (a mip of the world, or the prune). Built in
`8ec9c995f` (the jitter), `413edebe4` (frames that cast only their pixels),
`35492127d` (those keep the samples' boxes and faces) and `a0e5c39f7` (the
resolve).

## Why

Far terrain flickers and crawls as the camera moves (the user, on :5173). Two
causes were found, and neither is a bug:

- A thin row that no sample hit: the samples lie 4×4 a cell
  (pxl-spec-core `param::beam::SAMPLE`), and 0.76–0.90% of stops fall between
  them.
- One sample a pixel at grazing angles: the zoom a ray asks is by distance
  alone, so a face seen edge-on holds many cells in one pixel.

Eight samples a cell costs about five times the sample pass (1.25 → 6.23 ms);
a walk for every pixel about 250 ms a frame. The user kept 4 and asked for
antialiasing.

## What was built

- The renderer gives each frame a jitter, Halton(2, 3) over 8, in drawn
  pixels (pxl-spec-core `eye::JITTER`, frame word `JITTER`). The cast, the
  screen pass, the samples' raster and every pass drawing into the world's
  color agree on it (`8ec9c995f`).
- A frame where only the jitter moved casts the pixels again off the samples
  that stood (`FrameKind::pixels`, `413edebe4`), and keeps the samples' boxes
  and faces (`35492127d`).
- gpu/resolve.rs, screen_resolve.wgsl: the frame's color blended into two
  histories at the world's size, in turn. Each pixel's nearest depth is moved
  into the last frame's view by one matrix made in f64. The history is held
  within the mean and spread of the pixel and the four beside it, in YCoCg.
  The share is max(1/(n+1), 0.1); where nothing but the jitter moved, the
  history is not held and the share is the sequence's even one. The jitter
  holds once the frame anything else moved in and the seven after it are
  drawn, and the frame stands; the renderer asks for those frames, as nothing
  else does while the view is still. The finish encodes the history written
  (`a0e5c39f7`).

## How

Two pages side by side in Chrome ("Chrome Tahoe"), each fresh, one tab in the
foreground at a time:

- **:5173**, the shared tree before the resolve (`9ae190913`).
- **:5184**, a worktree of it with the resolve and its 5-tap neighbourhood,
  before the review's fixes (the frames asked for, the sequence's last
  place); for pair 2, with the boxes and faces kept too.

**Flicker.** A script in the page (scratchpad `flicker.js`) dispatches a drag
of one CSS px a frame for 150 frames from the screen's lower middle. That pans
the ground under the pointer by a pixel and the far terrain by less. After
each frame it draws the canvas's top band (5–95% across, 4–26% down: the far
terrain) into a 2D canvas and takes its luma. Between consecutive frames that
changed, it counts the pixels whose luma moved by more than 40 of 255
("pops", per million pixels a frame) and the mean change. A drag one way then
back returns the view.

**Cost.** The profiler's report over an idle minute after one drag: GPU p50
and spans' p95.

## What was seen

**Flicker** (pops per Mpx a frame / mean luma change; each cell one run):

| pair | :5173 before | :5184 resolve |
|---|---|---|
| 1 | 5 156 / 2.56 · 6 206 / 2.99 | 438 / 1.37 · 491 / 1.36 |
| 2 | 4 890 / 2.41 · 3 892 / 1.94 | 438 / 0.81 · 730 / 0.85 |

The canvases were 3840×1986, but :5173's in pair 2, 3840×2098 (its tab
opened with a taller viewport).

About a tenth of the pops, half or less of the change.

Left out of the table:

- Each page's first run after its load: 9 557 on :5173 and 6 103 on :5184,
  tiles still coming in.
- A :5184 run of 512 / 1.41, which may have run in a background tab.
- Two :5184 runs after a rebuild, from a start view that had drifted from
  :5173's: 5 304 and 2 710. The render scale fell to 0.84 under load around
  then, which makes the histories again.

None of these was explained further.

**Cost:**

- Idle, in an earlier, smaller window (its size not recorded), the 9-tap
  resolve before the trim: GPU p50 11.9 ms on :5173 (every frame `standing`,
  the actor's clip playing), 16.2 on :5184 (every frame `pixels`). The
  resolve's p95 was 4.8–5.0 ms.
- The resolve's neighbourhood cut to the pixel and the four beside it: p95
  2.7 ms at 0.84 of 3840×1986 (5.4 M px), 4.8 at 1.00 (7.6 M px), about 0.6 ns
  a pixel.
- A `pixels` frame with the boxes and faces kept: sraster 1.2 ms p95 (1.7–2.3
  before), face only in full casts. GPU p50 16.0 ms idle at 3840×1986; the
  same resolution before the resolve was not taken.
- Memory: two rgba16float histories at the world's size, 8 bytes a pixel
  each: 122 MB at 3840×1986.

The page draws every frame whether or not the resolve is there: the actor's
idle clip and the cars move. So the jitter never holds idle, and every idle
frame casts its pixels again and resolves.

These ran with the user's own :5173 tab open in another window, so the GPU
was shared; the spans swing by a millisecond or two between reports.

What else a `pixels` frame redoes, by reading (the review), not measured:
the sun's receivers are marked again (they read the jitter, `cast.rs`), and
the occlusion's prepass runs again (its read takes the frame's words, jitter
and all, as its depth moves with it). The finish carries this frame's
jittered depth up beside the gathered colour, so the markers tested against
it may move by up to half a pixel while the sequence runs.

## What was not done

- **Holding the jitter while the view stands.** With the actor or a cloud's
  shadow moving, frames are drawn with the jitter held, and the history
  converges to that one sample in about ten frames (0.2 s). The antialiasing
  goes. Telling apart per pixel what moved needs a second target (MRT) or a
  storage texture kind neither the pipelines nor the passes have.
- **Fusing the resolve with the finish.** Same: one target a pipeline, and
  the two run at different sizes whenever the render scale is under 1.

## Estimated, not measured: a filter made ahead

Asked by the user: whether a mip of the world, or the prune, could stand in
for gathering over frames. Neither was built; this is reasoned from the
numbers above and the record.

**A mip of the world (prefiltered voxels, cone tracing; GigaVoxels, Heitz and
Neyret).** Each node keeps its mean color and the share of its volume it
covers. A ray takes the nodes its footprint asks, compositing them until it is
opaque. A thin row comes out as a faint row, and there is nothing to flicker.
It is the antialiasing a mip gives a texture. The pack's textures already
have theirs (gpu/pack.rs: a mip chain, sampled anisotropic).

- Here every pixel would walk. The cast is fast because a pixel takes the
  raster's pick of the samples' boxes and only the missed walk. A composite
  along a ray cannot be taken from the samples about it across a depth edge.
- A walk for every pixel to its first stop measured about 250 ms a frame. A
  composite walks further, until opaque; grazing rays skim the terrain
  through many partly covered nodes. Estimated: several hundred ms a frame,
  over ten frames' budget.
- Each node would also carry its coverage, and the tiles' bake would change.
  The values' colours from afar (`looks`) are a part of it, not per node.

**The prune.** It is a mip's choice of level without its mean: a node under
the footprint is dropped whole. As the camera moves the footprint crosses the
line, and the row comes and goes. That was the grazing rows of 10-28
([the rows seen](./20261028-the-rows-seen.md)), and the prune came out for it
(`7f9195c9c`). At its best, the 10-27 horizon, it cut the beam about three
times (10.9–12.4 → 3.7 ms), which would not bring a walk for every pixel near
a frame's budget either.

**The choice of level is already made** by the zoom a ray asks by distance.
What was missing is the mean. Made ahead in space, that is the prefiltered
world above; gathered over time, the resolve: estimated +5–10 ms a frame
(about 0.6 ns a pixel for the resolve, 3–5 ms for the pixels cast again),
within what the render scale can trade.
