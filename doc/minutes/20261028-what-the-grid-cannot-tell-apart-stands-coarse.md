# What the grid cannot tell apart stands coarse

**2026-10-28** — recorded with the user. Written against main at `8ee81810d`
(2026-10-28 00:00 KST). The checkout also held pixelet-d6's uncommitted reshape
of the observer's passes at the time. The record states a direction to build
from the code as it stands. Nothing in it is built yet.

> The philosophy settled in September and restated on the morning of 10-27 is
> the last key to finishing the drawing. Perspective closes the bound: what is
> far must be large. What is smaller than the grid is don't-care, drawn any way
> or passed. But a cell the grid can see is never don't-care. It stands, coarse
> if it must, and never as a hole. Each pixel is answered by a ray of its own,
> and the zoom it meets the world at follows how densely the rays land there.
> Every answer holds alike across direction, source, observer, axis and device.

## Thesis: the philosophy, as the user put it

Times are KST, 10-27, and each point below is the user's, put in English. The
September minutes those points rest on are
`archive/20260928-what-is-drawn-is-what-the-screen-covers.md` ("A cell's inside
is don't-care", "A box is all the view asks"),
`archive/20260923-a-coarse-cell-is-isotropic.md` and
`archive/20260926-the-view-is-cubes-about-the-eye.md` (the eye's cubes, the
pyramid, called RTE).

- **Far is large; under the grid, anything goes.**
  - 01:31: far must be correspondingly large; what is smaller than the grid
    may be drawn any way at all; solve it elegantly, mathematically,
    deterministically, with the bound closed.
- **Perspective is the bound.**
  - 01:32: perspective makes the upper bound by itself and guarantees it
    structurally.
- **The query is three-dimensional, in the pyramid's units.**
  - 01:41: we hold the one-axis view.
  - 01:48: the query asks in 3D and answers quantized to the pyramid's zoom
    and grid, which is symmetry, reduction and RTE.
- **The LOD look is the floating box.**
  - 01:55 and 02:33: the coarse look stays as it was, a box quantized to
    eighths, floating in its cell.
- **No direction is privileged.**
  - 10:17: looking up, down or at the horizon must cost and behave alike.
- **Don't-care means pruned, not drawn exactly.**
  - 10:19: treat what is under the grid as don't-care and prune it; it was
    never to be drawn exactly.
  - 10:28: the philosophy was settled in September.
- **Observers share units, not implementations.**
  - 12:18: the eye and the sun share resolution and pruning units and one
    interface, and each is implemented to its own effect.
- **The display decides nothing.**
  - 14:47: remove the display ratio's effect on LOD and rays, orthogonally
    (`0c84382cd`).

The September record says what the edge of don't-care is (20260928): "What is
not allowed is for an approximation to leak: to change what a neighboring cell
observes." And: "A center ray can miss a sliver between two cells; the sliver
then stays coarse, which is quality, not a hole." Its rule is tested "on each
axis of the screen", and "the grazing ground is the same rule and needs nothing
of its own".

## What the code holds, and keeps

The code is in good shape, and it is the base. Going back to an older form is a
regression, not an answer. What stands:

- The query asks a node and answers a box in its own units, every axis alike
  (`cf769abc1`; [a node is asked in its own units](./20261027-a-node-is-asked-in-its-own-units.md)).
- Nothing reads a ray's cone but a beam, as how far apart its rays lie
  (`cast::Spacing`): the eye's the grid's cell (`grid::CELL`), the sun's
  `sun::BEAM` texels; control's is none (`Spacing::NONE`).
- Nothing is passed: a ray enters every split whose box it meets (`cast.rs`
  `walk`, `meets_box`), the eye's and the sun's alike, so the rows are gone and
  the eye's horizon pays about a millisecond of GPU p95 more for it
  ([a node is asked in its own units](./20261027-a-node-is-asked-in-its-own-units.md)).
- Every view is cast at the view's grid, a cell 1/`CELLS_A_RAD` radians
  (pxl-spec-core grid.rs `CELL`), an angle, so the display draws and decides
  nothing (`0c84382cd`).
- The rays find and the raster hands out
  ([the ray finds, the raster hands out](./20261026-the-ray-finds-the-raster-hands-out.md)).
  The stages are beam, cell start, samples, the visible sample, the pixel and
  the walk again (`ac7785f21`, `bf1a3e20a`).
- A cell keeping less than its cube is drawn as its kept box, eighths floating
  in the cell (`answer::kept`, `cast::kept_bounds`).

## Antithesis: rows the grid can see go empty

Seen at a grazing angle, rows of coarse cells vanish whole. The user's
screenshot was taken 10-27 22:30. Looking straight down, none vanish. The user
saw them at 14:00 too and let them pass for speed, so this is not a late
regression. It belongs to the structure.

A whole cube is not don't-care. A cell at its zoom spans at least the grid
from any direction, by the zoom rule itself. Two readings of the code account
for the loss. The second was the rows' cause: with it off the rows went
([the rows, seen](../experiment/20261028-the-rows-seen.md)). The first is a
code reading, not confirmed on the screen:

1. **The zoom is the distance's alone, while the samples are sparse.**
   - `cast::Detail` takes `grid::cell_zoom_at(d²)`, isotropic, so a cell's
     extent across the screen along a slant shrinks with the slant's sine.
   - 4 × 4 samples a cell walk (`param::beam::SAMPLE`). A pixel takes the
     nearest box a sample stopped at that its ray enters, and walks only where
     its ray enters none.
   - A row thinner on screen than a sample's spacing has no box, so its pixels
     take the box behind it.
   - [A cell casts a beam](./20261022-a-cell-casts-a-beam.md) foresaw it. It
     says the cut was taken and "Sparser samples lose coverage" is
     answered by the fallback walk, not avoided. Its proof holds for a walk
     from a cell's start, not for the box a pixel takes from a sample.
2. **The prune passed as air what was as thick as a cone.** It is gone.
   - Each inner side of a split's box shrank by half a cone, so content up to
     a cone thick was passed, not drawn coarse.
   - A cell near the near end of its band kept a core `s − c` thick, where `s`
     is its side and `c` the cone, at most one cone. Samples a quarter-cone
     apart could step over it.
   - Passing a cell as air is a leak in 20260928's sense: the neighbor cell
     then observes what lies behind.

Both applied don't-care by distance, isotropically, and the first still does. September applied it by each
axis of the screen. That is the gap.

## What was tried and why each broke the philosophy

- **The touch rule** (`226b28d72`, `637b88db4`, `f3a2fbdd9`, `e5532c80c`).
  - It placed a coarse cell by bits only the ground set, so a terrace of placed
    cubes would look otherwise than the ground's. That breaks source symmetry.
  - The user (23:52): a user must not be able to tell, by its look, the ground
    from what is placed.
  - Reverted whole in `8ee81810d`. The lesson is in the reshape skill,
    "Proposing it" (`de7483394`).
- **Shadows and AO raised as costs.** They do not matter here. The user: what
  matters is going entirely empty.
- **A pixel walking again when its visible sample is far beyond its
  neighbors'.** It guesses at the observation instead of following the rule.
- **Reverting the prune, or τ.** Either regresses the horizon (10:17) or the
  display's independence (14:47). The user took the horizon's regression over
  the rows on 10-28, and the prune is gone; τ is gone too, the view's grid the
  only one, and the display still decides nothing.
- **A finer grid, or denser samples.** Either moves the threshold and keeps the
  hole.
- **The prune's cone at each ray's own width.** The idea was a sample's quarter
  cell, or a pixel, in place of the grid's cell. It was offered as keeping the
  horizon while the rows stay. It holds neither.
  - It answers the prune's half of the loss alone. The distance zoom and the
    sparse samples stay, so rows thinner on screen than a sample's spacing
    still go. They only get thinner.
  - The bound grows with it. A grazing ray crosses about `1/s` of the cone's
    cells an octave, and a cone four times narrower lets it cross up to four
    times as many: the horizon regresses.

## Synthesis: the direction

The user, 10-27 23:58: shoot a ray for every pixel, aim the cone well with the
beams, and set the LOD and zoom in proportion to how densely the rays land.
Read with the morning's philosophy:

- **Every pixel is answered by a ray of its own.** It is resumed from the beam's
  conservative start, as 20261022 was first built, where the proof held: 0 of
  5,627,328 rays differed from the root walk, at 10.24 visits a pixel resumed
  against 33.16 from the root.
- **The zoom follows how densely the rays land.**
  - It is the footprint of the ray's cone on what it meets: far is large, along
    each axis of the screen, as 20260928's rule is.
  - A grazing surface is met coarser. What is under the grid is then inside one
    cell, and stands as that cell's floating eighths box, the LOD look. It is
    never passed as air where it fills a cell.
- **The bound is perspective's.** A grazing ray met at its footprint's zoom
  stops at a coarse node, so its walk is closed by perspective, not by a prune
  standing in for it.
- **Symmetry holds across all five.**
  - Direction: up, down and the horizon alike.
  - Source: the ground and what is placed are one kind.
  - Observer: same units, each its own implementation.
  - Axis: one axis, the query in 3D.
  - Device: the display draws and decides nothing.

The fix lives in the walk and the zoom rule, which every source and observer
share. It is never in a source or a pass alone.

pixelet-d3's reading sits beside this. The user likes it, but it is not the
core, and where the two part the user's stands.

- The distance zoom stays the finest any ray asks, the eye's bound for what is
  fetched and held, and the footprint only makes a ray coarser.
- The cone's angle stays the grid's cell, so rays are per pixel while the
  quantum stays display-free.
- The slant can be read off the beam's four corner depths, which costs
  nothing.

## The user's verdict and turn (10-28)

- **The axis alone lets the rows go.** The user: this is the original axis, but
  on its own it concludes that the rows do not matter. A row thinner than the
  grid on the screen is don't-care under it, and the 14:00 rows were let pass
  for speed on that ground.
- **The turn.** Keep the axis, go to the pixel, and turn the LOD toward the
  rays' density. Many rays then land on the same place.
- **Concern: the prune.** Can a walk still prune when many rays land on one
  node? Read so far, and not yet decided: with the zoom at the footprint, a
  grazing ray stops at a coarse node, and the prune becomes cutting the
  descent there and drawing that node's box. It no longer means passing its
  content as air; the prune that passed it as air is gone. The cost is the look. A node is a cube, so one coarse
  enough along the screen's depth is as coarse across it (pixelet-d6, 10-27
  10:16).
- **Concern: the observed pyramid's room.** The observation cache is keyed to
  the pyramid's nodes in a table of fixed size (`observed`). Per-pixel walks
  write it far more, and a zoom that follows the slant puts more zooms of one
  place in it. As it stands, it will run out of room.

## 10-14 already answered it

The same stripes were met on 10-13 and proved; the drawing took a ray a pixel
on 10-14 for it ([the eye decides](./20261014-the-eye-decides.md),
[the GPU casts a ray a pixel](./20261014-the-gpu-casts-a-ray-a-pixel.md)).

- **The stripes.** With a ray a cell of the camera grid, a chunk drew only the
  blocks rays stopped on, and the ground came in rows with sky between them:
  "Between two neighboring rays on ground seen at a grazing angle lie blocks
  that no ray stopped on, so they are never built."
- **The proof.** Two conditions cannot both hold:
  1. only what a ray reaches is built;
  2. what is built is rasterized at more pixels than there are rays.

  10-14 dropped the second: "A pixel is not the final form of the rule, only
  its finest setting: one ray a cell, with the cell shrunk to a pixel."
- **The structure holds both again.** Only the boxes the samples stopped at
  are rasterized (condition 1), and they are handed to every pixel
  (condition 2). [A cell casts a beam](./20261022-a-cell-casts-a-beam.md)
  records it as "the cut was taken". The visible sample's axis refusal and the
  walk again answer part of it; by 10-14's proof the rest returns as rows. The
  rows are that proof come back, not a new problem.
- **The ray's rule is a floor.** Among the five things whose removal brings
  their problem back, 10-14 names "the ray's rule — the zoom its distance asks,
  and a stop at the first block that fills". So the zoom stays the distance's,
  and no density field is needed for the rows: a ray a pixel answers them, as
  ESVO, GigaVoxels and Teardown draw. The prune passed blocks that fill, which
  stood against the same floor, and it is gone; the eye's horizon pays about a
  millisecond of GPU p95 more for it.

## The experiment

- **When the rows began.** The felt speed-up
  ([what made it smooth](../experiment/20261026-what-made-it-smooth.md), main
  at 10-26 22:42, now `9c8562e4f`) came before the rows, and the rows were
  there at 10-27 14:00. Between them lie the box in the node's units and the
  prune (`cf769abc1`, 11:54), and earlier that night the beams passing a slab
  they clear. Exports of these, side by side, seen on the screen and not
  timed, place the start.
- **Pixel and density.** Build the turn above as an experiment on the code as
  it stands, against the state archived on 10-28. Before the rows, the
  drawing ran near the pixel and fast enough.

## Open

- **Where the slant is read.** The beam's corner depths, or the box face met.
- **Control.** Control meets what is drawn at the zoom the eye draws there
  (ARCHITECTURE.md, "The query, the only seam"), and must follow the same zoom
  rule.
- **Cost.** The pixel walks `bf1a3e20a` removed come back. They are timed once,
  at the end, against the backlog's baseline.
- **Confirming the first reading.** Seen side by side on the screen, not timed.
