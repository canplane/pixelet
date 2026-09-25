# Coarse tiers need a larger unit

The chunk is 64 cells on a side, and it is the unit of everything: one
composite, one scan, one node, one mesh, one draw call, one scheduler entry.
That is the right unit for the finest tier, where a chunk is a few thousand
runs and the work is worth a slot of its own. It is the wrong unit for the
coarse tiers, and at a wide zoom the wrong unit is now the whole cost.

## What the measurements say

Everything in [20260912-what-to-render-when.md](./20260912-what-to-render-when.md)
is in, and a wide zoom-out still looks like this (2026-09-12, 18:45):

- 1,079 chunks on screen. 725 of them at step 16, 222 at step 8.
- 474 MB live in the core pool against a 512 MB reservation, 128 MB of it
  spilled to the heap. The pool tracks the on-screen count almost exactly.
- Seven seconds to drain the queue after the zoom, against one scan slot,
  at ~3.1 ms per scan whatever the tier.
- 986 chunk draw calls for 457 k triangles of terrain.

A step-16 chunk emits sixteen boxes. It costs the same scan, the same node,
the same draw call and about the same pool block as a step-1 chunk that
emits thousands of faces. So at the tier where a chunk contributes least, it
costs the most per triangle, and the view that shows the most chunks pays it
1,079 times.

Two roads were tried or considered first, and both are closed:

- **Order.** Every ordering fix above was real, and none of them touches
  the drain time. Seven seconds of work is seven seconds whichever chunk
  goes first.
- **A strided scan.** A coarse cell is defined as the fold of every column
  under it — `begin_coarse` accumulates all of them and `layer_taps` averages
  four materials out of them — so scanning fewer columns is a different
  picture, not a cheaper one, and the chunk needs the full scan anyway the
  moment it refines.

## The rule

At step `s`, the unit is `s` chunks on a side. Step 1 through 4 keep the
chunk; step 8 works on a 2×2 block (128 cells), step 16 on a 4×4 block
(256 cells), and so on. The unit's cell count stays constant across tiers
— a 256-cell block at step 16 is 16×16 coarse cells, the same grid as a
64-cell chunk at step 4 — which is what makes the cost per unit flat.

This is the ordinary arrangement for chunked terrain LOD: a quadtree over
the world whose leaves are one geometry each, with the leaf size doubling
per level. Everything here already speaks that language. Pages are 16
chunks on a side; a step-16 block is a quarter page.

At the observed distribution: the 725 step-16 chunks become ~46 blocks.
Forty-six scans, forty-six draws, forty-six nodes, for the same 11,600
boxes. The 222 step-8 chunks become ~56. The on-screen node count goes from
1,079 to roughly 140, and the pool with it.

## What already bends, and what is bound to 64

The mesher is parametric now: both bake paths read `span` off the run set,
and the scratch grids (`MERGE_RUN`, `CO_START`) are sized for the largest
unit (`MAX_UNIT_CHUNKS`, 4 on a side). The
mesher also has a note worth reading in this light: at a chunk's own
boundary it cannot see the neighboring columns, so it emits every edge
face conservatively and never shades the seam. A 4×4 block puts fifteen
former boundaries _inside_ one scan, where the neighbor is known.

What is bound to 64 is the unit of work, in five places:

1. **The composite.** `composite_chunk_layers` paints one `ChunkKey` into
   one 64-wide volume, and that stays: a block is not composited whole. A
   4×4 block would be sixteen 64³ bands per Z band, 21 MB, for a tier that
   folds it all down to a few hundred boxes. The layers' `paint` is per chunk
   already; a block composites its chunks one chunk-row at a time.
2. **The scan.** _Done (step 1, 2026-09-12)._ The run set carries its
   `span`, and `begin_column_scan` takes a chunk-row of composites rather
   than one chunk, walking them together unit row by unit row. A chunk-row
   rather than a chunk, because the run set's prefix index is over the
   unit's columns in row-major order and a unit row crosses every chunk of
   the chunk-row: walking one chunk to completion before the next appends
   runs in an order the prefix cannot describe. A chunk-row of a 4×4 block is
   four composites, ~5 MB, alive at once. Span 64 with a chunk-row of one is
   exactly the previous path, and the existing tests say so.
3. **The node.** `ChunkNode` is one chunk key, one group, one layout. A
   block node covers `s×s` keys and subscribes to all of their source
   revisions — the store's column keys are per chunk and stay that way.
4. **The range and the page.** `in_range_chunks` enumerates 64-wide
   offsets inside a page; `PageChunks` owns one node per offset. A block
   is enumerated at its own stride, and a page owns blocks at every tier
   it currently shows.
5. **The tier decision.** Today each chunk decides its own tier from its own
   distance. A block decides once for its span, and the transition between
   tiers becomes a split or a merge: a step-16 block refining to step 8
   becomes four step-8 blocks, and back. The blend band applies to the block
   as a unit.

## The shape that keeps the chunk

Making the block the only node at every tier was the first idea and it is
the wrong one: eighteen sites in `ChunkNode` assume one chunk, and decor,
bounds and the object-store subscriptions would all have to generalize
sixteen ways for a change whose whole value is in two coarse tiers.

The chunk stays the node at every tier. What changes is _who bakes the
cubes_: at step 8 and 16 a chunk no longer scans and bakes its own, it is a
member of a block that does so once for all of its chunks. Everything else
a chunk does — decor, bounds, the frustum sphere, the scheduler entry, the
subscriptions — is untouched. Decor in particular stays per chunk at every
tier; it is a scatter contributed into one world-wide field, not a mesh,
and its density is already gated per step.

The block is one geometry, but it is **adopted chunk by chunk**. Every
vertex carries `local_pos`, its position in the unit, and a visibility
float the dither blend already selects per fragment. A chunk that adopts
its block sets visibility to 1 on the vertices in its own 64×64 region; a
chunk that has not yet adopted leaves them at 0 and keeps drawing its own
mesh. So the handoff rule from the record — keep drawing what you have
until the replacement is ready — holds per chunk, exactly as it does
today, and no four-way atomic swap exists. Refining works the same way in
reverse: the chunk keeps its region of the block visible until its own
fine mesh has built, then hides it.

The block itself is a small object: the unit's key, its run set, its baked
mesh per coarse step, a refcount of adopting chunks, and the scan-in-flight
state. It has no scene presence of its own; the chunks mount its mesh (once
— the first adopter mounts, the last to leave unmounts) and select their
regions of it.

## What it does not change

- The finest tiers. A chunk at step 1, 2 or 4 is the same node, the same
  scan and the same mesh as today.
- The store, the page as the streaming unit, the object layer's column keys.
- The scheduler's rules: the same four classes, the same aging, the same
  budget. A block is one more kind of entry with a larger span.
- The dither blend. A block blends as a whole, which is what a chunk did.

## Open

- **Split and merge timing.** A block that refines has to keep drawing
  until all four children have built, and four children that coarsen have to
  keep drawing until the parent has. The existing "commit keeps the layout
  until the new tier is ready" rule extends to this, but across nodes rather
  than within one; that is the part that needs care.
- **Which page owns a block that straddles pages.** At step 16 a block is a
  quarter page and never straddles; at step 32 it would be a whole page.
  The rule of keeping block size at or below a page keeps ownership simple
  and covers every tier the current range can reach.
- **The elevation seam between tiers.** A step-16 block next to a step-8
  block shows a step in the surface where their cells differ. The current
  per-chunk tiers have this already, one chunk at a time; larger units make
  the seams fewer and longer. The blend band exists for this and stays.

## Status 2026-09-13

Not taken. The scan cost this was for went away by another road — the
terrain layer writes coarse runs from the page heightmap directly, 10.6×
cheaper than the scan
([20260912-coarse-tiers-what-the-field-does.md](./20260912-coarse-tiers-what-the-field-does.md),
"Done") — so the chunk stays the unit at every tier. The block module and
the span-parametric scan are kept, unwired; the open items above are the
ones a wider unit would have to answer if draw calls ever make it worth
it.

## Retired 2026-09-13, evening

The unwired block was not free: the scan's and the mesher's scratch were
sized for a 4×4 unit (a megabyte of core memory), and the fine bake
cleared the whole merge plane on every open — 256 KB of memset for the
16 KB a chunk uses, two thousand times per zoom-out. `chunk_block.ts`,
its tests and the span-parametric scan are at
`17cb91c0`; the scan is one chunk again and the
scratch is sized to it. If draw calls ever make a wider unit worth it, it
is built on the direct path as the document above says, and the archived
module is the reference, not the base.
