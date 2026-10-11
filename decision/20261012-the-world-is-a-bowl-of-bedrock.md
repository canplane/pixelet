# The world is a bowl of bedrock

**2026-10-12** — decided in the conversation.

The ground was stone without a bottom, the world open past the service area,
and nothing kept an edit off ground no source describes. The world now ends
in one material, bedrock, laid as a bowl: a floor under the ground and a wall
about the service area. Bedrock is the catalog's `unknown`, drawn black — the
ground the world holds no more of — so the floor, the wall and a byte no bake
wrote are one fact, not three.

Every limit is one module, `pxl-spec block/limit.rs`, which every path calls.

- **The floor** is `BEDROCK_DEPTH`, 64 units, under the ground or sea level,
  whichever is lower: `min(0, face) − 64`. Minecraft opens −64 to 320 on its
  own blocks; it is held from sea level here so a shore below it, the Dead
  Sea's at −211 units, is not buried. It is laid under every stratum, last,
  so whatever is carved later, caves among it, bedrock wins.
- **The ceiling** is `HEADROOM`, 128 units, over the ground or sea level,
  whichever is higher: `max(0, face) + 128`.
- **The wall** is one cell thick, just past the service area: a column with
  no ground beside one with ground is bedrock up to that ground's face
  (`wall`), the highest beside it. The service area is not sent: a tile says
  it cell by cell, every cell inside it a look and every one past it none
  (`pxl-geo-bake terrain/absent.rs`), so the wall moves with the area as more
  is baked, and nothing keeps it. A node past the area reads the index cells
  about it for the wall's height, as the column reads the columns about it.

Where an edit may go is one function, `pxl-kernel query/edit.rs`
`World::editable`: ground the world knows exactly under it, over its bedrock
and under its ceiling. The write every edit makes passes it, the placement
check asks it, and the pointer points at nothing it refuses.

## Caves

Caves are Minecraft 1.18's noise caves: tunnels where two noises are both
near zero, its spaghetti, and caverns where one is high, its cheese, each
noise Ken Perlin's improved noise on a lattice of its own (`pxl-spec
procedural.rs` `hollow`). A cave is a pure function of where a cell is and
the one seed, so it runs on across chunks with nothing kept, and nothing
bounds its depth. What wins is an order, first first: bedrock, the ground's
face, the ways and the ground about them, the caves. So a cave never opens
the face; it is met by digging.

A column is still composed from four cells under its lowest face, so a cave
shows within that, and below it where a hole is dug. Composed down to the
bedrock, a cube under one cut by sea level held ground and drew its top in
the plane where the cube over laid the block under its shrunk box; the two
fought over every low shore. Composing a chunk's own heights alone is next.

Bedrock is a stratum, a leaf cell's as the others are: a cell coarser than
the leaves draws its look whole, since one taller than the bedrock's depth
folded black under every far face.

## A cube draws only its own

Two meshes drawing one plane facing one way is what z-fighting is, so no
cube draws past its own space: the block a coarse cube laid under its lowest
box and the wall a fine one hung to the ground beside are gone. What they
covered is the composition's: a chunk's ground is composed under the lowest
face of its columns and the apron's (`pxl-spec block/sample.rs`
`floor_of`), so a step down to a neighbour is walled within the cube, and
past the leaves a whole fold cell under that, so a shrunk cell stands on
ground. How far is one function, `drawn_under`, which the terrain bound
reads too (`pxl-kernel layer/terrain/query.rs`): a cube holding ground is
one drawn, and the cube under one draws the plane between them.

The Y lane's ends and the X lane's seam are left for later.
