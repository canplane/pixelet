# layer

A layer is where the world's stored data lives in the core, and the only place
that knows how it is stored. Everything above asks the world's query (`query/`),
and the query asks the terrain and the objects; no other code reads either
directly, and the compiler holds that: the query is a child of this module
(`layer.rs`, though its files stand beside the layers), and a layer's reads are
visible within `layer` alone. A layer never decides what to fetch either: the
view says what an observation needs, and a layer holds what it is asked for.

## The contract

A layer is a payload and a one-zoom fold: a tile at zoom z is folded from the
tiles at z + 1 under it, by the layer's own rule (`layer.rs` `Base`; the object
store has its own). The terrain and the object store each answer the query of a
node at a zoom the asker states, by their own methods (`terrain/query.rs`,
`object/store.rs`), and no other layer answers it:

- its **bound**: the node is empty, or lies inside a conservative box;
- its **answer**, which says what it lacked: exact, coarser along with the tile
  it needed, or nothing along with that tile (`answer.rs` `Answer`); the store
  is always resident, so it answers exactly;
- a **cursor** that carries a descent from a node to its children, so each step
  is answered from its parent and never again from the top.

A layer fetches and keeps nothing for a question, so reading costs what is
asked, not what a tile holds.

A tile owns what is its own, and what it owns goes when it goes. Its vectors are
its own; a fixed room is right only for what is known at compile time and mostly
in use, as a tile's elevation is. A packet is owned the same way: a layer decodes
it and drops it, or, as the earth does, holds it in the tile until the GPU has its
texels.

## The base

The base layers are baked, and a base tile never changes within a version. A
kind's lattice, zooms and packet bound are one row in pxl-spec (`bake/tile.rs`
`BaseKind::row`, `Tile`), which the client and the bake both read.

**Terrain** (`terrain/`) is the ground. A cell is one byte, its asset id, a
ground cube of Carto's palette or pixelet's own (`spec/spec/asset/cube.json5`),
beside an elevation; each cell's is worked out from those samples as a brick is filled,
never sent. A tile also holds its separation, the bridges and tunnels as lines
with a top and a width, and says which of its children are tiles (pxl-spec
`bake/tile.rs` `Occupancy`), so it can be read as soon as it lands.

**Places** (`place.rs`) hold what Carto shows at a tile's zoom, a place a cell
one icon wide, each with the zoom it shows from. **Divisions** (`division.rs`)
hold the finest administrative division each cell is in, from `DIVISION_LEAF`
up. The **earth** (`earth.rs`) is the planet's picture at one zoom only, held
for the floor's tiles (`earth::window`), each holding its packet until the GPU has
its texels and then which device generation does, and answered by a coarser
picture of the whole globe where none is held.

## The objects

What people place is one sparse octree (`object/store.rs`): each leaf an asset's
id at its node, each branch its eight children folded. A write splits leaves
only on its path, folds again up it and joins eight equal leaves into one, so
one picture has one tree. A leaf of air erases what a lower layer would show.
A leaf may carry an extension, such as a marker's words, kept in a pool for its
class of ids (pxl-spec `object/ext.rs` `class_of`) and placed and erased with
its leaf.

The objects come from the objects' source, as the files of kind `OBJECT`
(`object/repository.rs`). It keeps a file for each shard that holds objects, and
the top: what each of those shards folds to. The top is read first and its folds
stand in the tree where shards are not read; a shard the top lists is read as a
baked tile is, when the view asks for the ground at the shard's zoom, and its
leaves replace its folds; a shard the top does not list is empty without asking.
Writes go up as batches of edits, in the order made, a while after the first and
no more than a frame allows. The source applies them and rebuilds the top's rows;
the core writes neither a shard nor the top. The client's `OBJECT` backend folds
by the same rule (`ts/client/src/service/sys/fs/object_tree.ts`), and one golden
record holds the two equal (`spec/golden/object/top.tsv`).

## What is held

The view asks for demand: at each zoom, every tile within that zoom's band about
the eye, and the ground under the pivot and under a place the camera is sent to
(`domain/view/demand.rs` `observe`). It is never a function of what the rays
met. Each layer holds what is demanded in its own tile cache (`tile_cache.rs`),
which keeps four promises:

- Every observation has an answer: the coarsest zoom the declaration supplies
  whole is held all round the eye, the floor (`layer/floor.rs`).
- A tile is held hard for the floor or for demand while it is demanded, and as
  cache otherwise; only the hard set is correctness. The slots are counted at
  compile time and asserted to hold everything that can be held hard at once,
  and for a layer that reads the view one more to land in
  (`TileCache::SLOT_CNT`). A tile that arrives takes a
  free slot, or else the farthest one nothing holds hard and the view does not
  read.
- A region goes from unresolved to coarse to as fine as it is observed, and
  never back. The view is loading until every tile demanded shows, exact or
  coarser (`TileCache::is_shown`).
- Nothing is fetched on a guess: a view converges on its misses alone, and a
  camera sent somewhere waits for the ground there.

## The ground's clipmap

Terrain holds its cells in a clipmap (`terrain/clipmap.rs`; Tanner, Migdal and
Jones 1998): a level a zoom about a center, addressed modulo its size. A ray
observes a zoom's cells only within a fixed reach of the eye (pxl-engine
`clipmap.rs` `RADIUS`), so one level holds every cell the ray can observe at
that zoom. The eye has a level at every zoom drawn; the pivot has two, at the
leaves and the zoom over them; and a place the camera travels to has one.

A level is a few bricks of cells a side, each held in the entry its place names,
so the level follows its center a brick at a time and no held cell moves. A
brick at a zoom is held once, however many levels want it. A tile's folds are
cut into its bricks as it lands, and the bricks are filled after the frame, a
part at a time within the entry's budget (`fill_share`): their cells, a
min-max mipmap over them, and their ways as spans, a run in one pool all bricks
share (`terrain/pool.rs`), 64 MiB and no more, what the GPU reads and nothing else.
All of it is uploaded, so the CPU's query (`clip_read`) and the GPU's cast meet the
same bridge. A tile's lines are vectors of its own on the heap and go with it,
bounded as the tile is, by the slots and the bake's limit on a tile's bytes. A brick whose spans the pool cannot hold
reads the coarser level's meanwhile, and its fill is tried again once the pool gives
a run back; a heap that refuses a tile waits on the heap.
A brick with a column past `way::SPAN_CAP` stays refused while a level wants it.
Nothing of a tile is kept for a brick still to come: a brick that enters asks
for its tile again, and the browser's cache answers.
