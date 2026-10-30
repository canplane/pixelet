# Why rays, and what makes them fast

**2026-10-31** — recorded with the user. Main was at `e40367e5f`. The two
questions and the judgment on the 9/26 build are the user's; the reasons are
gathered from the records cited, and nothing here is measured.

## What was asked

The backlog kept a parked item to measure what made moving smooth: three
consecutive commits and the frame against the chunk-mesh build of 2026-09-26
(`e78eeec55`), as first written in
[what made it smooth](../experiment/20261026-what-made-it-smooth.md). The
user put the questions that matter instead:

1. Against a raster, why the present structure is the right one for this
   project.
2. Against a pure ray a pixel, which optimizations make it fast enough, and by
   how much each.

And the 9/26 build is no fair raster to set it against: the rays have had
far more work put into them since, so the gap would measure the work as much
as the method. The three commits are not split out either: the structure has
changed much since them, and what they added (keeping, passes, the sun's
reads) is still in the code to be switched off where it stands.

## 1. Why rays, as the records give it

- **One answer, every reader.** What is at a block is the layers read in
  order by one function (pxl-spec-core `world.rs` `node`): the cast draws it,
  and a walk, a floor, an edit and where decor stands ask the same function at
  the same zoom ([the ground is a grid](./20261012-the-ground-is-a-grid-then-overwrites.md)).
  A mesh is a second form of the world that drawing reads and control does
  not.
- **Layers written over the ground, with no mesh to remake.** The ground is
  one grid, and caves, ways and what is placed write over it in 3D only where
  they hold something (same record). An edit or a cave shows as the next ray
  reads it; nothing is meshed again.
- **Detail by distance, from one rule.** The same rule picks the zoom a ray
  asks at a distance (`grid::cell_zoom_at`), and a coarser node is what is
  drawn there ([the GPU casts a ray a pixel](./20261014-the-gpu-casts-a-ray-a-pixel.md));
  there is no set of meshes a zoom to build and join.
- **Cost by the screen, not the world.** A ray a pixel of the render
  resolution, the resolution a scale the load sets in steps (same record), so
  a world the Earth's size costs what the screen does.

## 2. What makes them fast, against a pure ray a pixel

Each is discovery shared or kept, listed with its "off" in
[what made it smooth](../experiment/20261026-what-made-it-smooth.md): the
observed pyramid kept across frames (off: forget all every frame); the beams'
passes (off: one pass); beams to cells to samples to pixels (off: every pixel
walks from its cell's start, 4.5 times the cast,
[a proxy only starts the ray](./20261024-a-proxy-only-starts-the-ray.md));
faces asked once a box; the sun on the eye's pyramid and lines; empty space
skipped. Since then the screen's and the light's tiles keep what did not
change and claim only what did
([the screen keeps its tiles](./20261029-the-screen-keeps-its-tiles-as-the-light-does.md)).

The measure is each of these switched off alone at the tip, against the tip
and against all of them off: what each buys over a pure ray a pixel. Not the
commits that brought them, and not the 9/26 build.
