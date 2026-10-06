# A chunk goes past the leaves

**2026-10-06** — decided in conversation and built the same day. It takes
the step [every size is a zoom](./20261006-every-size-is-a-zoom.md) left:
objects drawn in chunks of their own zoom, as the ground is.

The chunk tree stopped at the leaves. Past them a second mechanism took
over: a leaf chunk's **tier**, the cells it was drawn in, chosen by its
distance — its own or its cells 2 × 2 averaged, and on a leaf holding
objects, finer down to their finest, packed as sub-cell boxes into the leaf
chunk's voxel volume. So the ground and the objects did their LOD two ways,
and a 64u leaf chunk near the eye could hold 512 of the objects' cells a
side in one mesh.

## What a chunk is now

- **One tree, past the leaves where something finer is held.** A chunk is
  64 cells a side, the cells the LOD rule measures, and splits into its
  eight children while their error crosses an observed cell and something
  under it is held finer than it draws (`Bounds::finest`). The ground stops
  where a chunk draws a leaf's cells, z24; objects go on to their finest,
  z27. A chunk is addressed by its own zoom.
- **Drawn a step coarser, as a retina draws.** A chunk draws its cells'
  parents, 32 a side (`DRAWN_STEPS`), as before every chunk but a leaf's own
  tier did, each drawn cell the mean of the four tile cells under it — their
  majority look, their mean height and the mean of what they show — read
  from the tile whose cells are the chunk's own (`ChunkSample::read`). An
  object's drawn cell keeps the fold's majority for its shape and shows the
  mean of its eight children a zoom finer (`Painter::mean_of`). Past the
  leaves the ground refines the leaf's sample, inventing nothing.
- **No tier.** A chunk has one form, or, at the top of the view, the cover's
  one cell. The tier, its retiering, the leaf's volume paint, the band scan
  and the objects' sub-cell boxes go; every chunk's runs are written one way,
  a column at a time from the sample and the folded objects.
- **Demand follows the rule's cell.** A tile is asked for where its cells
  are the ones the rule measures (`Tau::z_at`), a zoom finer than drawn: a
  drawn cell's color is its children's, which a mixed place — a road through
  a park, a field's edge — needs.

## Why 32 drawn cells and not 64

Drawing 64 cells a chunk, each its own, was tried first. It doubles the
chunk against the rule's cell, and a split takes all eight children, so the
far ones of a chunk split for its near edge were drawn up to two zooms
finer than their distance asks, where the leaf's tier had kept them one.
32 drawn cells is the grain every chunk above the leaves already had.

## Why not the bake's fold, or a mean stored by the bake

Reading the tile whose cells are drawn was the first form: a quarter of the
bytes, the bake's fold already 2 × 2. But the fold keeps one look a cell, the
majority, so a mixed place drew as one of its looks and the colors went
poor. Storing a mean color beside the look in the bake cannot work either:
what a look shows is the client's — the pack, textures on or off — so the
bake cannot know the color to average.

## What it costs

More, smaller chunks near objects, one mesh each, where a leaf chunk was
one mesh; every chunk's mesh is bounded the same at every zoom. Four times
the ground's bytes a drawn cell against reading the drawn tile, and a second
fold of the objects a zoom finer for their colors. The far
ring's cover still summarizes its chunk into one cell, the one place the
mesher averages.
