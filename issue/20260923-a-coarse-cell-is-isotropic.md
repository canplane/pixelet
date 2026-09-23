# A coarse cell is isotropic

**Decided 09-23, not built.** A coarse cell is one rule for the ground and
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
- Where the cost bites, the rule is not what gives: a box whose `f` is small
  enough to be under a pixel wherever its level is drawn can be dropped, as
  `level.rs` once dropped a fill under 1/64.
- [Level, step and zoom](./20260921-level-step-and-zoom-are-one-number.md)'s
  paragraph on the elevation lane and its "one thing does not merge" in
  _Objects fold too_ describe the 09-22 height field, and are rewritten when
  this is built.
