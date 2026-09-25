# A coarse cell is isotropic

**Decided 09-23 and built, but for the escape hatch below.** A coarse cell is one rule for the ground and
for objects alike: its size is the share of it that is not air, taken on
every axis, and its colour is what faces the air, weighted by how much of it
does. It undoes one decision of 09-22 and replaces one of 09-09, and is
written apart from
[level, step and zoom](./20260921-level-step-and-zoom-are-one-number.md) so
that the two can be read side by side before either is changed.

## What stands now

A coarse tier merges `step³` cells of the tier below into one box
(`pxl-render mesh/chunk_mesher.rs`). How it does so changed twice.

- **09-09, `7d1320b4`.** Size is occupancy: the fill `f` is the share of the
  `step³` cube that is not empty, and the box shrinks by `∛f` about the
  cell's centre on every axis, so its volume is the fill. Colour is taken
  from the topmost run of each column within a z-bucket — "the voxel you can
  see" — averaged without weight, in sRGB bytes; texture layers are the first
  four distinct ones met. The commit accepted that flat ground then reads as
  cubes standing apart.
- **09-22, `ac5677a2`.** The ground was taken out of that rule. A run that is
  neither a placed object nor a sub-cell is the ground (`is_ground`), and a
  coarse cell's ground is one run from the lowest floor under it to the mean
  of its columns' tops, whole across the cell, its colour the mean of those
  tops' (`pack_ground`). What objects add stacks above it by the 09-09 rule.
  A level chunk's column became a run to the elevation in units rather than
  a top cell inset by its fill. The coarse tier's mesh fell to a fifth.

## What is decided

1. **Air is AIR and VOID alike.** Whatever is not air counts, terrain and
   object with no distinction; nothing is read as the ground.
2. **Size is the fill, on every axis.** A coarse cell's box is `∛f` of the
   cell on each axis about its centre, `f` the share of the cell's `step³`
   voxels that are not air. A full cell is a whole box and merges with its
   neighbours as any run does. Height is not spared: the 09-22 height field
   is withdrawn, because it singles out Z, and a rule that treats one axis
   apart is what the 09-09 commit had already rejected on the evidence that
   thirty-two voxels lying flat and thirty-two standing up came out drawn
   and vanished respectively.
3. **Colour is the exposed surface's.** Each voxel weighs the number of its
   six faces that meet air, and the box's colour is the weighted mean of its
   voxels' colours, taken in linear light. Red with three faces to the air and
   blue with one make a colour three parts red to one blue. A buried voxel
   weighs nothing, so flat ground takes the colour of its top and a cliff or a
   wall the colour of its side, without the view from above the 09-09 rule
   assumed. A box has one colour on every face.
4. **Texture layers are ranked by the same weight**, the four heaviest kept,
   not the first four met.
5. **Heights stay in units** at every level, as 09-22 made them: it is what
   lets `f` be counted exactly where a column's top falls part way into a
   cell, and it does not favour an axis.

Ground and objects share every bucket, so a cell holding the foot of a
building on grass is one box of their mix.

## What it costs

A shrunken box stands apart from its neighbours: it merges with nothing
laterally and hides none of its faces, and a column whose cells below the top
are full and whose top is part full is two boxes rather than one, of colours
that differ once the colour is the exposed surface's. Flat ground, where the
ground is thin against a coarse cell, is mostly such cells. On 09-22 the
isotropic rule's coarse tier was five times the height field's, measured on
vertex meshes; boxes have not been measured.

The heap is not where it lands any more — a mesh lives in a slot on the
device (`world/chunk/mesh_slot.rs`) — but the coarse slot's 1,365 boxes were
measured under the height field and would cut, and the device's arena and
the vertices drawn grow with the boxes. The slot is to be measured again under
this rule and sized to it.

## Open for the build

- A face on a chunk's edge meets air or not by the apron column the run set
  already carries.
- A sub-cell object's exposed area is its side's fraction squared a face, as
  its volume is the fraction cubed.
- A bucket none of whose voxels faces the air still shows once its neighbours
  shrink; its colour falls back to the volume's mean.
- **Tried and reverted, 09-23.** Where the cost bites, the rule was not what
  would give: a box too small to stand over its own floor would not be
  written and its floor would rise to where its top was — the cell keeping
  its height and its colour and losing only the inset, one run rather than
  two.

  What such a drop costs is the notch it leaves at the cell's rim, `∛f` of
  the cell. A drawn cell is between one and two `TARGET_CELL_PX` on screen
  (`pxl-render look/lod.rs`): the ladder hands a tier off when its cell falls
  to one, so it is never drawn under that and never over twice it. At
  `∛f = 1/4` the notch is one and a half to three pixels, a quarter of the
  detail the ladder had already spent choosing this tier, and the pixels
  cancel — the threshold is a pure `f < 1/64` at every level, which is also
  the fill `level.rs` once dropped at.

  **It does not pay.** Measured over a zoom out and back over Seoul, 150u to
  19,200u and back, reading the ledger's `work coarse_boxes` — the largest a
  coarse tier asked its slot for:

  | fill dropped | cells dropped | coarse tier's largest ask |
  | ------------ | ------------- | ------------------------- |
  | none         | —             | 3,478                     |
  | 1/64         | 0.36%         | 3,436 and 3,547, twice    |
  | 1/8          | 2.9%          | 3,287                     |

  At 1/64 the cull off (3,478) falls between the two readings with it on: it
  is noise. At 1/8 — the ladder's whole budget, a three to six pixel notch —
  it is 5%, which that notch does not buy.

  The measurement says why. A coarse tier is about 3.4 runs to a cell: the
  full buckets under the surface, merged into one; the floor that fills the
  gap under the top bucket's box; and that box. A column's ground run is
  thick, so its top bucket's fill is rarely near 1/64, and only the buckets
  that barely reach into their cell are ever caught. No threshold moves the
  3.4.

- [Level, step and zoom](./20260921-level-step-and-zoom-are-one-number.md)'s
  paragraph on the elevation lane and its "one thing does not merge" in
  _Objects fold too_ describe the 09-22 height field, and are rewritten when
  this is built.

## The floor is a face, 09-25

The floor that 09-23 lays under an inset box was a run from the cell's bottom
up to the box: a column with the box on it, which read as a box standing on a
pillar. It is now the cell's bottom face alone — a run with no height, drawn
by its top — so the box stands clear over it, and the lowest box's skirt
stays, since what it hides is the step between two cells' floors.

The box's size and colour are still the one rule for the ground and objects
alike; the floor is not. A cell with any ground in it lays one, and a cell of
placed things alone lays none: its box floats at the cell's centre, and is a
placed thing's box, drawing its underside as its own
(`pxl-render mesh/chunk_mesher.rs` `pack_coarse_cell`).
