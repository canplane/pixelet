# One address, and every scale is a shift of it

Everything in this world has one address: a single 64-bit word. Every
coarser grouping of the world — the batch a renderer draws, the page a
request fetches, the square a floating origin sits in — is that same word
with its low bits cleared. Grouping is a mask, never a division, and a tier
is a shift and nothing else.

What a unit is worth in metres is not in this module. The lattice counts
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
  for a span resolved at runtime, and returns a `Pvf3` because the centre of
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

- `adapter/3857.rs` — EPSG:3857 planar metres. `U_PER_M` and `M_PER_U` are
  the whole of the conversion, and in this adapter `m` always means a planar
  metre, which is latitude-dependent against a real one.
- `adapter/4326.rs` — lon/lat, and the projection to and from 3857.
- `adapter/yup.rs` — a Y-up basis, for a boundary that meets one. Nothing
  imports it, which the file says is the expected state.

### What the adapter makes the tiers, here

The X lane is exactly one EPSG:3857 world. So the lattice is that world cut
2^24 ways, and every tier lands on a web map's zoom — not by design of the
ladder but because the lane width and the world are the same thing.

| tier          | extent | planar metres | tile |
| ------------- | ------ | ------------- | ---- |
| **L1** block  | 1u     | 2.39          | z24  |
| **L2** chunk  | 64u    | 152.9         | z18  |
| **L3** page   | 512u   | 1,223.0       | z15  |
| **L4** anchor | 4096u  | 9,783.9       | z12  |
| **L5** shard  | 65536u | 156,543.0     | z8   |

An L3 page is one 512-pixel z15 tile, and its cells are z16 pixels exactly.
That identity holds by three constants agreeing rather than one deriving the
rest — the X lane is one world, a cell is one unit, a page is 512 cells —
and one assert block in [`bake/constant.rs`](../bake/constant.rs) holds all
three to it and the pyramid's top level besides. That file is outside `space/`,
which is the split above doing its work: the lattice does not know it is a
map.

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

The two ladders meet at one number. A level-k page is zoom `15 − k`, and the
kernel's mesher measures its own step against the same scale, so the
mesher's step, the level a page is held at and a web map's zoom are one
quantity (`issue/20260921-level-step-and-zoom-are-one-number.md`).

## What else is here

|                                  |                                                                                                        |
| -------------------------------- | ------------------------------------------------------------------------------------------------------ |
| [`pvf3/`](./pvf3/)               | the packed words: `ivec`, `fvec`, their pair, and doubles in and out                                   |
| [`adapter/`](./adapter/)         | where a unit gets a meaning                                                                            |
| `hierarchy.rs`                   | `Scale`, the one form a size takes, and parent and child in any number of axes                         |
| `indexer.rs`                     | the tiers, and the four questions each answers                                                         |
| [`lattice/`](./lattice/)         | structures over the lattice: one address hash, points bucketed in 3D, rectangles indexed on the screen |
| [`sparse_tree/`](./sparse_tree/) | an octree in two flat lanes, storing what differs from the default                                     |
| `xy.rs`                          | `TO_XY`, which drops Z to give the key every 2D map surface is keyed by                                |
| `v3.rs`                          | three doubles, for the arithmetic that has left the lattice                                            |
| `mat4.rs`                        | sixteen doubles, column-major: the transforms that arithmetic is carried through                       |
