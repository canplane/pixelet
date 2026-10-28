# A ray is read against the light that arrives

**2026-10-28** — recorded with the user. Written against main at `76c992698`.
It records a way of judging the drawing, and the direction that judgment
gives the grazing rows
([what the grid cannot tell apart stands coarse](./20261028-what-the-grid-cannot-tell-apart-stands-coarse.md)).
Nothing in it is built yet.

> A ray goes out from the eye; a raster takes the light that arrives. They
> answer one question from either end, so a mechanism on one side is judged
> by its counterpart on the other. Read that way, the rows are not a pixel's
> problem: a raster never misses a whole row, because every thing is
> projected. What is missing here is a complete set of what is projected. A
> sample that stops where its cone touches anything, as a conservative
> raster covers what touches a pixel, makes that set complete in one frame,
> with nothing drawn larger than it is.

## The relativity

The user's point: a raster is the light arriving, so think of it the other way
round, by relativity.

Image order walks the screen and asks the world. Object order walks the world
and projects it onto the screen. [The eye decides](./20261014-the-eye-decides.md)
chose image order and set the two side by side once already: the hierarchical
depth buffer and the ground's levels are one shape turned around. This record
makes that a rule of judgment. A mechanism proposed on the ray's side is read
against the raster's counterpart. If the raster has none, or its counterpart
is known to fail, the proposal is suspect.

## What a raster does inside a pixel

Seen from the arriving light, the horizon is many things projected into one
pixel.

- **Point sampling.** A triangle covers a pixel only where it covers the
  pixel's sample point, and the depth test keeps the nearest that does. What
  is thinner than a pixel and misses the point is lost: far wires and fences
  break up and crawl.
- **No rows of holes.** Every triangle is projected, and neighbouring
  triangles share their edges (a watertight fill rule), so a pixel's point
  always lands on some surface. What a raster loses is under a pixel, never a
  whole row.

Each anti-aliasing mechanism answers that loss, and each has a counterpart
here:

| On the raster's side      | What it does in a pixel                                      | Its counterpart on the ray's side                                 |
| ------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------- |
| MSAA, SSAA                | several coverage samples a pixel, in space                   | samples denser than the pixel                                     |
| TAA                       | the sample point jittered every frame, the history blended   | the samples jittered, the boxes found kept over frames            |
| Conservative rasterizing  | a pixel counted covered where anything touches it            | a ray stopped where its cone touches anything                     |
| Mipmaps, anisotropic      | a texture prefiltered, read along the footprint's long axis  | how a cell looks at a grazing angle, not whether it is there      |

A raster never prefilters geometry itself. The voxel renderers that do,
GigaVoxels and Heitz and Neyret (HPG 2012), average coverage along a cone and
blend it. That needs transparency the opaque cells here do not have.

## The rows, read this way

- **The pixel's own test is the raster's.** A pixel takes the nearest box
  its own ray enters, tested exactly (`cast::entry`). That is point sampling
  with a depth test, and what it loses is under a pixel, as a raster's is.
- **The box set is not.** A raster projects every thing. Here only the boxes
  the samples stopped at are drawn, so a row no sample stopped at is not in
  the set at all. It is missed whole, as wide as a sample's spacing, which a
  raster never does. The stripes of 10-13 and the rows of 10-27 are that one
  fact (10-14's proof).
- **The prune has no counterpart.** No raster mechanism and no reference
  passes a thing that is there because it is thin. ESVO (Laine and Karras,
  2010) stops a ray at a voxel no larger than the ray, "since the exactness of
  the result is not relevant for very small voxels". It does not pass it. The
  prune passes, and so it errs toward holes.

  > **Note (10-28):** the prune is gone
  > ([a node is asked in its own units](./20261027-a-node-is-asked-in-its-own-units.md),
  > its note of 10-28).

## The direction: stop where the cone touches

> **Note (10-28):** not taken. Built, it held the visits and cost five to
> seven times the sample and raster passes
> ([the cone touches](../experiment/20261028-the-cone-touches.md)). The
> prune was taken off instead.

The user asked, before this record, why not stop the moment anything at all
is touched.

- **It is the prune turned around.** The prune shrinks a box by half the cone
  and passes what it then misses: it erodes, and holes follow. Stopping where
  the cone touches widens the box by half the cone and stops: it dilates, and
  no hole follows. Either one ends the grazing walk, because what made it long
  was a ray skimming close to a surface, and that ray now stops.
- **It is conservative rasterizing on the ray's side.** A sample's cone is the
  grid's cell, wider than the samples' spacing (a quarter of it). So every
  cell whose visible part lies anywhere on the screen is touched by some
  sample's cone, and its box enters the set.
- **What is drawn stays exact.** The raster draws a cell's own box, and a
  pixel takes it only where its own ray enters it. The widening only decides
  which boxes are candidates; nothing on the screen is drawn larger than it
  is. A sample that stopped short of what lies behind leaves that pixel's
  ray to refuse the visible sample and walk again.
- **The zoom is untouched.** It stays the distance's, so there is no patchy
  coarsening of the kind a stand would bring.
- **It is deterministic.** It is decided in one frame. TAA's counterpart would
  be cheaper, but it leans on the frames before, so holes flicker while the
  view moves and no frame's image is its own. The user asked for a
  deterministic answer, which rules it out.
- **Symmetry holds.** Boxes and a cone, so every source alike. Each observer
  keeps its own cone. Control and physics keep `Spacing::NONE` and are
  unchanged.

## Open

- **The walk again.** A pixel that walks again needs the same rule at its own
  width, a pixel's, where the widening is under half a pixel and unseen. How
  far those walks run near the horizon is to be seen.
- **Stopping in a split.** Where the cone first touches a split coarser than
  the zoom asked, the descent goes on with widened boxes to the first cell of
  that zoom the cone touches, bounded inside that node.
- **Order.** The prune taken off for the eye in an export, seen on the
  screen. Then this rule, side by side against `76c992698`. Both are seen, not
  timed; the timing comes once, at the end.
- **The hold.** Nothing here starts before pixelet-d6's shadow work is done.
