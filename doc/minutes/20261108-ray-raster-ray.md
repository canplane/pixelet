# Ray, raster, ray: what each step does, and why

**2026-11-08** — recorded with the user. The S23's p95 frame was traced to the light's rays,
and the user asked two questions: whether the light needs rays at all, and why the last step
is a ray. A list of the light's cells, drawn by the raster, was measured the same day and set
aside. The structure stays as it is, for the eye and the light alike. This record gives each
step's reason.

## The structure

```text
  1. rays find              2. the raster draws         3. a walk completes
  beam rays walk the        each voxel the rays         a pixel that no fresh
  descent that a cell's     found is a box; the         voxel answers, a hole,
  pixels share; samples     depth test keeps the        walks the query from
  walk on and stop on       nearest box that each       its cell's start to the
  the first voxel           pixel's own ray enters      first thing it meets
          │                          ▲                            │
          └── kept in the record ────┴──── kept in the record ────┘
```

## What each step does

**The rays find.** At the corners of each angular cell, beam rays walk the part of the
descent that the cell's pixels share. Samples walk on from there and stop on the first voxel
they meet. A voxel is a node of the query, so what the rays find is the world's own answer.

**The raster decides what each covered pixel sees.** Each box the raster draws is a voxel
itself, not a bound around candidates. A face keeps a pixel only where the pixel's own ray
enters the box, so the depth test keeps the nearest box that ray enters, at the point where it
enters (`raster.wgsl`). For every pixel that a recorded voxel covers, the raster gives the
exact surface. The record keeps every voxel a ray stopped on, and the raster draws it each
frame until the data it read changes.

**The walk completes the frame.** A pixel that no fresh voxel answers is a hole. A hole is one
of three things: a strip of a node narrower than the samples' spacing, at the edge of what
hides it; a voxel whose data changed; or a node the GPU does not hold yet. The walk starts
where the pixel's cell starts (`queue.wgsl` `item_start`) and walks the query to the first
thing it meets. Its job is completeness: it finds what the samples missed. It does not choose
between candidates, since the raster has already done that.

## Why the last step is a ray

For a pixel with no candidate, the walk has to find the first thing along the pixel's line, at
the zoom its distance asks for, from the query. Three other ways were weighed.

- **A raster.** A raster needs something to draw, and a hole has no candidate. The cells that
  might lie there would have to be listed first. A list cannot stop at the first thing it
  meets, and the world is solid below its surface, so a list counts everything hidden as well.
  The light's list below asked 5 to 400 times as many nodes as the walks did.
- **Denser samples.** A strip may be as thin as any, so no spacing finds every one short of a
  ray per pixel. Denser samples cost every pixel of the screen, the square of the density, to
  find what lies along the edges. Dense samples, and a ray per pixel, were tried before and
  failed. The 11-06 reading that made the eye's samples denser in the grazing rows is not
  taken.
- **Geometry kept for drawing.** The world's one representation is the query. Geometry kept
  across frames would be the meshes of 10-12 again under another name.

A ray stops at the first thing it meets, so only what lies in front of it costs. The eye's ray
takes the zoom its distance asks for, and it needs nothing besides the query. No other way
does all three. The holes are walked by the same code as the samples (`queue_walk.wgsl`).

A cheaper last step was also weighed: testing a hole's ray against the raster's candidates
alone. It finds nothing, because a hole is exactly where the candidates missed.

**The cost stays small.** The walk runs only on holes, about 0.2% of the pixels in a usual
frame and 8 to 28 ms of a p95 frame on the S23. What it finds goes into the record, and the
raster draws it from the next frame on, so the walk runs only where something is newly seen.

## The light

The light is the second observer. The sun and the moon look along parallel rays, and each
tile of the light's box asks for one zoom, its band (`Light::band`). On the S23, the light's
walk (`light_hit`, `light_walk`) is the largest part of a p95 frame, 90 to 200 ms. On the
Mac's drive, a whole cast of the light reads 30,000 to 38,000 boxes at the median, casts 1.9
million texels and walks 146,000 of them
([the journal](../experiment/20261103-the-frame-on-the-s23/README.md), 11-08).

A list in place of the light's rays was weighed. The light has no zoom to follow ray by ray,
and the depth of the first thing each texel meets is what a raster's depth test of the band's
cells gives. Each tile's cells at its band would be found by a descent of the query over the
volume its texels' rays sweep, drawn from the light as boxes, and made again each cast. The
light's beam, samples and walk would go. A list leaves no hole: the light is orthographic, so
a texel's ray enters a box exactly where the texel's middle falls on the box's projection.
Against it, the eye and the light share the walk's code and its tests, and a list would need
its own, held to `cast::first` texel by texel.

### The list, measured

`list::cells` ran on the CPU (pxl-engine, not kept: [the patch](../experiment/20261103-the-frame-on-the-s23/tool/list/list-step1.patch)). It lists a
tile's cells at its band by a descent of the query over the parallelepiped that the tile's
texels' rays sweep, and keeps each split's children only within its content box. It was run
on the real world's test data, 14 cases of the light, 1,024 tiles of 8 texels a side each,
against `cast::first` for each texel:

| case | cells a tile | nodes asked a tile, listed | nodes asked a tile, 64 walks |
|---|---|---|---|
| about the eye | 13,361 | 17,307 | 2,106 |
| wide, over the clip levels | 594,309 | 707,742 | 1,798 |
| on a bridge | 3,477 | 4,660 | 1,820 |
| on what is placed, cut and hid | 856 | 1,359 | 3,077 |
| straight down | 10,809 | 13,502 | 1,772 |

- **Exact.** In 13 cases every texel took the depth its walk leaves. Past the area's wall,
  4,055 of 65,536 texels differed, the list's depth nearer. The cause was not found.
- **Not cheaper.** In all but one case the list asked 5 to 400 times as many nodes as the
  walks. A ray stops at the first thing it meets and walks nothing under the ground. The list
  takes every cell in the sweep that is not transparent, and the ground is solid as deep as
  the sweep runs.
- **Still not cheaper without what is hidden.** The user said that the ground under the
  surface is not to be counted. On a quarter of the tiles, a cell at the band was left out
  where each of its faces toward the light meets a listed cell. About the eye, 6,947 of 13,579
  cells a tile were left out; on a bridge, 2,647 of 3,503; straight down, 3,159 of 11,775.
  What remains at the band is the surface that the tile's rays sweep along. A low sun sweeps
  hundreds of its cells, and each has a face toward the light. Only something farther along
  the light can hide one, and that is the walk's own test. Coarser than the band, a tile lists
  2,300 to 2.3 million whole cells and lacks, under the ground or past what is held. Leaving
  out the hidden ones there needs a test against neighbors at each cell's own zoom, which was
  not taken.
- **What would make it cheaper** is an answer that a node is solid throughout, under the
  ground, which the 11-06 cell raster put first. Caves, bores and placed objects open the
  ground below its surface, so such an answer has to take each of them in, and what remains is
  a front-to-back test of what hides what. That is the walk's own work.
- The 11-06 estimate of a list near 100,000 cells counted no cell under the surface.

**So the light keeps its rays.** It does not need them for its zoom, since a tile has one. It
needs them because only what is seen costs, and the world is a solid volume. A list kept
across frames was not weighed: it would be the meshes of 10-12 again.

## Closed

**The eye and the light are one observer, by one rule.** The light sees every kind of thing
through the query, as the eye does: the ground, its caves, the ways and placed objects.
Neither handles a kind apart. A raster of the ground's heights from the clip levels, for the
light, was weighed and not taken: the caves, the bores and placed objects would each need a
caster of their own, and the ground would need a reader beside the query.

**10-26's "each frame's geometry is found by rays" holds for the eye and the light alike.** So
does the rest of that record: the world's one representation is the query, no geometry is kept
per square, chunk or column across frames, and a job goes to what does it better.

**The structure stays.** The eye and the light are each rays that find, a raster that draws
what they found, and a walk where a hole is left (ARCHITECTURE.md, Drawing). What remains of
the S23's p95 is sought within this structure.

## The light's raster on the S23, 11-11

Recorded with the user, in the session that took up the light's tail on the S23
([the experiment](../experiment/20261107-the-lights-tail/README.md)). Main was at
`3de91d61b4`.

**What was found.** On the S23's page the light's raster drew next to nothing, so its
texels were walked as holes. Its span `light_raster` was shared by three passes, two
compute passes and the render pass, and the first alone wrote the beginning. On the S23's
Chrome (157.0.8103.50) a render pass that writes only its end-of-pass timestamp loses an
indirect draw, with no console error; with both timestamps, or with none, it draws. The
Mac's Chrome (157.0.8103.39) and the S23's app do not lose it. The eye's render pass had a
span of its own and was never affected.

**A draw whose count the GPU makes is drawn indirectly, the eye's and the light's alike.**
How many boxes a raster draws is decided in the frame by a compute pass, so the count is
the GPU's, and the GPU reads it where it wrote it. A draw whose count the CPU knows, a
full-screen pass or the tiles a frame claims, is drawn directly. Which way a draw goes
follows from who knows its count, not from a measurement.

**The light's direct draw goes.** It went in on 11-10 (`360e05dd5f`) as a workaround for
the loss. Its count stood in for one the CPU does not know: an upper bound, a word of 0 for
each place no box reached, and the buffer cleared each cast, so the light's cost followed
that bound and not what was seen.

**Every pass writes both of its timestamps.** A span that several passes share is the sum
of their passes, and no pass writes its end alone.

**Where the draw is lost is not ours to find.** Whether Chrome, Dawn or the phone's driver
drops it is not sought: no pass here writes its end alone, so nothing here meets it.

Weighed and not taken:

- **Direct everywhere.** It is symmetric too, but every raster would cost its bound each
  frame, against the rule that a frame costs what it sees.
- **A span of its own for the light's render pass**, with a check in `gpu/api.rs` that
  refuses an end without a beginning. It closes this case and leaves the shared spans,
  which can make it again; a pass that cannot write its end alone leaves nothing to check.
