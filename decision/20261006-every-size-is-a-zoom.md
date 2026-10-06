# Every size is a zoom

**2026-10-06** — decided in conversation and built the same day. It carries on from
[a zoom names a page grid](./20261006-a-zoom-names-a-page-grid.md), whose
page-grid zoom is the tile zoom here, every value unchanged.

The X lane is one world of 2^24 units, so a thing world / 2^z wide is at
zoom z, and z = 24 − log2(its side in units). That one number names every
size there is: a cell's, a chunk's, a tile's. Sizes have been said three
ways — a zoom for a page, a `Scale` for a cell, a tier (L1 to L5) for a
fixed extent on a leaf — and the conversions between them were where facts
went double; today's fold tie was three numbers for one parity.

## The words

- **Cell**: the granule at a zoom. The ground's cell is z24, a unit (1u is
  a z24 cell's side); objects' finest is z27. Where the finest a layer
  holds is meant, it is the leaf cell.
- **Chunk**: what is built and drawn as one, 64 cells: a chunk's zoom is
  its cells' less 6.
- **Tile**: what is fetched and stored as one, the convention's word: a
  layer's tile holds 2^k cells a side, and its zoom is its cells' less k
  (terrain's k is 9, so z24 cells make z15 tiles; POI's is 3). A tile's
  zoom equals a web map's tile zoom; a z24 cell is a web map's z16 pixel,
  which the 3857 adapter alone says.
- **Block**: a thing placed, never a size.
- **LOD** is the policy's name; what it yields is a cell zoom.

In code, `pub type Zoom = u8`, and a name says whose zoom it is:
`cell_zoom`, `chunk_zoom`, `tile_zoom`. A plain `z` is the height axis and
nothing else. Sizes are zoom arithmetic: a chunk of cells is
`cell_zoom − 6`, the cell a step coarser `cell_zoom − 1`.

## What goes

- `Scale` (`pxl-spec space/hierarchy.rs`), for `Zoom`.
- The tiers L1 to L5 (`space/indexer.rs`): each was a role at a leaf, now
  named by its role and zoom. L5, a shard, names no size that follows a
  zoom and was never built, and is deleted.
- **Page**, for tile: `Page`, `page_scale`, `PAGE_CELLS`, `world/paging.rs`
  and the rest. The kernel's memory pages (`.fixed`, `PAGE_BYTES`) keep the
  word, which is theirs.
- **The OS words** for a tile's life: a fault is a demand or a miss, the
  fault radius a reach, paging residency.
- **The floating origin**: the origin, its square (`origin_for`, the L4
  anchor), the lap and the rebase. The GPU's points are the eye subtracted
  in doubles since 2026-09-29, and a double holds the whole lattice with its
  fraction exactly, so the origin keeps nothing precise. What it still did
  was a translation between canonical points and a local frame of doubles;
  positions are kept in world doubles instead, X running on past a lap, and
  a canonical point enters them through one function that unwraps its X
  nearest the eye. Every draw already uploads its point less the eye
  (`pxl-render frame/draw.rs` `translation`) and the frame puts the eye at
  0, so the GPU sees nothing different. The clouds' lattice base, an
  integer wrapped with the lattice and a fraction under a lattice unit, is
  split from the eye's world position, and the globe is laid under the
  eye's XY at height 0.

## What stays

A tile's name (its quadkey), key and bytes, and the bake's directories
named by tile zoom: no published set changes. Making a chunk 64 cells of
its own zoom for objects too was the next step:
[a chunk goes past the leaves](./20261006-a-chunk-goes-past-the-leaves.md).
