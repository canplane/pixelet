# The pad at a shard's edge

**2026-10-07**

Every elevation sample a tile holds, its pad included, is the source's value at
that place. The one exception is a coarser tile's pad over a neighbor that is
not baked yet. There the edge sample stands in, and baking the neighbor later
refolds only the tiles whose pad reaches it.

## The question

A tile holds its samples and two more on every side, the pad
(`crates/spec/src/bake/constant.rs` `ELE_PAD`), so the bicubic reaches past its
edge without the neighbor resident. A coarser tile's pad is folded from its
neighbors' children (`crates/spec/src/bake/fold.rs` `fold_ele`). So a pad at any
zoom depends on the tiles beside it, and on the tiles beside those at the zoom
below.

Shards are ingested and baked at different times. The question was what a tile's
pad holds when the shard beside it has not been baked, and what happens when it
is.

## What was weighed

- **One sample field, every zoom, from a wider ingest.** Each zoom's samples
  defined over the whole source, and each shard ingested a sixteenth of its
  width past its edge, so no pad ever waits for a neighbor and nothing is baked
  again. Set aside: the margin and the per-zoom field are state kept to save a
  refold that the next point shows is small.
- **A missing neighbor read as 0.** The interpolant's weights at a tile's edge
  are about [−1/16, 9/16, 9/16, −1/16], so the edge falls to half its height, at
  every zoom, over half a sample: about 16 m at a leaf and about 2 km at z8.
  Rejected.
- **Chosen: the source where it is held, the edge sample where it is not, and a
  refold when it arrives.** A tile's inner samples never read a pad, so a
  changed pad never reaches a parent's inside. Only the row of tiles along the
  shared edge changes at each zoom: 64 + 32 + … + 1 = 127 tiles a shared edge
  from z14 to z8, and the coarser parents, which change anyway.

## How it holds

- **A leaf's pad is the source.** The elevation is ingested a shard at a time
  with an apron of a leaf past its edge, taken out to whole raster blocks, at
  the coverage's outer edge too
  (`crates/server/geo-bake/src/bin/elevation_shards.rs`). A leaf is far wider
  than its pad and the resample's reach, so a leaf's pad is always read from the
  raster (`crates/server/geo-bake/src/elevation.rs` `tile_grid`), never filled.
- **0 means sea, and nothing else.** Where the raster has no value, `tile_grid`
  writes 0. With the apron, that is the source's own void, the sea, never a
  place nobody ingested.
- **A coarser pad over an unbaked neighbor repeats the edge.** `fold_ele` reads
  a pad from the neighbor's inner samples; without the neighbor it reads the
  block's own children's pad, which holds the source one sample out and repeats
  the edge past it. The 0 fallback there is unreachable: a parent exists only
  when its own four children do.
- **The neighbor arriving refolds what changed.** `--grow` bakes the shards the
  coverage gained over the base, and the refold walks up only the tiles whose
  content changed (`crates/server/geo-bake/src/pyramid.rs` `refold`). Coverage C
  baked, then grown by B, equals C ∪ B baked at once.
- **The index follows.** A tile's extent reads its pad, and a parent's extent
  joins its children's, but a border tile's parent is a border tile, so a
  changed extent stays in the same rows.

## Two clamps that are not one

Two different things in this code are called clamping, and they are easy to
confuse:

- **An index held to the grid**: past the padded grid's edge, the edge sample is
  read again (`crates/spec/src/bake/surface.rs` `held`, and `fold_ele`'s
  fallback above). That decides _which sample_ is read.
- **A value held to its samples**: the interpolant's result cut to the lowest
  and highest of the sixteen samples it read, so the bound over the surface is
  the samples' own (`surface.rs` `bicubic`). That decides _what height_ comes
  out.

The first is this record's subject; the second is not.

## Not yet seen

No tile has been baked with the elevation apron yet. The first full bake should
show the coverage's edge tiles standing at their height, not dropping toward the
sea.
