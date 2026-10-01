# Decor is its own layer

Decided 2026-10-01. Decor was built inside a terrain chunk's build, held by
the chunk's slot and drawn or dropped with it, and how it was drawn was read
off the chunk's level: a kind's largest model projected at the far edge of
the chunk's band. The impostor tier (gpu/pass/impostor.rs) made the seam
plain — what a tree is drawn as is its own size at its own distance, which
the ground's cube has nothing to say about — so decor becomes a layer of its
own: its own cells, its own frontier, its own lifetime, and the ground's
pages as its only tie to the terrain.

## The population is multiresolution, and has to be

A scattered kind at level L draws at most one candidate an L cell: the cell's
channels, folded by majority, decide whether the kind may stand there, and a
descent keyed to the kind and the node alone picks one z16 cell under it,
whose own stream decides the rest (layer/decor/build.rs `representative`).
A zoom keeps the instances it had and adds or drops a quarter a level.

One z16 population everywhere is not on offer, for two reasons:

- Whether a z16 cell is eligible is its z16 page's channels, and only pages
  near the eye are resident; far away only coarse pages are. Pulling far z16
  pages for decor breaks the view's bound — what is held is bounded by the
  screen, not by the world.
- An L cell covers 4^L z16 cells. The thinning is what keeps instances a
  screen area about constant; without it the count grows fourfold a level
  out.

So a decor cell is a cell of the pyramid, `(level, cell)`, and its
population is the level-L scatter over the level-L page and the seed.

## Invariants

1. **The decor frontier draws no cell and its ancestor at once.** It is an
   antichain over the ground in view, as the terrain's is, decided by decor
   from the eye with the shared rule (`Tau::level`), never read off the
   terrain's frontier. One region holds a level-L cell or its children.
   What is drawn is a cover that moves to that frontier one whole
   replacement at a time: a refining cell stays drawn until every wanted
   cell under it is ready, coarsening cells until their parent is, and what
   waits under a drawn cell is held hidden — so a change of level never
   leaves a region with no decor on it (layer/decor/cell.rs `step`).
2. **Population LOD and presentation LOD are separate.** Which level a cell
   is decides what its block holds, and a change of level is a new block.
   Whether an instance is drawn as its model, its sprite or not at all is its
   actual distance, its actual scale and its variant's extent, chosen a frame
   at a time into transient index lanes; it never rebuilds a block.
3. **Decor depends on the terrain's pages and nothing past them.** A cell
   asks for the level-L page it scatters over; no chunk, mesh or
   materialization state is read — an instance's height is the query's
   (world/access.rs), not the drawn surface's.
   The ask is a demand where a fault could name the page, inside the
   level's fault radius about the eye's foot, so the demand's bound holds
   as it is; a cell split only because a sibling is near reaches past that
   by its parent's diagonal — four pages a side at a level rather than
   three — and its page is a hint there, its parent drawn until it comes
   (world/pyramid.rs `observe`).

## The shape

```text
           eye + Tau
              │
     decor frontier (own antichain)
              │
         level-L cells
       ┌──────┴──────┐
page + seed       object store
(scattered)        (placed)
       └──────┬──────┘
    instance blocks, one a cell
              │
  presentation, a frame at a time
    ┌─────────┼─────────┐
  model    sprite     gone
```

- **Admission is conservative.** Whether a cell is wanted is decided before
  its block exists, so it is read from the largest instance that _could_
  stand in it: the largest extent of the kinds that scatter, at the largest
  scatter scale, and the largest placed size. A built block's own largest
  instance can only narrow it.
- **The two sources stay apart.** Placed decor is the object store's entries,
  as they are; scattered decor is derived. They share the instance format and
  the cell, not their rules: placed entries are not thinned.
- **The hit walks the cells.** A ray crosses the cells in view, each bounded
  by its largest instance, then the instances' bounds, then the asset's
  triangles — never the terrain chunks drawn.
- **Presentation starts on the CPU.** The first solver sorts each cell's
  instances into model and sprite index lanes a frame at a time. A compute
  pass with indirect draws is the same solver moved, if a measurement asks
  for it; what is not done is running both passes over every instance and
  discarding in the vertex shader, which pays the model's vertices for the
  instances the sprite was there to spare.
