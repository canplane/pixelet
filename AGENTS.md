# Agent Instructions

## The main axis

- **The eye decides.** The view is a grid of angular cells about the eye. A
  block is observed at the zoom whose block spans at least a cell at its
  distance. A cell's beam ray walks the descent its pixels share
  (doc/minutes/20261022-a-cell-casts-a-beam.md); samples under each cell walk
  on, and a pixel shows the nearest of their blocks its own ray enters,
  walking itself only where none is. The pixels are the render resolution's,
  which the load sets (pxl-kernel view/frame/resolution.rs).
- **The query is the only seam.** Every reader of the world — a ray, a
  walk, an edit, a placement — asks the same query for what is in a node at
  its zoom. Each source answers from its own level of detail.
- **Drawing and mechanics compose the same blocks** (`pxl-spec block/`):
  the ground's grid, then what overwrites it
  (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md). Control is
  headless; render only colors.
- **Level of detail is the world there.** What a zoom's fold discards is
  absent at that zoom.
- **A kind of thing is drawn wholly on the CPU or wholly on the GPU.**
- **Dependencies point one way:** turn, door and shell wire everything from
  the top, and nothing below names them; control, which the door's events
  enter, moves the eye and asks the query, naming no scene; scene → view → query → layer, a
  DAG whose edges never point up; io and the turn's edge and phases below
  everyone; leaves (settings, models, billboards) below what reads them. The
  profiler reads anything, and production reaches it only where it records.
