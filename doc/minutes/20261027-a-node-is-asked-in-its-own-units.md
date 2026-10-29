# A node is asked in its own units

**2026-10-27** — decided with the user. It records what a ray asks the world
and what it is told, and the bound that follows. It is a decision record, not a
rule each change is held to.

> A ray marches the pyramid's grid at the zoom its cone asks, and asks the
> world one node at a time. The world answers in that node's own units: a box
> in its own steps, every axis alike. The look is the LOD's as it was.

## Decided

- **The question is a node.** What a ray asks is a node of the pyramid, its
  zoom and its index on three axes (`cast::Query::node`). Nothing in the walk
  knows what the world is made of: a heightfield, a way or a placed cube is the
  query's business (`world::node`), as
  [a box is all the view asks](./archive/20260928-what-is-drawn-is-what-the-screen-covers.md)
  decided for the view.
- **The answer is in the node's units.** A split answers where its content may
  lie as a box in the node's own steps, 1/1024 of its side, the pyramid ten
  zooms under it, on x, y and z alike (`answer::box_word`, its two block
  words), conservative: what the node holds lies in it. It replaces a span
  along z in leaf cells, which singled one axis out against
  [a coarse cell is isotropic](./archive/20260923-a-coarse-cell-is-isotropic.md).
  Each source gives its own box and the query joins them; a source keeps its
  own measure inside (the ground's span along z), and only the box leaves.
- **The ray is a cone.** It widens with distance, and at each distance it
  steps in the grid of the zoom whose cell is that width (`grid::cell_at`), one
  rule shared with what is fetched and held (`window::RADIUS`) and with the
  beams' spacing (`grid::CELL`). The container a step lands in is the
  cone's; the box inside it is what is drawn there, in eighths, floating as the
  LOD always has. The two are independent.
- **Nothing the ray meets is passed.** A ray enters every split whose box it
  meets and stops at the first block that is not transparent, the eye's and
  the shadow map's alike. Its cone is read only by a beam, as how far apart its
  rays lie (`cast::Spacing`): the eye's an angle, its grid's cell; the sun's a
  width, `sun::BEAM` of its map's texels; control and physics none
  (`Spacing::NONE`). Passing what the cone cannot tell apart, a split's box
  shrunk by half the cone at the node's nearest point
  ([a cell's inside is don't-care](./archive/20260928-what-is-drawn-is-what-the-screen-covers.md)),
  left a row missing whole at grazing angles, which no visible sample could
  bring back. With the eye's prune off the rows went, at about a millisecond of
  GPU p95 at the horizon ([the rows,
  seen](../experiment/20261028-the-rows-seen.md)), which the user took over the
  rows on 10-28. The sun's prune, the same machinery, bought nothing that could
  be measured or seen ([the sun without its
  prune](../experiment/20261028-the-sun-without-its-prune.md)), and the
  mitigations tried with the prune kept held neither the frame nor the rows
  ([the cone touches](../experiment/20261028-the-cone-touches.md), [the
  ring](../experiment/20261028-the-ring.md)).
- **A walk resumed meets what the root's meets.** With nothing passed, a walk
  resumed inside a node and the walk from the root meet the same cells.

## What follows

- **The work a ray does is closed.** Per octave of distance a ray crosses
  about `1/s` of the cone's cells (`s = τ / SPLIT`, about 192 at τ = 1), so a
  ray crosses at most about `(1/s) · log₂(reach / d_near)` of the cone's
  cells, and enters about twice that many nodes, every node over them on its
  path included, plus the near-eye floor where a block is wider than the cone,
  plus the levels it descends. It holds whatever the world holds.

  > **Note (10-28):** τ is gone. The cone's cell is the view's,
  > 1/`CELLS_A_RADIAN` (96) radians (pxl-spec-core grid.rs `CELL`), and the
  > windows are sized `WINDOW_MARGIN` (2) times finer, `SPLIT` (192).
- **The horizon costs what any direction does.** Without the pruning a ray
  grazing a surface passed a node only where it cleared all of its box, so it
  went down to nodes about as tall as its clearance, about `1/s` of them an
  octave: the band just above the horizon ran at the bound's edge. Passing
  what its cone cannot tell apart, a ray grazing ground smoother than the cone
  passes whole nodes, as a ray looking up or down does. At the horizon,
  turning, rough, a load each: the beam span 10.9–12.4 ms to 3.7 at p95, GPU
  p50 17.9 ms to 13.0 and p95 17.0, 59.6 fps; on the fixed path the frame as
  it was (GPU p50 10.4 ms, 51.9 fps).
- **The step is a quantum, kept under the grid.** A box rounded out to a step
  of its node stands over what it holds by up to that step, and a ray grazing
  it clears it that much later. At an eighth of the node (the first cut,
  b64c4c7ae) a large node's top stood far over the grid there and the beam
  span grew, 3.7 ms to 4.5–4.7 ms at p95 on the fixed path; at 1/1024 a node
  ten zooms over the grid still rounds within one of its cells.

## Not taken

- **A child-occupancy mask.** Eight bits naming the children that hold
  anything: isotropic, but half a node along each axis, coarser than the box's
  steps, and over the ground it marks every lower child, so a grazing ray
  enters the same nodes. A box answers more.
- **Drawing a coarse block as its content's box.** It would let an area rule
  leave grazing ground coarse, and change what a far plain looks like. The look
  stays the LOD's.
