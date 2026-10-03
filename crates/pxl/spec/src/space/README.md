# One address, and every scale is a shift of it

Everything in this world has one address: a single 64-bit word. Every
coarser grouping of the world — the batch a renderer draws, the page a
request fetches, the square a floating origin sits in — is that same word
with its low bits cleared. Grouping is a mask, never a division, and a tier
is a shift and nothing else.

What a unit is worth in meters is not in this module. The lattice counts
units; what a unit means belongs to whoever projects it. That split is why
`indexer.rs` names tiers and no distance, and it is the one thing to
understand before reading anything here.

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
does not pay here is [pvf3/README.md](./pvf3/README.md).

A position between two cells is a `Pvf3`: the `IVec` and an `FVec` beside
it, which holds each axis's place within its cell in 256ths, the two reading
as one fixed-point scalar an axis. Two positions are equal when their words are, and a sum lands on a
lane rather than between two doubles.

## A tier is a shift

`indexer.rs` names five tiers. Each is a cumulative bit distance from the
base unit, so a tier's extent is a power of two and its boundary is a mask.

| tier          | shift | extent | inside the one above | what this program puts there |
| ------------- | ----- | ------ | -------------------- | ---------------------------- |
| **L1** block  | 0     | 1u     | —                    | the canonical unit           |
| **L2** chunk  | 6     | 64u    | 8×8 in L3            | a render and LOD batch       |
| **L3** page   | 9     | 512u   | 8×8 in L4            | a request and paging unit    |
| **L4** anchor | 12    | 4096u  | 16×16 in L5          | a floating origin's square   |
| **L5** shard  | 16    | 65536u | —                    | nothing yet                  |

The tiers are scale and nothing else — the rightmost column is this
program's arrangement, not the ladder's meaning. L0, which the module's own
doc names, is the `FVec` above: a fraction of a unit rather than a tier. L5
is a name and a shift only, its helpers commented out and nothing reaching
for them.

Each tier answers four questions about an address, and the four are one mask
and one shift between them:

- `L3_BASE(addr)` — the tier boundary at or below it, its low bits cleared.
  Inclusive, where `L3_LIMIT` is exclusive.
- `L3_CENTER(addr)` — the middle of that square. `SPAN_CENTER` is the same
  for a span resolved at runtime, and returns a `Pvf3` because the center of
  a 1u cell is 0.5u and lives in the fraction.
- `L3_IDX(addr)` — which square, counted in squares rather than units: the
  base shifted down. `FROM_L3_IDX` shifts it back.
- `L2_IDX_IN_L3(addr)` — where the child sits inside the parent, the bits
  between the two tiers.

A tier's extent can reach a lane's width, and then that lane collapses: its
base and its limit meet, and it contributes no partition above that tier.
That is a consequence rather than a special case — Z is 16 bits wide and L5
is a shift of 16, so L5 is horizontal, and nothing had to say so.

## A unit has no size

`pvf3/projection.rs` states the rule: _"The scalar space is normalized: 1.0
is one lattice unit, and whatever a unit is in meters is the caller's scaling
outside this module."_ Nothing under `pvf3/` or in `indexer.rs` carries a
distance.

Meaning enters at an **adapter**, and only there:

- `adapter/3857.rs` — the DB's EPSG:3857 coordinates. Lengths are the
  plane's, in u, or the ground's, in metres; `scale_at` relates them, and
  `to_db`/`from_db` are the only way to or from the DB's coordinates.
- `adapter/4326.rs` — lon/lat, and the projection to and from 3857.
- `adapter/yup.rs` — a Y-up basis, for a boundary that meets one. Nothing
  imports it, which the file says is the expected state.

### What the adapter makes the tiers, here

The X lane is exactly one EPSG:3857 world. So the lattice is that world cut
2^24 ways, and a cell, the ground's atom, is a z16 pixel of the standard's
256-pixel tiles exactly — not by design of the ladder but because the lane
width and the world are the same thing.

| tier          | extent | planar meters |
| ------------- | ------ | ------------- |
| **L1** block  | 1u     | 2.39          |
| **L2** chunk  | 64u    | 152.9         |
| **L3** page   | 512u   | 1,223.0       |
| **L4** anchor | 4096u  | 9,783.9       |
| **L5** shard  | 65536u | 156,543.0     |

That identity holds by two constants agreeing — the X lane is one world, a
cell is one unit — and any cell's zoom follows from its size alone
([`adapter/3857.rs`](./adapter/3857.rs) `zoom_of`). A page is no part of it: a page only bundles cells into a
file, and its name's length follows its size, never a zoom.

## The pyramid is not a tier

A terrain page is fetched at several resolutions, and those levels are
**not** tiers of this ladder.

```
z16            z15
┌────┬────┐    ┌─────────┐
│    │    │    │         │    the same 512-cell record,
├────┼────┤    │         │    a cell of 2u instead of 1u,
│    │    │    │         │    over the four pages beside it
└────┴────┘    └─────────┘
```

A level-k page holds the same record a leaf does — 512 cells a side — over
2^k leaves a side, so its cells are 2^k units. Its key is a leaf key shifted
down k, and a key alone does not say by how much, so a page coarser than z16 is
named by a level and a key together (`LevelPage`, `pxl-spec
space/pyramid.rs`). `L3_IDX` always answers the leaf question: which
512u square.

The two ladders meet at one number. A level-k page's cells are z(16 − k), and
the kernel's mesher measures its own step against the same scale, so the
mesher's step, the level a page is held at and a web map's zoom are one
quantity (`issue/20260921-level-step-and-zoom-are-one-number.md`).

## The trees share their geometry and nothing else

Every tree in this world is dyadic. A node's parent is its coordinate
halved and its children are the 2^D under it (`hierarchy.rs`), its size is a
`Scale`, a cube of a scale is one box (`Scale::cube`), and a ray crosses a
box by one slab test (`Aabb::span_along`, in `v3.rs`). That much is one law,
written once. What a node holds, whether it is resident, how it is stored
and how a reader walks it are each tree's own, and follow from what it
represents:

|                                                       | axes   | resident                     | a node holds                              | stored                                             | walked                                             |
| ----------------------------------------------------- | ------ | ---------------------------- | ----------------------------------------- | -------------------------------------------------- | -------------------------------------------------- |
| the pyramid (`pyramid.rs`)                            | 2      | paged: a page can be missing | a layer's fold of all under it            | a record a page                                    | from a point, up to the nearest resident page      |
| a page's index (`bake/format.rs` `Index`)             | 2      | with its page                | the lowest and highest drawn under a cell | dense lanes in the page, and blocks of them        | a coarse cell passed whole where a ray is above it |
| the sparse tree (`sparse_tree/`)                      | 2 or 3 | whole                        | the way down, and a fold if asked         | two flat lanes, only what differs from the default | down a key's path, or every leaf                   |
| the objects' folds (pxl-kernel layer/object/store.rs) | 3      | whole                        | the majority of its eight                 | in a band's tree under a band, a map over it       | one read a cell                                    |
| the objects' bounds (the same)                        | 3      | whole                        | the box of everything under it            | a map, from a band up to the widest box asked      | down by a box, or along a ray nearest first        |

The pyramid is a tree across levels: every page holds a summary of the
ground under it — terrain's cells voted and averaged, places summed — so a
far view draws a coarse level as it is, the way a mipmap is read. Placed
objects need a tree of another kind: they are all resident, mostly empty
space, and three-dimensional, so what matters is storing one level without
its empty cells. That is the **sparse tree**, a tree inside one level whose
inner nodes are the way down and, where the caller asks, a summary too: a
folded write (`set_folded`, in `sparse_tree/mem.rs`) refolds every branch it
passed through by the caller's fold, and a walk can stop at a depth and read
a branch there by its fold (`walk_to`).

The two ways of stopping are one rule. A node stops growing where it
already says all of the ground under it, and a block of equal cells is the
case where any summary loses nothing. The pyramid asks it of the layer's fold
(`Folds::settles`, in `pyramid.rs`): a page of places settles a child when
no cell over it holds two places, so the tree of places is only as deep as
a crowd needs, and a layer whose fold is lossy, like terrain's vote, never
settles one.

They stay two because one is paged and the other is not. Objects keep their
coarse levels stored all the same, refolded along a write's path rather than
folded again when a chunk is built: inside a band in its tree's branches, and
past a band in the object store's own records a level. Were objects ever
paged, those records would be a layer of the pyramid, and their octree's
collapse its `settles`.

A reader that crosses two trees walks each its own way and puts the answers
together (pxl-kernel world/access.rs `first_hit`). A ray reads the ground
again from each point along it, since a page over it may be missing and a
descent would stall on the gap, and it walks the objects' bounds nearest
first, since none is missing. The spatial hash (`lattice/spatial_hash.rs`)
is no tree, on purpose: for many points moving in a small volume a tree
prunes nothing.

## What else is here

|                                  |                                                                                                        |
| -------------------------------- | ------------------------------------------------------------------------------------------------------ |
| [`pvf3/`](./pvf3/)               | the packed words: `ivec`, `fvec`, their pair, and doubles in and out                                   |
| [`adapter/`](./adapter/)         | where a unit gets a meaning                                                                            |
| `hierarchy.rs`                   | `Scale`, the one form a size takes, and parent and child in any number of axes                         |
| `indexer.rs`                     | the tiers, and the four questions each answers                                                         |
| `lod.rs`                         | the level a distance is drawn at, and the pages of a level an observation can fault on                 |
| [`lattice/`](./lattice/)         | structures over the lattice: one address hash, points bucketed in 3D, rectangles indexed on the screen |
| [`sparse_tree/`](./sparse_tree/) | an octree in two flat lanes, storing what differs from the default                                     |
| `xy.rs`                          | `TO_XY`, which drops Z to give the key every 2D map surface is keyed by                                |
| `v3.rs`                          | three doubles, for the arithmetic that has left the lattice                                            |
| `mat4.rs`                        | sixteen doubles, column-major: the transforms that arithmetic is carried through                       |
