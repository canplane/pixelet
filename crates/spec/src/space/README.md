# One address, and every size is a zoom

Everything in this world has one address: a single 64-bit word. Every
coarser grouping of the world — the cell an object fills, the chunk a
renderer draws, the tile a request fetches — is that same word shifted down
each lane. A size is a **zoom**, one number for all of them, and grouping is
a shift, never a division.

What a unit is worth in meters is not in this module. The lattice counts
units; what a unit means belongs to whoever projects it. That split is why
no file here carries a distance, and it is the one thing to understand
before reading anything here.

## An address

A point on the lattice is an `IVec`: one `u64` holding three signed lanes.

```
 63                      40 39                      16 15                 0
┌──────────────────────────┬──────────────────────────┬────────────────────┐
│       X  (24 bits)       │       Y  (24 bits)       │    Z  (16 bits)    │
└──────────────────────────┴──────────────────────────┴────────────────────┘
```

One word rather than three so that an address is one comparable, hashable
key. X carries a cyclic seam, its two ends meeting where the world wraps; Y
and Z are plain signed extents. Z is the narrowest because altitude needs the
least room, and the three widths are what fit one word.

Signedness is a lane's own interpretation, not a storage rule, so every
operation on a lane is per lane. `ivec::sh` shifts each lane separately and
never the packed word: one raw word shift leaks bits across a lane boundary
and corrupts every lane but the outermost, positive or negative alike. The
same holds for add, subtract and compare, and why the obvious SWAR shortcut
does not pay here is [coord/pvf3/README.md](./coord/pvf3/README.md).

A position between two cells is a `Pvf3`: the `IVec` and an `FVec` beside
it, which holds each axis's place within its cell in 256ths, the two reading
as one fixed-point scalar an axis. Two positions are equal when their words are, and a sum lands on a
lane rather than between two doubles.

## A size is a zoom

The X lane is one world of 2^24 units, so a thing the world over 2^z wide is
at zoom z: z is 24 less the log of its side in units (`coord/zoom.rs`). A bigger
zoom is finer, one zoom finer is half as wide, and the unit itself is z24.

| zoom | side | what this program puts there                     |
| ---- | ---- | ------------------------------------------------ |
| z27  | 1/8u | a placed object's finest cell                    |
| z24  | 1u   | a ground cell, the canonical unit                |
| z15  | 512u | a leaf tile, the finest the ground is fetched at |

The zooms are size and nothing else — the rightmost column is this program's
arrangement, not the number's meaning. A zoom finer than z24 is a fraction
of a unit and lives in the `FVec`, down to z32, a 256th.

A size answers the same questions at every zoom (`coord/zoom.rs`): its side in
units (`units`), how many of a finer zoom's cells stand across it (`count`),
which cell a point lies in and where that cell begins (`index_of`,
`base_of`), and its box (`cube`). An address goes to its cell's index and
back the same way, a shift of each lane (`idx`, `from_idx`). The world is
zoom 0's cell, once around in X (`ROUND`, `around`, `wrap_index`), and a
stored X stands where it is nearest the eye (`nearest`); all of it is `coord/world.rs`'s,
not PVF3's, which is only the format.

A zoom's side can reach a lane's width, and then that lane collapses: Z is
16 bits wide, so from z8 coarser a cell is horizontal, and nothing had to
say so.

## A unit has no size

`coord/pvf3/pack.rs` states the rule: doubles go in and out with 1.0 as one
lattice unit, and whatever a unit is in meters is the caller's scaling. Nothing under `pvf3/` carries a
distance.

Meaning enters at an **adapter**, and only there:

- `coord/adapter/3857.rs` — the DB's EPSG:3857 coordinates. Lengths are the
  plane's, in u, or the ground's, in meters; `scale_factor` relates them, and
  `mer_from_u`/`u_from_mer` are the only way to or from Mercator's meter.
- `coord/adapter/4326.rs` — lon/lat, and the projection to and from 3857.

### What the adapter makes the zooms, here

The X lane is exactly one EPSG:3857 world. So the lattice is that world cut
2^24 ways, and a z24 cell is a z16 pixel of the standard's 256-pixel tiles
and a z15 tile its z15 tile, exactly — not by design of the number but
because the lane width and the world are the same thing.

| zoom | side  | planar meters |
| ---- | ----- | ------------- |
| z24  | 1u    | 2.39          |
| z18  | 64u   | 152.9         |
| z15  | 512u  | 1,223.0       |
| z12  | 4096u | 9,783.9       |

That identity holds by two constants agreeing — the X lane is one world, a
ground cell is one unit — and a web map's zoom is the adapter's to name
([`coord/adapter/3857.rs`](./coord/adapter/3857.rs) `web_zoom_of`), for comparing with
one; nothing in the pyramid counts in it.

## The pyramid

A terrain tile is fetched at several zooms, z15 to z5, each a tile of the
same record.

```
z15            z14
┌────┬────┐    ┌─────────┐
│    │    │    │         │    the same 512-cell record,
├────┼────┤    │         │    z23 cells instead of z24,
│    │    │    │         │    over the four tiles beside it
└────┴────┘    └─────────┘
```

A tile at z holds the same record a leaf does — 512 cells a side — so its
cells are z + 9, a layer's cells always its tile's zoom plus the layer's own
constant (`Lattice`). Its key is a leaf's key shifted down 15 − z, and a key
alone does not say by how much, so a tile is named by its zoom and a key
together (`Tile`, `layer/tile.rs`); `idx(LEAF_ZOOM, addr)` always answers the leaf
question: which 512u square.

The distance chooses a cell zoom (`camera/grid.rs` `cell_at`), and terrain's tile
zoom is the one whose cells those are (`Lattice::z_of`); the kernel's mesher
measures its own step against the same cell zoom
([every size is a zoom](../../../../decision/20261006-every-size-is-a-zoom.md)).

## The trees share their geometry and nothing else

Every tree in this world is dyadic. A node's parent is its coordinate
halved and its children are the 2^D under it (`hierarchy/tree.rs`), its size is a
zoom, a cube at a zoom is one box (`coord/zoom.rs` `cube`), and a ray crosses a
box by one slab test (`Aabb::span_along`, in `coord/v3.rs`). That much is one law,
written once. What a node holds, whether it is resident, how it is stored
and how a reader walks it are each tree's own, and follow from what it
represents:

|                                           | axes | resident                             | a node holds                                                    | stored                                                           | walked                                                          |
| ----------------------------------------- | ---- | ------------------------------------ | --------------------------------------------------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------- |
| the pyramid (`layer/tile.rs`)             | 2    | resident by tile: one can be missing | a layer's fold of all under it                                  | a record a tile                                                  | from a point, up to the nearest resident tile                   |
| a tile's index (`bake/format.rs` `Index`) | 2    | with its tile                        | the lowest and highest drawn under a cell                       | dense lanes in the tile, and coarser cells of them               | a coarse cell passed whole where a ray is above it              |
| the objects (pxl-kernel object/store.rs)  | 3    | whole                                | its leaf's value, or the majority of its eight, read when asked | the leaves alone, one tree a column (`hierarchy/sparse_tree.rs`) | down a cube's key range, by a box, or along a ray nearest first |

The pyramid is a tree across zooms: every tile holds a summary of the
ground under it — terrain's cells voted and averaged, places summed — so a
far view draws a coarse zoom as it is, the way a mipmap is read. Placed
objects need a tree of another kind: they are all resident, mostly empty
space, and three-dimensional, so what matters is storing them without
their empty cells. They are kept as their leaves alone, in a **sparse
tree** whose branches hold nothing but the way down: a node's summary is
read from what is under it when asked, a write splits only its path, and
eight equal leaves are joined into their parent.

The two ways of stopping are one rule. A node stops growing where it
already says all of the ground under it, and a coarser cell of equal ones is the
case where any summary loses nothing. The pyramid's tile says it in its
occupancy (`bake/format.rs` `Occupancy`), which children are tiles, written by
the bake's fold: a tile of places marks a child none when no cell over it
holds two places, so the tree of places is only as deep as a crowd needs,
and terrain marks only a child that is the zero tile, its vote being lossy.

They stay two because one is resident by tile and the other is not. Objects
keep no coarse zoom: a chunk folds a node's leaves when it is built. Were
objects ever resident by tile, their branches would be a layer of the
pyramid, and a block of equal leaves joined its occupancy.

A reader that crosses two trees walks each its own way and puts the answers
together (pxl-kernel world/access.rs `first_hit`). A ray reads the ground
again from each point along it, since a tile over it may be missing and a
descent would stall on the gap, and it walks the objects' bounds nearest
first, since none is missing.

## What else is here

|                              |                                                                                                                                         |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| [`coord/`](./coord/)         | where and how big: the packed words (`pvf3/`), sizes as zooms (`zoom.rs`), what the lanes mean (`world.rs`), the adapters, `v3`, `mat4` |
| [`hierarchy/`](./hierarchy/) | parent and child in any number of axes (`tree.rs`), a node's one key (`morton.rs`), a sparse octree (`sparse_tree.rs`)                  |
| [`camera/`](./camera/)       | the camera grid, its cell zoom a distance is drawn at and the chunk (`grid.rs`), and the eye (`eye.rs`)                                 |
| [`layer/`](./layer/)         | a layer's lattice (`lattice.rs`), the pyramid of tiles (`tile.rs`), and what an observation demands of them (`demand.rs`)               |
