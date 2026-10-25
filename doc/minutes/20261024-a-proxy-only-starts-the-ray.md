# A rasterized proxy may start a ray, and nothing else

**2026-10-24** — decided in the conversation, on one condition the user
set: none of the machinery the chunk meshes needed comes back. Not built.
What is built first is the clean-up below, which holds whichever way the
start of a ray goes.

A ray costs what it does because every pixel walks the world's tree on its
own, though thousands of pixels looking at one face end on the same block. A
rasterizer shares that work in hardware: one box covers the pixels it covers
and each pays a depth test. So a rasterized proxy may tell each pixel where
its ray starts. It may not tell it what the ray meets. That stays the query's
answer, the one every reader asks
([a ray asks the world](./20261013-a-ray-asks-the-world-whether-a-node-is-empty.md)).

## What may not come back

At 10-12 the world was drawn from chunk meshes the CPU built. The meshes were
cheap to draw. What cost was everything that kept them: the run sets a chunk
was baked from, the frontier and the cover that chose which chunks to build,
the chunk table, the mesh slots, the build queue, and the invalidation that
told a mesh it was stale. Each was a state held beside the world, and each
needed its own rule for when it was right. None of them may return, under
any name.

## The conditions

1. **The proxy holds no fact of its own.** Its shape comes from what already
   bounds the world: the ground's per-square height bounds the eye's windows
   already hold (`bound_cs`, the levels of rust/spec/core/src/window.rs),
   and the objects' tree's nodes that hold something. Nothing is baked,
   meshed or cached for the proxy.
2. **It is made on the GPU each frame, from those bounds as they stand.**
   There is no CPU partition, no queue and no slot, so there is nothing to
   invalidate: a bound that changes draws differently on the next frame.
3. **It is conservative, and it only starts the ray.** A box over the
   highest ground in its square is never farther than the first block a ray
   through it meets, so a ray started at the box's front face misses
   nothing. The ray then walks as it does now, through the same query, and
   what it stops on is the pixel's.
4. **The camera grid picks the boxes.** At each distance the proxy draws the
   bound level whose cell spans a camera cell there, the rule a ray already
   asks by (`Tau::cell_at`), so the level of detail stays one rule.

## What it would replace

A ray's start is set today by three passes over a buffer sized by the screen
(rust/spec/core/src/beam.rs): beam rays at the camera grid's corners
(`beam_cs`), a start for each cell off them (`cells_cs`), and samples a quarter
cell apart that pixels take their block from (`sample_cs`). That buffer is
the one that met the 128 MiB per-binding floor on a 4K surface. A depth a
pixel from a rasterized proxy gives each pixel its own start directly, so
the proxy is a reason to remove those passes, not to add beside them. If it
cannot remove them, it is not taken.

## What is done first

Each of these holds whether or not the proxy is taken, because the ray stays
what decides the pixel. They are in:

- a block's face bits asked of the query once a block, by the sample that
  stops on it (rust/spec/core/src/faces.rs);
- the camera grid computed once a frame and handed to the GPU in the eye's
  words (`beam::Grid`);
- the per-pixel miss list gone: a pixel that missed says so in its own ray
  word (`ray::MISS`);
- one depth rule for what is laid on a face, owned by the screen pass;
- the sun's map cast incrementally, only where it moved or the world changed.

Landing them first means a proxy, if built, is measured against a cast whose
start is the only thing it changes.

## What has to be measured

- What drawing the boxes costs: a square's boxes instanced over every window.
- What a ray still pays from a near start, since it descends from the root
  again at that depth.
- How loose the boxes are on steep ground, where a square's highest point
  stands well above most of it and the start gains little.

## Deferred: the image shimmers

Pixels and outlines shimmer as the eye moves. Read from the code, not yet
measured on the page (the build was broken by work in flight). Three causes,
the first the largest:

1. **A sample decides what a pixel shows.** Since 2026-10-23 a pixel does not
   walk: of the blocks the 4 × 4 samples about it stopped at, it takes the
   nearest its ray enters (pxl-shader `cast_cs`). Samples lie a quarter cell
   apart, about three drawn pixels. A face drawn thinner than that, a riser
   seen at a slant or a far tree, is met by a sample or not depending on
   where the screen's grid falls, and where none meets it the pixel shows a
   block behind it, with no walk of its own, since a candidate was entered.
   The grid moves with the eye, so the face comes and goes frame to frame,
   and its outline with it. The beam of
   [20261022](./20261022-a-cell-casts-a-beam.md) had none of this: its proof
   held every ray to the root walk's. The samples gave that up for speed.
   The references agree: ESVO's coarse rays, Reshetov's beams and DIST's
   split rays give each pixel a start, never its first hit, and Lumen
   interpolates sparse probes only for low-frequency light.
2. **The render resolution moves continuously.** _Fixed since: the scale
   takes one of `SCALE_STEPS` steps down to `SCALE_MIN`, a step up held
   until `SCALE_UP_WINDOWS` windows in a row would fit it and no window
   judged for `SCALE_SETTLE_WINDOWS` after a change (pxl-kernel
   domain/view/frame/resolution.rs, domain/view/param.rs)._ Out of a tenth of its
   budget, the scale moves by the square root of the ratio, between half and
   whole, in no steps (pxl-kernel domain/view/frame/resolution.rs), and each move
   lays every pixel anew. Engines that scale by load hold small changes back
   and wait frames between changes, and upscale over time to hide them.
3. **Lines and silhouettes are point-sampled**, one sample at a pixel's
   middle, so a line a pixel wide crawls as it crosses pixels. TAA would
   answer it with a history buffer, a state to weigh only if 1 and 2 leave
   it visible.

What is proposed, not yet ordered:

- **For 1, remove the samples' answer.** A pixel walks from its cell's
  beam, the path 20261022 proved (`resumed`), and the candidate pick, its
  axis test and the samples' block and lack words go. Making the samples a
  second beam level was weighed and set aside: a proxy that is taken
  removes the beam's levels, so it would be built to be thrown away, while
  the pixel's own walk from a start is what a proxy feeds as well. It costs
  more than now: about ten visits a pixel (20261022, As built), and face
  bits a pixel's own unless a sample stopped on the same block. Whether
  the sample pass stays only to share face bits is decided by measuring.
- **For 2,** a few steps of scale, each held a while once taken. Done
  (cause 2).
- **Then the proxy is measured** against that exact cast, whose start is
  the only thing a proxy changes.

## Measured: a pixel's own walk

The steps view (setting `ray_steps`) then walked every pixel's own ray from
its cell's start (8f8c6152d; no longer, below), the cast "For 1" proposes.
On the page (Chrome, dpr 2, render scale 1.00, the dev build with the
profiler on, one standing view over 30 s, 97 cast frames each):

| The cast                        | Cast span, p95 |
| ------------------------------- | -------------- |
| The samples' answer, as built   | 62.5 ms        |
| Every pixel walking its own ray | 281.6 ms       |

About 4.5 times. "About ten visits a pixel" reads small beside the samples
because a cell has sixteen samples and a hundred-odd pixels, so walking
every pixel walks several times the rays. A pixel that walks also misses
the observed pyramid more often and walks again in `miss_cs`; that is in
the number. A comparison, not a profile: one load each, its first casts
among the frames.

## Recommended: the samples go with the proxy, not before it

Recommended by the reduce session, not ordered.

- **The samples' answer is not removed on its own.** The cast already
  stands about four times over the gate of
  [20261020](./20261020-a-visit-reads-one-entry.md) (p95 ≤ 16.7 ms), and a
  pixel's own walk from its cell's start multiplies it again.
- **Cause 1 and the walk's cost have one answer: a shorter walk.** That is
  the hybrid this record allows: rasterized boxes start the ray, and the
  ray still decides the pixel. A pixel starts at its box's front face, near
  the block it stops on, not at its cell's start. So "For 1" is measured
  together with the proxy, as one cast: the boxes drawn, every pixel
  walking from its box, no samples' answer. If that cast is under the one
  as built, both are taken; if not, the samples stay and the shimmer is
  answered another way.
- **Until then the steps view shows a path the frame does not run.** It
  should show the frame's own: a pixel that took a sample's block reports
  that sample's visits, recorded beside the sample's block. That waits for
  the beam layout in flight (`beam::Grid`) to be committed. Since then the
  steps view runs the frame's own cast, but a pixel that took a sample's
  block reports no visits, as a sample keeps none beside its block
  (pxl-shader `cast_cs`, `word`; `beam::SAMPLE_WORDS`).

## Deferred: the sun's map costs what 10-12's did not

Measured 2026-10-24 on the start view, shadows on, the camera still: about
5 to 10 frames a second for the first ten seconds, 40 to 70 after, and a
frame of 200 to 300 ms now and then; with shadows off, 120 throughout. At
10-12 the map was the chunks' meshes rasterized, a cost of their
triangles. Now each of its texels walks a ray (pxl-shader `sun_cs`), about
200 ms for the whole map, and the whole map is walked again where a
square lands that the map lies inside, and whenever the sun steps
(pxl-render gpu/shadow.rs `aim`). Since then a texel is cast again only
where the box moved onto it, the eye's zoom at its stop changed, or its ray
passes over a node the query answers otherwise (pxl-spec-core sun.rs
`Sun::kept`, `Sun::stands`, `over`); a step of the sun still casts the
whole map. The eye's rays read the observed
pyramid and share their descent by the beam; the sun's do neither: each
walks from the roots through the world's query (`World`, `cast::first`),
composing every node it visits again. Left as it is: a proxy taken here
starts the sun's rays as it starts the eye's, through the path the eye's
take, and the map's cost is measured with it. Shadows stay off by
default, as at 10-12.
