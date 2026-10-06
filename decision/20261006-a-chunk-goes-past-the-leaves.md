# A chunk goes past the leaves

**2026-10-06** — decided in conversation and built the same day, then set
right the same day. It takes the step
[every size is a zoom](./20261006-every-size-is-a-zoom.md) left: objects
drawn in chunks of their own zoom, as the ground is.

The chunk tree stopped at the leaves. Past them a second mechanism took
over: a leaf chunk's **tier**, the cells it was drawn in, chosen by its
distance — its own or its cells 2 × 2 averaged, and on a leaf holding
objects, finer down to their finest, packed as sub-cell boxes into the leaf
chunk's voxel volume. So the ground and the objects did their LOD two ways.

## What was decided

- **The camera grid is the resolution.** Its cell is the distance's, about
  twelve pixels at the reference view, and one cell of it is one thing drawn
  (`pxl-spec space/lod.rs` `SPLIT`, the one knob of it). Inside a cell
  nothing is promised: what lies there is folded into one by its layer.
- **One tree, past the leaves where something finer is held.** A chunk is
  32 of the grid's cells a side (`CHUNK_STEPS`), the batch a view is built
  and drawn in, never seen. It splits into its eight children while their
  error, a cell on every axis, crosses an observed cell and something under
  it is held finer than its cells, read off what is stored
  (`Bounds::finest`). The ground stops at a leaf's cells, z24; objects at
  their finest held. A chunk is addressed by its own zoom. No tier, and no
  second form: the roots drawn all around the eye are chunks a chunk's worth
  coarser than the coarsest the reach draws, each cell one piece of what a
  one-cell cover had drawn, with their room kept apart in the coarse pool
  rather than in a pool of their own.
- **Folding is the layer's.** A cube folds its records one zoom finer
  (`space/pyramid.rs` `Lattice::fold`, the ground's `GROUND`): a cell's color is the mean of its
  children's top surfaces, with up to four of their texture layers, and
  what objects add is a box of the fill's volume, kept in the children's
  eighths, shrunk about the cell's center (`mesh/chunk_mesher.rs`). Places
  and decor fold their own way, and the Earth follows the grid in its own
  proportion (`bake/earth_tile.rs` `EARTH`, its lattice). Nothing outside a layer
  knows how it folds.
- **A fold is a summary.** Nothing in a folded cell is struck, hidden, cut
  by a preview or pointed at, without exception.
- **Knobs are orthogonal.** The grid (`SPLIT`), the chunk (`CHUNK_STEPS`)
  and the meshes' bytes per pool (`param.rs`) are each one number, and
  what depends on one is derived from it; the store's band is a storage
  format of its own (`bake/constant.rs` `BAND`), apart from how a view is
  chunked.

## Why 32

Chosen for a real-time engine against 64: a placed block rebuilds a
quarter of the columns, a build is short enough to slice into a frame, and
a chunk split for its near edge draws less too finely at its far one. The
cost is four times the chunks over an area, which is draws and table
entries, not what is drawn. Voxy, which folds a voxel world the same way,
uses 32³ sections at every level.

## What went wrong on the way

The first build drew every chunk a step coarser than its cells under a
separate constant, read the tile whose cells it drew so the bake's majority
replaced the mesher's mean, and then, set back toward the mean, averaged
four tile cells in the sampler and eight object cells beside the fold.
Each was a second rule for what is inside a cell, outside the layer that
owns it, and the coarse cell's surface mean, fill box and texture layers
went missing in between. A mean color stored by the bake was rejected:
what a look shows is the client's — the pack, textures on or off — so the
bake cannot know the color to average.
