# The ground under its face

**2026-10-11** — decided in the conversation.

A column of ground was its look all the way down: a road dug into was asphalt to
the floor, a lawn grass. The ground is a Carto palette
([20261005-the-ground-is-a-carto-palette.md](./20261005-the-ground-is-a-carto-palette.md)),
and Carto paints a face and nothing under it, so the looks under a face come
from the one precedent that lays a column, Minecraft Beta's
`ChunkProviderGenerate.replaceBlocksForBiome`: the biome's top block on the
face, its filler — dirt, or sand where sand lies — `stoneNoise / 3 + 3` deep
under it, `rand.nextInt(4)` of sandstone under sand, and stone under all.

## What is taken

The palette gains a section of its own, `underground`, past Carto's looks:
`soil`, `sandstone` and `stone`, which no bake writes and only the client's
composition lays (`pxl-spec block/sample.rs` `strata`). Their colors are
Minecraft's map colors for dirt, sand and stone, as Carto gives none. Soil wears
the pack's dirt, the tile under land, shipped as painted since it is no Carto
look; sandstone and stone have no tile and are drawn as their colors.

The filler is 3 deep and the sandstone 2, the noise and the draw left out: a
column is laid from its look and depth alone, the same at every read. A noise
over the leaf cells, the one caves would want too, can vary it later.

A face is one cell of its look whatever it is — water, a building, a road — and
sand and beach are sand's filler; the rest is soil's. A tunnel's floor keeps the
tunnel's look.

Strata are a leaf cell's: a chunk whose cells are coarser than the leaves' is a
summary of the faces under it, so its ground is its look whole, as a fold sums a
column as its face. Laid there, a cell many units wide over a face one unit deep
showed its walls as soil and stone from afar.
