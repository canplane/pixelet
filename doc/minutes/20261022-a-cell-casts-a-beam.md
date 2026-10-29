# A cell of the camera grid casts a beam, and a pixel finishes it

**2026-10-22** — decided in the conversation, after the references below were
weighed; kept up to what the code holds.

The cast walks the pyramid's shared descent once per camera-grid cell, and each
drawn pixel pays only what lies past it. The descent, which is the log n part
of a ray, is paid per cell; what a pixel pays does not grow with the depth of
the world.

Per cell: a beam ray at each of the grid's corners walks the shared descent and
stops early; each cell takes its start from the beam rays about it; samples
four a cell a side (`param::beam::SAMPLE`) walk on from that start; the raster
hands each pixel the nearest block a sample stopped at that its own ray enters
([the raster picks](../experiment/20261026-the-raster-picks.md)), tested again
exactly (`cast::entry`); a pixel none is handed walks on from its cell's start
itself. The proof below is that a walk from a cell's start, a sample's or a
pixel's, ends where the walk from the root ends. A pixel that takes a sample's
block is not held to its own root walk: that cut was taken, and "Sparser
samples lose coverage" below is answered by the pixel's own walk where no
sample's block holds it.

## The problem it answers

A ray a pixel
([the GPU casts a ray a pixel](./20261014-the-gpu-casts-a-ray-a-pixel.md)) asks
the zoom its distance asks by the camera grid, so its level of detail does not
depend on the screen. Its count does: every pixel walks from the root, and the
rays of one cell, about a dozen pixels a side, ask the same nodes over and over
before they part. The cost of a frame grows with the pixels, which the user
raised days before (2026-10-21).

A ray a cell, turned back into geometry, drew the ground in stripes
([the eye decides](./20261014-the-eye-decides.md)): blocks between two
neighboring rays are never found, and nothing covers their pixels.

## The references, and what each one says here

- **One ray a pixel is the baseline of image order.** Dick, Krüger and
  Westermann, _GPU Ray-Casting for Scalable Terrain Rendering_ (EG 2009), and
  both image-order methods of Malyshau, _Six Ways to Draw Vangers with WebGPU_
  (2026), cast one ray per pixel.
- **Sparser samples lose coverage.** Malyshau's forward-scattered points "do not
  guarantee pixel coverage; undersampling appears as isolated gaps", the stripes
  of 20261013 measured on another engine.
- **Interpolating sparse samples is for low-frequency signals.** Lumen's screen
  probes trace sparsely and interpolate, for indirect light, not for what a
  pixel sees first, and lean on the frames before.
- **Coarse rays share the descent; pixels finish it.** Laine and Karras,
  _Efficient Sparse Voxel Octrees_ (I3D 2010), §4.2: rays at the corners of 4×4
  or 8×8 pixel blocks render "a coarse, conservative distance image", each pixel
  starts from it, and the guarantee holds because a coarse ray stops at "a voxel
  that is not large enough to certainly cover at least one ray in the coarse
  grid", contour tests off. Reshetov, Soupikov and Hurley, _Multi-level ray
  tracing algorithm_ (SIGGRAPH 2005), find one deep entry point for a beam of
  rays; DIST's coarse-to-fine sphere tracing splits a ray into four as it nears
  a surface.

The last group is the structure taken. The coarse rays here are the camera
grid's own: the corners of its cells, so the beam's spacing is the view's cell,
`grid::CELL`, 1/`CELLS_A_RAD` radians, and does not depend on the screen.

## The beam

A **beam ray** is cast through each corner of the camera grid's cells. It walks
as an eye's ray does (`cast::walk`, `Until::Beam`), with two differences:

- **It meets every split its cell's rays may meet.** It passes whole a split
  whose box, grown by `2√2 · s · d` on every axis in the split's own steps, it
  does not meet, as the grid's rays about it do not either; it enters every
  other (`cast::NARROW`, `meets_box`). `s` is the cell's angle and `d` the
  node's farthest distance from the eye.
- **It stops early.** It stops at the entry of the first block that is not
  transparent, and of the first split it meets that is narrower than
  `2√2 · s · d` (`cast::narrow`), which may lie between the grid's rays.

It hands on two things: how far it got, shortened by a margin (below), and its
line, the stack of what was handed each node down to where it stopped
(`cast::Keep`).

## Why a walk may start there

Take a ray of a cell, a sample's or a pixel's, and `X`, the block that ray,
walked from the root, ends at: the first that fills, or a lack. Let `e` be
where the ray enters `X`, and `Y` the cube of the pyramid's grid that holds `e`
and is the smallest at least `√2 · s · d` wide, `d` its farthest distance. `Y`
is either a node over `X`, or a cube inside `X` where `X` is wider than that,
as a block near the eye is.

- **`Y` holds a beam ray.** The ball inside `Y` has a radius of half its side,
  so it covers an angle of at least `side / (2d)` about its center, and a screen
  only stretches angles. That disk, at least `s / √2` in radius, holds a corner
  of the grid, and that corner's beam ray passes through `Y`.
- **That beam ray stops where it enters `Y`, or sooner.** Where `Y` is over `X`,
  it is a split, since `X` lies in it at a zoom asked within it (`cast::asked`
  is a function of the node and the eye, so every ray asks it alike), and it is
  narrower than `2√2 · s · d`, since its child toward `e` is narrower than
  `√2 · s · d`. The beam ray meets `Y`'s grown box, which holds `e`. Where `Y`
  is inside `X`, the beam ray meets `X`, which ends any walk, no later than it
  enters `Y`. Either way it stops at some `t_c` no farther than its entry into
  `Y`.
- **The margin.** Two rays enter one cube no more than its diagonal apart, and
  `Y`'s diagonal is less than `γ · t` with `γ = 2√6 · s / (1 − 2√6 · s)`, `t`
  the beam ray's entry into it. A beam ray hands on `t_c · (1 − γ)`, which is
  then no farther than where the other ray enters `Y`, and that is no farther
  than `e`.
- **The window.** The corner whose ray passes through `Y` lies within `Y`'s
  image, at most `√6 · s · (1 + γ)` from its centre as an angle, stretched by
  the screen's slope at that cell's corner farthest from the axis. A cell
  starts from the least of the beam rays within that many cells of its own, a
  square window over the corners each cell takes for itself
  (`beam::cell_window`, `cell_cs`).

So nothing that ends a walk lies on the ray before its start, and the block it
ends at is entered at or after it: the walk's end, its distance and the axis it
entered across are the root walk's.

## Where a walk resumes

Starting from the root at its distance would still pay the descent. A walk
from a cell's start takes the line of the beam ray that start came from: the
deepest node on that line that holds the starting point (`cast::Node::over`),
climbed to the first whose planes hold the ray there (`cast::holding`), what
was handed it, and the stack over it (`cast::Over`; pxl-shader beam.rs
`resumed`). Each node's answer is the node's and the eye's alone, and what is
handed a node is its own whatever walk reached it, so the walk from there is
the root walk's from its start on. It descends the few zooms between that node
and the blocks, and steps on. A sample and a pixel the raster hands nothing
resume alike (pxl-shader observer.rs `resume`, `walked`).

## What a frame asks, by count

- **Beam rays:** one a corner of the grid, each a full walk.
- **The window:** a minimum over the corners about each cell, once a cell.
- **Samples:** four a cell a side, each a walk from its cell's start.
- **Pixels:** the raster's pick, and a walk from the cell's start only for the
  pixels it hands nothing.

What the rays asked is kept between frames in the observed pyramid
(pxl-spec-core observed.rs;
[a visit reads one entry](./20261020-a-visit-reads-one-entry.md)), which
decides how much is asked again, never what is drawn: nothing depends on the
order the threads ran in, so the image is a function of the world and the eye
(20261012, "Determinism").

## As built

pxl-shader writes the passes once over the one a cast is for, the eye or the
sun's map (observer.rs `Observer`): `beam_cs`, `cell_cs`, `sample_cs`,
`sample_box_cs` and the raster's draw; the eye's pixels then take `cast_cs`,
and `miss_list_cs` and `miss_cs` for the pixels it marked; the sun's map its
own (sun.rs).

- **pxl-spec-core `cast`:** the beam's stop rule beside `Until::Opaque` and
  `Until::Transparent`, and a walk that starts from a line's node at a
  distance, its stack read from the line, which a walk from the root is the
  case of.
- **The stop's distance is the entry plane.** A walk's stop takes its distance
  from the near plane the ray entered across, the block's and the ray's alone,
  so a walk resumed at a start and one from the root report one distance
  (`cast::walk`).
- **A resumed walk starts in the node the ray is in.** The starting point is
  floored to a Q3 step, which can lie a step outside the node the ray is in, so
  the start climbs to the first node whose planes hold the ray there
  (`cast::holding`).

When it was built, a walk from a cell's start was checked against the walk
from the root, counted on the CPU over the r12 tiles at E120, the canvas
2528 × 2226 (a probe, not kept), every ray of the frame walked both ways:

|                                                |             |
| ---------------------------------------------- | ----------- |
| rays                                           | 5,627,328   |
| rays whose end, distance, node or block differ | 0           |
| beam rays (corners, the window 5 cells)        | 41,065      |
| visits a beam ray                              | 41.38       |
| visits a pixel, from the root                  | 33.16       |
| visits a pixel, resumed                        | 10.24       |
| visits in all, from the root                   | 186,599,875 |
| visits in all, beams and resumed               | 59,317,697  |

On the page (Chrome, dpr 2, the same canvas, pinned `&at=`), a cast resumed a
pixel against the same tree casting every pixel from the root: 51 pixels of
5,627,328 differed at E120, 56 at W120, 20 at E600, each alone, and where looked
at, a tree's trunk standing where the ground meets it. The CPU's walks agree on
every ray, so these are the GPU's f32, which is not strictly IEEE
(pxl-spec-core `cast`, its module note): two ways to one block can part in the
last bit where a ray grazes an edge. A comparison, not a proof.
