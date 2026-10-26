# The world is a bowl of bedrock

**2026-10-12** — decided in the conversation.

The ground was stone without a bottom, the world open past the service area, and
nothing kept an edit off ground no source describes. The world now ends in one
material, bedrock, laid as a bowl: a floor under the ground and a wall about the
service area. Bedrock is the catalog's `unknown`, drawn dark gray — the ground
the world holds no more of — so the floor, the wall and a byte no bake wrote are
one fact, not three. It was black first, and hid: past the area nothing is drawn
but the air, which follows the hour, black at night.

Every limit is one module, `pxl-spec block/limit.rs`, which every path calls.

- **The floor** is `BEDROCK_DEPTH`, 64 units, under the ground or sea level,
  whichever is lower: `min(0, face) − 64`. Minecraft opens −64 to 320 on its own
  blocks; it is held from sea level here so a shore below it, the Dead Sea's at
  −211 units, is not buried. It is laid under every stratum, last, so whatever
  is carved later, caves among it, bedrock wins.
- **The ceiling** is `HEADROOM`, 128 units, over the ground or sea level,
  whichever is higher: `max(0, face) + 128`.
- **The wall** is a leaf column thick, just inside the service area: a leaf
  column with ground beside one past the area is bedrock up to its own face
  (`pxl-spec block/sample.rs` `wall_off`), and past the area is void. Bedrock is
  a block of the leaves as the strata are, so a column coarser than the leaves
  folds the wall away as it does any leaf block, and no rule of its own makes it
  a cell of whatever zoom draws it. Laid one cell past the area, it was that
  cell thick, its outer face moving out with every fold and its height the
  highest face about it; inside, its outer face stands on the area's edge, a
  shard's, at every zoom. The service area is the shards the source serves
  (`pxl-kernel io/source.rs` `past_at`), asked of each column and node, never of
  a tile, since a tile past the area is never held and one standing in for it
  may be none: a cell past it reads as `unknown` whatever a tile folded there,
  so the wall moves with the area as more is served, and nothing keeps it. A
  node past the area holds nothing; one beside it holds its ground down to the
  bedrock, so the wall is drawn whole.

Where an edit may go is one function, `pxl-kernel query/limit.rs`
`World::editable`: ground the world knows exactly under it, over its bedrock and
under its ceiling, and no column of the wall: what is `unknown` is neither
placed on nor taken. The write every edit makes passes it, the placement check
asks it, and the pointer points at nothing it refuses.

A walker stays in the service area and off its edge, `World::within` beside it:
the edge is the wall's columns, one rule for both (`pxl-spec
block/limit.rs`
`at_edge`), so it slides along the wall's inner face, as the step-slide meets
any wall, and never stands on the wall.

## Caves

Caves are Minecraft 1.18's noise caves: tunnels where two noises are both near
zero, its spaghetti, and caverns where one is high, its cheese, each noise Ken
Perlin's improved noise on a lattice of its own (`pxl-spec
procedural.rs`
`hollow`). A cave is a pure function of where a cell is and the one seed, so it
runs on across chunks with nothing kept, and nothing bounds its depth. What wins
is an order, first first: bedrock, the ground's face, the ways and the ground
about them, the caves. So a cave never opens the face; it is met by digging.
Whether a cell is a cave's is the hash's alone, the one source: a chunk carves
its own cells by it and the cells it reads past itself alike, so a cave crossing
a seam is one cave on both sides, and control carves the same where the cells
composed are the leaves' or finer (`caves_in`), composing as the chunk that
draws the leaves folded does. What is drawn is the bound's and the demand's,
never the cave rule's. Gating a cave on an edit nearby mixed the two and walled
a cave where its cube held no edit. Where an edit may go is read off the limits
(`floor`, `ceiling`), never the runs: a fold draws no bedrock.

A chunk asks nothing of a chunk beside (`pxl-kernel chunk/build/run.rs`): it
composes one of its own cells past its floor and roof, and its apron past its
sides (`pxl-spec block/sample.rs` `APRON`), from the one function the world is,
and a face there shows only where that cell is open, at whatever zoom the chunk
beside is drawn. The apron is one of its own cells, a fold's where its runs are
folded, so a fold decides its rim at its own resolution too. A cell past every
tile held is not known, no ground, never a stand-in read off the chunk's own
tile: that read the elevation past its padding once the apron was a fold's cell.
Which chunks are drawn is the bound's: a terrain node holds its ground from a
cell under the lowest face about it, its own or beside
(`pxl-kernel layer/terrain/query.rs`), and what is placed; under that every
column is solid, seen by no one.

Bedrock is a stratum, a leaf cell's as the others are: a cell coarser than the
leaves draws its look whole, since one taller than the bedrock's depth folded
black under every far face.

## A cube draws only its own

Two meshes drawing one plane facing one way is what z-fighting is, so no cube
draws past its own space: the block a coarse cube laid under its lowest box and
the wall a fine one hung to the ground beside are gone. What a cube shows is
closed by the cube that holds it, never left to another: under a shrunk cell the
plane is the top of the ground under it, which that ground's own cube draws,
since the cell over it is not full. A shrunk cell drawing that plane instead,
and the cube under leaving its top to it, opened a hole wherever the cube over
was drawn at another zoom. Chunks are drawn under the lowest face about a node,
its own columns' and those beside, so a step down to a neighbour is walled by a
cube's own ground, and wherever the eye's rays land, whatever the bound holds
(`pxl-kernel chunk/domain/walk.rs`): the rays are cast against the world the
hash carves, so a cave seen through a dig is met and drawn as deep as they go,
each chunk of it composed on its own.

The Y lane's ends and the X lane's seam are left for later.
