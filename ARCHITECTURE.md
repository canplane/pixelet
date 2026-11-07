# Architecture — pixelet

pixelet draws the real Earth as cubes you can build on, in a browser tab or a
device app. What it draws and what is walked on follow from one function. A
pixel's ray, a walker's step, the pointer and an edit all ask the same question
of a **node**, a cube of space at a zoom: what fills it? One function answers,
and one descent asks it, compiled for the CPU and for the GPU alike, so the cell
a pixel shows and the cell a foot stands on are one answer (The query, the only
seam).

```text
  a pixel's ray      a walker's step      the pointer      an edit
   (the GPU)           (the CPU)          (the CPU)       (the CPU)
       └──────────────────┴──────────┬──────┴───────────────┘
                                     ▼
                      what fills this node, at the zoom
                      its distance from the eye asks?
                                     │
                  what players placed              ┐ each written
                  the ways off the ground          │ over the one
                  the ground's grid and its caves  ┘ below it
```

How finely the world is drawn is one rule too: a node is drawn at the zoom at
which it spans one angular cell of the camera grid, and what a zoom folds away
is not there, to draw, to walk on or to miss (The camera grid). Each kind of
thing is drawn wholly on one side: the GPU's rays find the world and the GPU
generates its decor, and everything else the kernel draws, it lists for the renderer
(Drawing).

pixelet is an application over [xpute](./xpute/ARCHITECTURE.md): a Rust kernel
that runs as xpute's core, the TypeScript around it, and Rust servers and
bakes that share the kernel's model. The memory, the rings and the turn are
xpute's and described there; this document is what pixelet builds on them.

The kernel is one order of dependency (`rust/client/kernel/src/`): an edge may
skip a level but never points up, and its folders are that order. The domain is
two heads, the scene and control, over one trunk, the view, the query and the
layers, and neither head names the other. A question goes down from a head
through the trunk to the reads that bring a tile in; its answer comes back up
the same way. The check-rules tool holds the order (pxl-tool
`bin/check_rules.rs`).

```text
  turn · door · shell · pointer   the top: wires everything
  ──────────────────────────────────────────────────────────────────────
  domain/                the world and the eye, two heads over one trunk

  asks                                                          answers
    │   scene/       a head: what this frame draws                  ▲
    │   control/     a head: moves the eye, asks the query          │
    │   smooth.rs    the smoothing both heads share                 │
    │   param.rs     the tuned numbers both heads share             │
    │   cmd.spec.rs  the shell's enums both heads may read          │
    │   preview.rs   an edit not yet made, control's to the scene's │
    │   view/        the trunk: the pose, the lens, demand          │
    │   query/       the trunk: the only seam                       │
    │   layer/       the trunk: the base and the object             │
  ──┼───────────────────────────────────────────────────────────────┼───
  sys/                   what all of it stands on                   │
    ▼   io/          reads, credits, the runtime's end           │
        sched/       a turn's edge and its phases
        mem/         the core memory's sections
        boot · load · stop   bringing it up, what it loads, its end
  machine.spec.rs        the machine it asks of its runtime (machine.json)
  unpack.rs              the device transport's zstd decoder

  beside   perf/: reads anything; no decision reads it
```

The parts follow that order. **The world** is what the kernel carries: sizes,
positions, layers and what fills a cell. **The eye** goes from the eye down to
the layers. **Drawing** is the last step up, from the eye's rays and the
kernel's list to a frame. **Below the view** is what all of it stands on.
**Making the data** is a chain of its own, run ahead of the view, that ends
where the view reads.

---

# The world

## Sizes and positions

Every size in the world is one number, its zoom. X goes once round the world in
2^24 units, and a thing the world over 2^z wide is at zoom z, so one zoom finer
is half as wide (`pxl-spec space/coord/zoom.rs`). A **cell** is the granule at a
zoom, possibly empty. A **quadrant** is a node of the plane at a zoom (`pxl-spec
space/layer/quadrant.rs` `Quadrant`), an **octant** one under a shard, through
the height as well (`octant.rs` `Octant`). A **tile** is a quadrant fetched and
stored as one, and a layer's tile has the same number of cells a side at every
zoom. The **pyramid**
is the tree of all tiles: a tile at z covers the 2 × 2 under it at z + 1. The
same zooms, cut through the height as well, make an octree of nodes, eight under
each, and that octree is what a ray descends (The query, the only seam). A
**shard** is the coarsest cube, a tile as wide as the world is tall with the
world's whole height over it (`pxl-engine lib.rs` `SHARD_ZOOM`); the world is
ingested, baked and kept a shard at a time. To a shard the octree is the plane's
quadtree, Z having no digit yet; the tree and a ray count Y and Z from the world's
edge (`overwrite.rs` `path`, `cast.rs` `grid_of`), so the world is one root and a
shard one cube, split at sea level under it.

A position is a **`Q3`**: three `i32`, one an axis, each counting 256ths of a
unit, so it is the same bits on every machine (`pxl-spec space/coord/q3.rs`).
X wraps once round the world. What a unit is worth on the Earth is Web
Mercator's, which only an adapter knows, and the planet around the plane is a
sphere, which Drawing covers. The zooms this program uses, the lattice, the
adapters and the pyramid's keys are
[rust/spec/src/space/README.md](./rust/spec/src/space/README.md)'s.

## The layers

The world is stored as **layers**. A layer is a payload and a one-zoom fold: a
tile at z is folded from the tiles at z + 1 under it, by the layer's own rule
(`pxl-kernel domain/layer.rs` `Base`, the object store's own). Which tiles are
fetched is the eye's, never a layer's.

The layers split on what changes. The **base** is baked from the sources, and a
base tile is fixed by its name within a version. The **object** layer is what
people author, and changes under its keys.

| Side   | Layer    | Stores                                                         | Shape                             | Folds by                                   |
| ------ | -------- | -------------------------------------------------------------- | --------------------------------- | ------------------------------------------ |
| base   | terrain  | the ground's palette index and height, and its separation      | a raster tile, 2.5D               | majority (palette index), average (height) |
| base   | place    | the places Carto shows at the tile's zoom, and each one's zoom | a raster tile, an icon a cell     | what shows from the parent's zoom          |
| base   | division | the finest division each cell is in, with its ancestors        | a raster tile, `DIVISION_LEAF` up | majority                                   |
| base   | earth    | the planet's picture                                           | a raster tile, top zoom only      | —                                          |
| object | object   | what players place                                             | a sparse octree a shard           | majority                                   |

**Terrain** is the ground. A tile holds its ground cells, their elevation, and
its **separation**: the ways at another grade, bridges and tunnels, each a line
with a top and a width (`pxl-spec bake/separation.rs`). It also says which of
its children are tiles (`pxl-spec bake/tile.rs` `Occupancy`), so it is read as
soon as it lands. A ground cell is one byte, a **palette index** into
OpenStreetMap Carto's palette, numbered in the order Carto paints and drawn as
land, water, transport or a structure (`spec/spec/asset/cube.json`); void and
the unknown past the shards baked give no surface. Heights are worked out on the
GPU from the elevation and read back, never sent (`pxl-engine ground.rs`
`height`).

**Objects** are placed, not baked: one sparse tree, each leaf an asset's id at
its node and each branch its eight folded (`pxl-kernel
domain/layer/object/store.rs`). A write splits leaves only on its path and joins
eight equal leaves into one, so one picture has one tree. A leaf of `AIR` erases
what a lower layer would show. A leaf may carry an **extension**, such as a
marker's words, kept in a pool for its **class** of ids (`pxl-spec
object/ext.rs` `class_of`), placed and erased with its leaf.

They come from the **objects' source**, below the `OBJECT` root, which keeps
them a file a shard, a shard with none having no file, and keeps the **top** of
the tree: what each node at the shard's zoom holding objects folds to
(`file.json` `object.TOP`; `pxl-kernel domain/layer/object/repository.rs`). The
top is read first, and its folds stand in the tree where their shards are not
read, so a view from afar draws from it alone. A shard the top lists is read as
a bake's tile is, the shards about the eye as the view asks the ground at the
shard's zoom (Reads, and the one request scheduler), and its leaves go into the one tree
in place of its folds; a shard it does not list is empty without asking, and a
shard read stays.

Writes go up as **edits**: each batch the writes `<key, id, ext>` the tree took,
in the order made (`object.EDITS`), sent a while after the first and no more
than a turn allows. The objects' source applies them to its shards and rebuilds
the top's rows of the shards they reach; the kernel writes no shard and no top.
It folds as the tree does, a rule the client's `OBJECT` backend holds again
(`ts/client/src/sys/fs/object_tree.ts`), held to the store by one
record both read (`rust/spec/golden/object/top.tsv`). Which source answers, the
client's own database or a server, the kernel cannot tell.

**Places** are tiles named like the ground tiles under them, each holding what
Carto draws there at that web map zoom (`pxl-spec bake/tile/place.rs`), by one
table taken from Carto's rules (`spec/spec/bake/place_theme.json`). A cell one
icon wide keeps one place, so each place carries the zoom it shows from, and a
parent is its children with what shows from its zoom on.

The **earth** layer is the planet's picture, baked at its top zoom only
(`pxl-spec bake/tile/earth.rs`).

## What fills a cell

What fills one cell is an **asset**, kept as its id. It has two independent
axes:

- Its **kind** is how it is drawn: a cube, a decor model, or a marker facing the
  viewer. An id is its kind over its shape (`spec/spec/asset/id.json`
  `SHAPE_BITS`).
- Its **source** is which side holds it: the base, whose ground generates it, or the
  object layer.

So decor is no layer. A generated tree and a placed tree are one kind from two
sources, arriving as one record (`pxl-kernel domain/layer/instance.rs`
`Instance`).

| Kind   | Base                                         | Object          |
| ------ | -------------------------------------------- | --------------- |
| cube   | the ground's palette entry, a cell's surface | a placed cube   |
| decor  | what the ground generates                    | a placed decor  |
| marker | a place, Carto's icon and its name           | a placed marker |

The ground generates its decor as it carves its caves, each a function of the
node and the one seed (`pxl-engine seed.rs` `node_seed`), evaluated where it
is asked from its root down, never stored and never handed down a descent:
a count at a root handed to the children by weights a hash draws, exactly,
within each one's room (`pxl-content share.rs` `halve`), the cumulus's
too. Decor's root is a column at the coarsest zoom a decor stands at, holding
as many as a hash draws about its mean, and a leaf cell holds one or none;
where it stands is a function of the cell, a seed and the ground's face, one
rule on both sides (`pxl-content scatter.rs`). The caves' roots each hold the
same tunnels, a node its piece of one and the ones not yet started
(`pxl-content cave.rs`). One
whose box meets a neighbor's that drew lower gives way, so two generated decors
never share a cell. A decor, placed or generated, stands as itself, a model or
its impostor, at every zoom whose cell its box keeps an eighth of or more, as a
folded cube of that box is drawn (`answer::eighths`), and is none where it keeps
none, so neither is told from the other.

A cell coarser than a leaf is what its zoom holds. The ground's coarse cell
stands at its folded height, its palette index the majority of those it folds,
and its top cell is filled to the share its height reaches (`pxl-engine
ground.rs` `top`). What is placed and the ways fold to the kind most of their
fill is (`overwrite.rs` `fold`).

A coarse cell keeps how much of its cube the ground and what is written over it
fill (`answer.rs` `cover`), in eighths, floored: its **fill eighths**. It is
drawn as its cube shrunk about its center by the cube root of that, so its
volume is its fill (`cast.rs` `fill_bounds`), and a ray stops where it enters
that box, the eye's, the sun's and the pointer's alike (`cast.rs` `entry`).

```text
   a cross-section of one coarse cell:      the box drawn in its place:
   eight cells, one filled (four shown)     half the side, one child cell

        ┌───┬───┐                              ┌───────┐
        │   │   │                              │ ┌───┐ │
        ├───┼───┤   fill = 1/8                 │ │▓▓▓│ │
        │▓▓▓│   │   ∛(1/8) = 1/2  ──────▶      │ └───┘ │
        └───┴───┘                              └───────┘
```

A folded cell is a summary: nothing in it is struck, hidden, cut by a preview or
pointed at (`pxl-kernel domain/control/edit.rs` `Pick`).

---

# The eye

## The camera

One **camera** looks, and every controller moves it
(`domain/control/camera.rs` `Camera`). It is an **eye** and the
direction it faces, the camera's **facing**, in spherical coordinates about the
world's Z: θ the polar angle from straight down, φ the azimuth from north toward
east. Nothing else keeps a copy.

The first-person camera turns about its eye; the orbit turns about a point a distance
ahead of the eye along the facing. The point a camera turns about is the
**pivot**, the eye itself in the first-person camera, derived and never kept
(`camera/controller.rs` `pivot`). The character stands there, and a change from one
camera to the other moves the camera about the pivot, never the pivot. Code past
the controllers reads the eye, the facing and the pivot.

The **pose** is the eye and the turn from the world's axes to the camera's
(`domain/view/frame/pose.rs`). The world stays in its own axes, in doubles; what
turns is the observer. Every draw uploads its point less the eye, subtracted in
doubles before it is narrowed (`pxl-render frame/draw.rs` `translation`).

## The camera grid

How finely each part of the view is drawn is set by the **camera grid**: angular
cells about the eye, each the least the observer promises to tell apart, the
same angle wherever it lies (`pxl-spec space/camera/grid.rs`). An angular cell
is 1/`CELLS_A_RAD` radians on every view (`CELL`), so what is observed at a
distance is the same on every screen; a display only draws it.

The cell sets what is observed, the beam rays and the cells their starts are
found for, a cell apart (`pxl-engine beam.rs` `Grid::of`), and the samples'
lattice, `SAMPLE` a cell a side.

That sets the rule of detail. A node is drawn at the zoom at which it spans one
to two angular cells at its distance (`cell_zoom_at`): each zoom is drawn over a
band of distance, and the bands double going out, as an angular cell widens with
distance.

```text
  bands of distance from the eye; s is a zoom-z node's side, D the distance
  at which such a node spans one angular cell

eye ●───────────┬───────────────────┬─────────────────────────────────────┬──▶
               D/2                  D                                    2·D
                │◀──── zoom z ─────▶│◀─────────── zoom z − 1 ────────────▶│ …
  an angular   s/2                  s                                    2·s
  cell spans
  its zoom's      2 cells … 1 cell    2 cells … 1 cell
  node spans
```

Distance counts every axis alike, height included, and the ground's tile zoom
for a distance follows from the same bands (`pxl-spec space/layer/lattice.rs`
`z_at`).

## The query, the only seam

The query is the only place a layer's shape is known. Terrain and the object
store answer one question of a node, a tile or a cube, at a cell zoom the asker
states (`pxl-kernel domain/layer/query.rs` `Query`); places, divisions and the
Earth answer none.

- Its **bound** is empty or inside a conservative box. What is not held is not a
  bound but the answer's (`Query::bound`).
- Each **answer** says what it lacked: exact, coarser with the tile it needed,
  or nothing and that tile (`domain/layer/answer.rs` `Answer`).
- A descent carries a **cursor** from a node to its children, so each step is
  answered from its parent and never again from the top.

The query fetches and keeps nothing, so reading costs what is asked, not what a
tile holds.

The **world** composes the layers into one answer, for drawing and for control
alike (`pxl-engine world.rs` `node`, which `pxl-kernel domain/query/world.rs`
`World` answers on the CPU):

1. the ground's grid, each cell a cube shifted up to its height;
2. the caves' air under its face, Minecraft 1.18's noise caves in integers off
   the one seed, asked only of the cells and nodes a reader asks for
   (`pxl-content cave.rs`, asked through pxl-engine `content::Content`);
3. the ways off the ground, where each crossing of a line over the column is a
   deck and its pier, a bore, an embankment or a cutting (`pxl-engine
way.rs`);
4. then what is placed, air included.

A ray descends the pyramid, by one function the CPU and the GPU compile alike
(`pxl-engine cast.rs` `first`). A node coarser than the zoom its distance
asks is not what is drawn there, so it is asked only whether anything under it
fills at the zooms the ray asks; what those zooms fold away is not there to
miss. Each part answers that from what it already keeps, a bound, a hash node or
nothing held under the node (`pxl-kernel domain/query/node.rs` `World::node`).

```text
a node coarser than the zoom its distance asks
   +-- nothing under it fills ---------> passed whole
   +-- something may, or not held -----> split; children asked in
                                         the order the ray meets
                                         them, each handed what its
                                         parent found
a cell at the zoom its distance asks
   +-- fills --------------------------> met: its pixels show it
   +-- not known ----------------------> the ray stops there
```

A descent leaves its **line**: the node it reached and every node over it, each
with what was handed it (`pxl-engine support.rs` `Line`). A reader that asks
about the cells beside one it reached asks them off that line, from the deepest
node holding each (`support.rs` `asset`), never again from the top.

Control asks the same world, one ray at a time, at the zoom the eye's rays draw
there, so what control meets is what is drawn (`pxl-kernel domain/query/ray.rs`
`first_hit`). A walker stands on the first cell that fills down its cell's
columns, at the finest zoom placed there (`domain/query/support.rs`), and the
pointer's ray strikes decor as the box it claims where the rays draw decor as it
is (`domain/query/decor.rs`). None reads a layer.

Only the query reads a layer. The layers' reads are the form both answer
(`Query`), the terrain's and the store's past it (`TerrainQuery`,
`ObjectQuery`), the clip levels as the CPU reads them (`CpuClipmap`) and the
ways' pool. Rust lets them out of `domain/` alone; inside it they are named by
the layers and `domain/query/`, which re-exports none of them, and check-rules
holds that (pxl-tool `bin/check_rules.rs` `LAYER_READERS`). What the query
answers in public is the composed world: an answer of one layer alone, such as
the ground's surface a walker starts from (`domain/query/surface.rs`), stays
inside it. A reader that needs what a layer holds asks the world for it.

What the bucket serves is said in its **declaration**: the coarsest zoom
supplied whole, each kind's format and version, and the shards served
(`pxl-geo-bake source.rs` `declaration`; Publishing a version). The world ends
in bedrock at the edge of the **service area**, those shards: a walker and an
edit stay inside it, and an edit goes only over known ground, in shards whose
objects are read (`domain/query/limit.rs`).

## What the view holds

The view asks storage for **demand**: what an observation requires, before it is
any tile. At each zoom from the declaration's top zoom to the leaves, it is
every tile within that zoom's band about the eye (`domain/view/observation.rs`
`ByEye`), mapped to the layer's tiles (`domain/view/demand.rs` `observe`). It is
a function of the eye alone, never of what the rays met. A tile it names that is
not held is a miss. Every layer is asked this way and holds what it is asked for
in its own **tile cache** (`domain/layer/tile_cache.rs`):

- Every observation has an answer: the coarsest zoom the declaration supplies
  whole is held all around the eye, the **floor**
  (`domain/layer/tile_cache/floor.rs`).
- A tile is held hard for the floor or for demand while it is demanded, or else
  as cache; only the hard set is correctness. The tile cache's **slots**, which
  tiles are held in, are counted at compile time and asserted to hold all that
  can be held hard at once and one slot more to land in (`TileCache::SLOTS`). A
  tile that arrives takes a free one, or else the farthest nothing holds hard
  and the view does not read.
- A region goes from unresolved to coarse to as fine as it is observed, and
  never back. The view is loading until something has reached the screen and
  every tile demanded shows, exact or coarser (`TileCache::is_shown`).
- Nothing is prefetched: a view converges to exact on its misses alone.

The GPU holds the ground's cells in a **clipmap** (Tanner, Migdal and Jones
1998): a **clipmap level** a zoom about a center, addressed modulo its size. A
ray observes a zoom's cells only within a fixed reach of the eye
(`pxl-engine clipmap.rs` `RADIUS`), so a clipmap level about a center holds
every cell it can observe at its zoom. The eye has one at every zoom drawn; the
pivot two, at the leaves and the zoom over them; and a place the camera is
traveling to one, at the leaves (`domain/layer/terrain/clipmap.rs`):

- A clipmap level is a few **bricks** of cells a side, each held in the entry
  of the level its place names, so the level follows its center a brick at a
  time and no held cell moves. A brick at a zoom is held once however many
  clipmap levels want it, and kept while any does.
- A brick is filled on the GPU as its tile lands (`pxl-render
gpu/ray/ground.rs` `grids`): its cells, a min-max mipmap over them, its
  **levels**, and its ways as a tree in one pool every brick shares
  (pxl-engine `way::tree`). All three are read back into the kernel, so the
  CPU's query and the GPU's cast meet the same bridge or tunnel.
- A brick whose tree the pool cannot hold at its cap stays absent while a
  clipmap level wants it. Where a brick is not present, a reader reads the
  coarser level's (pxl-engine `clipmap::step`).
- Nothing of a tile is kept for a brick still to come: a brick that enters
  asks its tile again, and the browser's cache answers.

```text
  one axis of a clipmap level, ACROSS bricks a side (drawn as 8); brick x is
  held in entry x mod ACROSS

  brick x     12   13   14   15   16   17   18   19
            ┌────┬────┬────┬────┬────┬────┬────┬────┐
  entry     │ 4  │ 5  │ 6  │ 7  │ 0  │ 1  │ 2  │ 3  │
            └────┴────┴────┴────┴────┴────┴────┴────┘
  the centre moves a brick east: 20 enters and takes entry 4, which 12
  leaves; 13 to 19 stay where they are

  brick x          13   14   15   16   17   18   19   20
                 ┌────┬────┬────┬────┬────┬────┬────┬────┐
  entry          │ 5  │ 6  │ 7  │ 0  │ 1  │ 2  │ 3  │ 4  │
                 └────┴────┴────┴────┴────┴────┴────┴────┘
```

## What stands over the bricks

Decor and the markers stand beside the ground, and both are gathered over the
eye's clipmap levels' bricks.

Decor is generated on the GPU from the same clipmap levels the rays read
(`pxl-render gpu/generator.rs`). The leaf cells under a clipmap level's brick
are read off the hash a level's cell a thread, each candidate that stands at
the level and stands clear is generated on a thread of its own. A brick's decor
is thinned and generated whole only where its clipmap level holds another brick
there; elsewhere its candidates, its hash's alone, stand, and a record is
generated again only where what its own candidate reads changed: an edit, a
cut, or a brick filled at its level's zoom or finer within reach of it. Every
frame each generated record is taken where the rays ask the zoom it was
generated at, held to the finest level holding its column (the coarser level's
ground drawn where a finer one holds no brick, over open sea), and compacted
into the instances, a model down to its own size on screen, an impostor below
it and culled below that. Placed decor is handed in
as records the GPU draws beside it, taken where the rays ask the coarsest zoom
it stands at or finer (`domain/scene/decor/gpu.rs`,
`pxl-content generation.rs` `drawn_at`).

A **marker** is drawn flat to the camera: a place, a pin someone put up, or the
chat's line. Each kind reads its own, and one arrangement draws them together
(`domain/scene/marker/arrangement.rs`). The places and the pins are read over
the bricks, taken as one partition of the ground at the zooms the eye asks
(`domain/view/observation.rs` `bricks`), each standing at its cell's center on
the surface drawn there. A place shows what Carto shows at the zoom its spot is
drawn at (`domain/scene/marker/place.rs` `point_of`), so where nothing is drawn
there is no place. The chat's line hangs over its speaker's head
(`domain/scene/character/companion.rs` `line_hangs`).

## Each concern's bound

Taken together, the view is six concerns between the world and the screen, each
with its own bound:

| Concern    | Asks                                          | Bounded by                                                                            | Where                                                     |
| ---------- | --------------------------------------------- | ------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| Storage    | a layer's records, in the layer's own shape   | nothing of the view                                                                   | a layer; the bucket                                       |
| Query      | what a node holds, a point, or first on a ray | —; it holds nothing                                                                   | `domain/query.rs`                                         |
| Drawing    | what each pixel's ray meets                   | the screen's rays                                                                     | `domain/scene/cast.rs`, `pxl-render gpu/ray/cast.rs`      |
| Demand     | which records an observation requires         | a zoom's reach about the eye (`demand_tiles`)                                         | `pxl-spec space/layer/demand.rs`, `domain/view/demand.rs` |
| Quadrant cache | which records are held, and why               | its slots, asserted at compile time                                                   | `domain/layer/tile_cache.rs`                              |
| Scheduling | which waiting work runs now                   | a turn's time and the fetches let out at once ([Reads](#reads-and-the-one-request-scheduler)) | `sys/io/request.rs`, `sys/io/policy.rs`                   |

Every number a person would tune is named once, with its reason, in a `param.rs`
beside the lowest part that reads it (`pxl-kernel domain/view/param.rs`,
`sys/io/param.rs`). None is a correctness bound: set to its least, the view
still converges, only coarser or later.

---

# Drawing

This part turns the eye's rays and the kernel's list into a frame.

## A ray a pixel

Each drawn pixel is one ray on the GPU, walking the query's descent over the
ground's clipmap levels, the caves, the ways and the objects' tree as the GPU
holds them. What a ray stops on is the query's answer; the rasterizer only
orders what the rays found.

Neighboring rays share most of their descent, so the cast walks that part once
an angular cell and hands the rest to the rasterizer:

```text
  one angular cell of the camera grid
  ●─────────────●   beam rays at its corners walk the descent its
  │ ·  ·  ·  ·  │   pixels share (beam_cs); the cell starts at the
  │ ·  ·  ·  ·  │   least of them (cell_cs)
  │ ·  ·  ·  ·  │
  │ ·  ·  ·  ·  │   samples, SAMPLE an angular cell a side, walk on from
  ●─────────────●   there, and the node each stops on is drawn as a box
                    (sample_cs, sample_box_cs)

  each pixel ──▶ the nearest box its own ray enters (raster.wgsl)
                   ├── a node a sample answered ──▶ takes it
                   ├── none, and no sample met ───▶ the sky
                   │   anything
                   └── else ──────────────────────▶ listed, walked on
                                                    the holes' queue (queue.wgsl)
```

The entries are pxl-render's WGSL (`beam.wgsl`, `sample.wgsl`, `raster.wgsl`,
`cast.wgsl` `cast_cs`, the listed pixels' walks `queue_walk.wgsl`, `queue_ask.wgsl`,
`queue_last.wgsl`). Once
a frame the kernel hands them the eye and the camera grid's layout
(`domain/view/lens.rs` `gpu_cast`), and the cast draws at the display's pixels
times a scale, which the `resolution` setting picks
(`domain/scene/resolution.rs`).

The scale buys back only the work done a drawn pixel at a time: the pixels' cast
and misses, the boxes drawn into the visibility buffer, the world's passes, its
occlusion, gathering and marks, and the light's tiles marked where the eye's
rays meet the world. The beam rays, cells, samples, their boxes and faces run on
the camera grid's angular cells and the rest of the light's passes on its map's
texels, which it does not shrink; the finish and the markers draw at the
display's pixels at any scale, their text sharp. Each GPU span says which it is
(span.json `per_px`), and a span whose passes are of two kinds is split. The
scale does not reach the rest, so a second knob gives every pass more time: the
rate, the interval between turns, a refresh a step. `auto` is a budget
controller, as congestion control is, over both: the frame's budget is the
interval turns are paced at, no tighter than 1/60 s; while the frame's GPU time
runs over it a knob steps down, the rate to 30 Hz first (`RATE_MIN_HZ`), then
the scale to its least, and a knob steps up, the scale first and the rate last,
where the step above, the rest of the GPU's time as it is and the per-pixel
spans' grown by their pixels, would fit its budget; one step a settling window.
The rate is the runtime's: the kernel asks it (`TURN_PACE`) and reads it back off
each turn's grant. Without timestamps it reads the frames' gaps, which cannot
foresee room, so only the scale moves, and a step up is a probe whose failure
doubles the wait before the next.

What the rays asked of each node is kept between frames in the **node
cache** (`pxl-engine node_cache.rs`), a table keyed by the node, and
forgotten only where what a node read changed (pxl-render `forget.wgsl`
`forget_cs`).

A ray leaves four words (`pxl-engine ray.rs`): the asset it met, with its
face, zoom and fill eighths; how far; the palette indices a coarse cell folds;
and which of the face's edges and corners meet the air or a rising cell. The
**screen pass** paints them with the one function every lit face uses
(`frame/shader/screen_ray.wgsl` `pixel_ray`, `frame/shader/paint.wgsl` `paint`):
its swatch or image, shaded edges, light, and the air before it.

The effects on a drawn surface, its lines, its corners' and the screen's
occlusion and the shadows, reach one distance from the eye, where the camera
grid's cells one zoom coarser than the ground's leaf begin, and fade alike
before it (`pxl-kernel domain/scene/stage.rs` `STAGE_AT::EFFECT_REACH`,
`frame/shader/effect.wgsl` `effect_share`). The rim and the air, which read
only the face's normal and distance, reach as far as the eye draws.

## The shadow map

Shadows are read off the shadow map, the world's depth from the sun in a square
box of the light about the eye (`gpu/shadow.rs`), each texel fixed in the world
and held in a map slot by its index modulo the map's side (`pxl-engine
light.rs` `Light`). The sun is an observer as the eye is: one set of passes,
written once over a part each observer gives (pxl-render `sample.eye.wgsl`,
`sample.light.wgsl` and the like), asks the world, each tile of the box at one zoom, its band: the
camera grid's cell zoom at the tile column's nearest point to the eye. This
contract was changed on purpose. The light asked at the eye's zooms, so a
shadow had the detail of what it falls on, and a tile was cast again as the eye
moved; a band is the tile's place in the box alone, so a tile holds while the
eye moves, and where the sun is low or the eye high the light asks two or
three zooms finer than the eye draws there.

It casts only what a receiver reads:

```text
  the world, each voxel's box of the eye  ┐
    and each pixel it walked apart        │
  decor, where it is placed               ├─▶ lists the tiles it stands on,
  a character, the beacon, where drawn    ┘   within the effects' reach,
                                              and its footprint
                                              (record_draw_cs, light_mark_walked_cs,
                                               light_mark_draws_cs)
                                                   │
                                                   ▼
            a tile is read if it lies sunward of one stood on,
            within the tallest decor's height (light_propagate_cs)
                                                   │
                                                   ▼
            the tiles read anew, since what a tile holds last moved,
            are cast; the rest stand; the map is laid in the tiles read
```

Every pass past the receivers' own is sized by what they list, so where nothing
within the effects' reach is seen, as with the eye high over the ground, the
light runs no workgroup past them and lays no tile.

Every walk of the light asks one zoom, the band of the tile it lies on: the
zoom the eye's rays ask as near as the tile's column comes to the eye, from the
box's middle, held to the eye's zooms (`pxl-engine light.rs` `Light::band`),
a slot's, a sample's and a beam ray's alike. A ray takes another's walk only
where that one asked as it does: the cell a sample stopped at, the sky past the
samples about it, and its cell's start and line, where every beam ray of the
cell's window did; else it walks on its own, from the world's root where it
was the window (`pxl-engine cast.rs` `Detail::alike`; pxl-render
`light.wgsl` `visible_entry`, `sky`, `cell.wgsl`, `light_cell.wgsl`). A band is the tile's place in the box
alone, so what a tile holds moves with the light, the eye's zooms and the world
under it, and no walk reads where the eye stands: the eye and the light run the
same entries (`beam_cs`, `sample_cs`) over one walk. The eye moving keeps every
tile: the box follows the eye a tile at a time, and what the cast read moves
with it; a tile the box slid onto, or slid into another band, is read anew, the
rest stand (`light_slide_cs`, `light.rs` `Light::keeps`).

Decor and the characters, which move, are drawn into the map each frame, their
depth only. A receiver compares nine taps, each moved by its own plane's slope,
and casters are pushed by the same formula, so a plane does not shadow itself
(`shadow.wgsl` `shadow_visibility`). Beside the shadows, the lighting settings
outline an edge that meets the air, lift a rim, and shade corners or run N8AO
(`spec/client/kernel/domain/view/setting/schema.json`, section `lighting`).

## The frame

Each turn ends with a **draw list** (`pxl-render frame/draw.rs`): entries tagged
by the pipeline that draws them, each with its mesh and uniform, for what the
kernel decides to draw beside the world — the cumulus clouds, the characters,
the location indicator that shows where the device is, an edit's ghost box, the
markers and the debug lines. A character's draw also records where it stands
(`DrawList::stand`), for the shadow map. The rays and the generated decor are
not in it, nor a preview's character: a preview of the character picker draws
the companion's person or pet by an eye of its own, its draw handed to it
apart from the list, so what the world's eye sees of the character moves
nothing there (`gpu/pass/preview.rs`).

The kernel stages the frame (`domain/scene/stage.rs`) and calls the renderer
(`gpu/renderer.rs` `render_frame`), which draws every open surface, the world
and the character picker's previews, and reports what the frame cost and
whether it wants another turn. Its passes are modules by what they draw
(`gpu/ray/`, `gpu/pass/`):

```text
ahead       impostor atlases, fresh bricks filled, the atmosphere's
            tables, the air's volume
cast        the screen's rays; decor generated; the shadow map
world pass  generated decor → rays at their depth → generated impostors,
            each toned into the world's color, linear → the occlusion's
            half-resolution map brought to each pixel and multiplied into
            the color, as the claimed tiles' quads →
            sky and stars at the far plane → listed characters, cumulus,
            impostors → high cloud planes → location indicator, ghosts
occlusion   N8AO found at half resolution off the depth the world pass laid
            before its map, where what it reads moved: the pass ends there
            and begins again; held where not, the pass drawn as one
            (`gpu/ao.rs`)
finish      the world's color encoded onto the canvas once
over        markers → debug lines → previews
```

The calls are WebGPU's, written through xpute's encoder (`gpu/api.rs`) into one
stream, which xpute's GPU runtime runs: the page's on WebGPU after the turn returns
(`@xpute/gpu` `WebGpu`, `ts/client/src/service/gpu.ts`), the device app's on
wgpu around each turn (`xpute-gpu/runtime/native.rs`). A frame where nothing it
reads moved submits nothing, and the canvas keeps its frame.

The renderer is made before its first frame, in three steps
(`gpu/renderer.rs` `render_frame`, `gpu/pipeline.rs` `Pipelines`), so that
making it never holds the GPU process's main thread, which every frame the
browser presents waits on: every shader's module, a few a turn, within
`stream::MODULE_BYTES` and the stream's room; then the renderer, each of its
pipelines made by WebGPU's async call, which compiles off that thread, its
completion a ticket's word (xpute `RENDER_PIPELINE_ASYNC`,
`COMPUTE_PIPELINE_ASYNC`); then no frame until every word says it is made. A
frame turn before then draws nothing and asks for the next, and the kernel's
snapshot carries how much is made (`renderer_progress`), which the bar over the
world shows until the view is first drawn (`shell/cover.ts`). Every pipeline it draws with
is asked for in the second step, the cast's and the ground's included; one
asked for once it draws is a bug (`EBUSY`). The device app's wgpu has no async
call, and its GPU runtime answers the same op at once by the same ticket.

A frame where only the draws the kernel bounds moved is drawn again in the
screen's **tiles** alone (`gpu/tiles.rs` `Tiles`), by the shape the light's cast
casts by: what moved marks the tiles it covered and covers; the light's map is
drawn again in the tiles its casters that moved mark, and the change is
propagated on the GPU to the screen's tiles whose pixels read one of those
(`gpu/propagate.rs`); a tile stays claimed until its history has gathered the
jitter's frames (`gpu/resolve.rs`). The CPU decides from what it handed whether
a frame is drawn whole, through the tiles, or not at all, as the light decides
whether to cast: the propagation runs only in a frame whose casters mark the
screen's tiles drawn, which the CPU counts as marked, so nothing it marks is
read back.

The shaders are WGSL by hand, assembled from parts (`frame/shader/part.rs`);
the layouts and numbers they read are generated from pxl-engine by the
renderer's build. The compute entries over the world (the cast and its
node cache, the light's, the fill's, the ways' and decor's) are each held word
for word to pxl-engine's Rust by a test over a real world's buffers
(`rust/client/render/tests/`). Every walk, the engine's and each by hand,
ends at `cast::STEPS_MOST` steps (`pxl-engine cast.rs` `walk_at_most`). What
a rule both sides decide divides by, or takes the square root of, the frame
holds: the renderer rounds it once on the CPU and hands it in a word, and
both multiply by it (`pxl-engine light.rs` `W_WIDTH` to `W_BAND_SCALE`,
`beam.rs` `GRID_S_INVERSE`, `GRID_SLOPE`). The light ray's start is fixed
point, summed in integers (`light.rs` `Starts`, `light_aim.wgsl`
`texel_start`); the eye's ray at each pixel and a beam's width, which only a
GPU walks, the GPU works out alone, each in one function (`walk_node.wgsl`
`eye_aim`, `spacing_width`). The screen's tiles' passes (`screen_*`,
`box_args_cs`), the air's (`atmos_*`), the clouds and the location indicator
are held by none.

The build validates every shader with naga and checks each pipeline's groups,
compute and render, against the bindings its shaders read, both ways: each
binding an entry point reads is laid out, of a kind its declaration takes, and
each entry laid out is read by a pipeline that lays out its group (build.rs
`BINDS`, `frame/shader/record.rs` `lays_out`). A layout keeps the group it
was made of, and a bind group is made over it alone: entries other than the
group's bindings, each once and of its kind, stop the call (`gpu/api.rs`
`Layout`, `Gpu::bind_group`). No struct of a shader holds a struct, in a
draw's pass as in the compute entries (build.rs `no_nested_struct`). A test
compiles each shader with Tint, the compiler a browser runs, and makes every
pipeline the renderer makes over it, each by its entry points
(`ts/tool/shader/shader.test.ts`).

## Past the pyramid: the sky and the sphere

The pyramid is planar and leaves the poles out, but the sky and the ground past
the terrain are drawn too. The earth's picture is a layer: reflectance with city
lights in alpha (`pxl-spec bake/tile/earth.rs`), held only for the floor's tiles
(`domain/layer/earth.rs` `earth_window`), and answered by a coarser global
picture where none is held. Nothing waits for that picture: until it comes,
the land past the terrain shows the air before it alone. The sun, moon and
stars, the scattering and the clouds are no layer: each is evaluated per pixel
from a few parameters.

The sky is drawn at the far plane, so only pixels no ray met run it
(`frame/shader/sky.wgsl`). Where such a ray falls to the land, it shows flat
land shaded from the earth layer and lit as the terrain is (`atmos_air.wgsl`
`beyond`). The air between the eye and what is drawn is Hillaire's
aerial-perspective volume (`gpu/atmosphere.rs` `froxel`), read by each drawn
point along its direction from the eye and at its depth (`atmos_air.wgsl`
`air_before`). Each light has its share of it, laid as its sky view is: about
the world's up and the light's azimuth, in Hillaire's directions, finer
toward the light and the horizon, and in km deep by one law for every
direction. Neither reads the view, so a turn of the eye keeps both, and both
hold the light per unit of it, unexposed, so the exposure moving keeps both;
a reader shows each as its light's sky is shown. A light's view and its air
are drawn and read only where the light reaches the air, its sky showing
half a display level along its vertical at the exposure, which the exposure
pass measures; the sun and the moon by the one rule. They are drawn again
together where the light's height has moved half its step, the height over
which its sky moves a display level, or the eye's altitude half its own step,
and laid with words a reader takes as they are: the horizon and the depth
law where they were drawn. The exposure is drawn where the words it reads
move, or the ground under the eye half its step. Every step comes from a
slope its table measures as it is drawn, each measured again apart, only
where its own quantity has left the sample it was taken over.

The **clouds** are three layers on one wind (`pxl-content cloud.rs`): the
cumulus, models drawn like decor but claiming no cell
(`domain/scene/environment/cloud.rs`), and above it altocumulus and cirrus,
planes read where a ray meets them (`frame/shader/cloud.wgsl` `sheet`).

## Markers and the atlas

Markers are one instanced quad over one texture packed shelf by shelf, the
**atlas** (`pxl-render gpu/atlas/shelf.rs`, `frame/shader/marker.wgsl`). A
**sprite**, a design as the atlas holds it, is found by what it paints, never
by who shows it, so equal designs share one place on the atlas
(`domain/scene/marker/design.rs` `Design`). It is drawn on the GPU only when
what it shows changes: its shapes as signed distances, its text as lines the
platform drew, so shaping, bidirectional text and emoji are each platform's own.

---

# Below the view

This part is what the view stands on, `sys/`, and the top that wires the kernel
to its runtime: how what it asks for arrives, what it costs in time and memory, and
how a failure leaves what is drawn alone. Nothing in `sys/` names anything above
it: a layer asks io for a tile, and io never asks what the tile is for.

## The kernel and its runtime

The kernel is a unikernel on a virtual machine: it holds the world and
decides everything about it, but owns no device. `pxl-kernel` and
`xpute-core`, its library OS (its scheduler, door, memory and stop), are one
image, a core of xpute. Its **runtime** is its VMM: it implements the devices
the kernel's machine asks for and gives it the rest, one memory, granted once,
and a share of time on each call, a **turn**
([xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md)). The kernel owns the
abstractions and the policy over those grants.

Around the kernel pixelet has three parts, by xpute's picture of a core's
world:

- the **shell**, the screens and components, the client outside the VM (a
  unikernel has no userspace): React over the kernel's snapshot
  (`ts/client/src/shell/`);
- the **services**, what answers the kernel's app ABI and what each platform
  gives — the surfaces, the position, storage, files, text and the server
  (`ts/client/src/service/`, `port/`, `sys/`, `origin/`);
- the **runtime**, xpute's, the kernel's VMM, booted once by the client
  (`ts/client/src/runtime/boot.ts`) and kept for the rest of it
  (`ts/client/src/runtime/`).

The kernel runs apart from the shell, so a turn holds none of the shell's
events, and the runtime sits where the kernel's memory is
(`ts/client/src/runtime/open.ts`, `open.native.ts`; both start it as
`runtime/start.ts` does). A device runs the kernel on its own thread and the
runtime on the shell's JavaScript thread. A page runs the kernel and the
runtime in a worker (`runtime/worker.ts`) with the files, the network, the
text and the GPU on the canvas the shell lends it, and the shell on the main
thread: the shell's commands and the snapshot cross xpute's remote as frames,
and the rest, where the boot stands, the GPU's and the network's states, the
kept settings, a canvas lent and the profiler's readings, as
`runtime/page.ts` says.

The kernel's crate is its artifact, in xpute's core form for its target
(`pxl-kernel lib.rs`, one `xpute_core::core!` over its machine,
`rust/client/kernel/machine.json`): the kernel implements xpute's entry
(`door.rs` `run`), and the form hands it what it hands any core
(`xpute_core::entry` `BootInfo`: where a form sharing its process says what
stopped it, `stop_hook`, and its GPU runtime's answer, `admit`). On wasm32 it is
the WebAssembly module the page runs (`ts/client/src/service/kernel.ts`);
anywhere else it is the library linked into the device app under xpute's native
runtime (`xpute_gpu::native_core!` over `xpute_runtime::native_core!`), which runs
each call on the kernel's own thread, since a turn that draws through wgpu
outgrows React Native's JavaScript stack, runs the GPU's runtime around each turn,
and exports it to the app through xpute's native module (`@xpute/native`). What
the app calls besides xpute's names is the transport's decoder (`unpack.rs`,
`rust/client/kernel/include/pxl_native.h`). Outside either form the kernel runs
under `cargo test`.

The kernel has two ABIs, xpute's two, on one number space. **xpute's predefined
ABI**, majors under 0x10, is the runtime's, the kernel's devices: the kernel calls
the file device for its files and posts its report, during a turn and taken
after it returns, so each is done before the next turn begins
(`sys/io/runtime.rs`), and the runtime raises the kernel's interrupts: its life
(`SYS_ATTACH`, `SYS_SUSPEND`, `SYS_RESUME`, `door/life.rs`) and a GPU answered
(`GPU_READY`), and completes its opens and closes (`FS_OPENAT`, `FS_CLOSE`). **The app's ABI** is what the
app defines, two protocols: the shell's, `rust/client/kernel/shell.json`, its
intents and what the platform's services give, the surfaces, the sensors, the
data and each turn's grant; and the text device's, `text.json` beside it, the
platform's text, which the runtime answers beside the core. Both sides' readers,
writers and bindings are generated from each (`door/cmd.spec.rs`, the enums the
domain reads apart in `domain/cmd.spec.rs`, and `ts/client/src/abi/cmd.spec.ts`;
`sys/io/text.spec.rs`, `ts/client/src/abi/text.spec.ts`), each packet an XTP tree, xpute's relocatable
binary tree. The shell's exports are what the shell and the services submit; its
imports are what the kernel posts unasked, the snapshot, the view's azimuth and
scale apart from it (`SHELL_VIEW`) and the sensors it starts and stops. The text
device's are the calls for a text's lines and its pixels (`TEXT_LAYOUT`,
`TEXT_RASTER`), answered before the call returns (`sys/io/text.rs`,
`ts/client/src/runtime/start.ts`, `port/text.ts`): a page's runtime through the
module's import, a device's on the JavaScript thread that waits on the turn. A
command's major is the module it is for (`INPUT`, `SENSOR`, `FS`, `SURFACE`,
`SETTING`, `SCENE`, `CAMERA`, `PERF`, `EDIT`, `SHELL`, `TEXT`, `TURN`): the door
applies each major's in its own file (`door/apply/`), and the client sends each
from its major's file in `port/`. Beside the two ABIs, only what xpute's form
hands over is called directly. An answer comes back as a number, read once by
`answer_of` into a count or one of xpute's `Errno` members; a code no member
names is the runtime breaking the protocol.

The **client** (`ts/client`) is the TypeScript the app bundles. Its boot
(`ts/client/src/runtime/boot.ts`) starts the kernel with `@xpute/runtime`'s
`start` where the platform runs it (`runtime/open.ts`, `.native.ts`), as the
kernel's machine declares it (generated into `abi/machine.spec.ts`), which asks
for both of xpute's devices, the GPU and the files (`gpu`, `fs`). The config,
made where the kernel runs (`runtime/start.ts`), holds what backs the files
(`sys/fs/namespace.ts` `namespace`), the app ABI's handlers (`shell_imports`),
what the kernel places with the shell, the frame pacing and each turn's
submission (`port/surface.ts` `submit_grant`); `port/kernel.ts` keeps the
runtime's link and the generated stubs bound to it (`kernel`). What else differs
by platform is a file pair in `ts/client/src/service/`, `<x>.ts` the page's
beside `<x>.native.ts` the device's, which Metro picks by the platform it
bundles for (`ts/client/metro.config.ts` `ownPlatform`), with the shapes both
share in `service/types.ts`: the kernel's module or none on a device
(`kernel.ts`), the GPU xpute grants (`gpu.ts`, `@xpute/gpu` `WebGpu`,
`NativeGpu`), the sensors, the visibility, storage, text, the network and the
surfaces; the app's own native module is bound in `service/pxl-platform.ts`.
What answers the kernel's files is `ts/client/src/sys/`: their names, versions,
kept keys and refusals (`sys/fs/`), and its stop report beside. What the app
ABI's client end carries is `ts/client/src/port/`, a file a major (SHELL's
snapshot in `snapshot.ts`, `shell.ts` the shell's way in). The server the client
reads from, its routes, its published tables and whether the device is online,
is `ts/client/src/origin/`; the numbers both read, generated from `spec/` or,
for the states the client reports and the shell shows, written beside it
(`abi/state.ts`), are `ts/client/src/abi/`.

The **shell** (`ts/client/src/shell/`) is React over the kernel's **snapshot**,
where every control is a command. The snapshot is replaced whole after a turn
only when what it is made of moved (`shell.rs` `SnapshotInputs`), and carries
only what the kernel owns; what is rounded or timed for presentation is the
shell's. The shell lends the kernel the surfaces to draw into and computes no
coordinate, tile or draw; what it needs of the model is generated from the same
JSON as the Rust. The runtime, `service/`, `sys/`, `port/`, `origin/`, `abi/` and
the shell are one package, `ts/client`, which one Expo app, `ts/client`, bundles
for iOS, Android and the page; the shell reaches the kernel, `sys/` and
`origin/` through `port/` alone, and `port/` reaches the kernel through the
generated stubs alone.

The stack reads shell and services, kernel, runtime: the clients outside the VM,
the unikernel, and its VMM. Both ABIs ride xpute's one pair of rings, the device
between the shell and the kernel, told apart by a command's major, and the runtime
carries the app's submissions without reading them. The shell meets the runtime
directly only for the surfaces it lends the GPU and the lifecycle (boot,
visibility), through `@xpute/runtime`'s TypeScript API.

```text
  shell      React over the snapshot         ts/client/src/shell/
  services   the app ABI's client end, what  ts/client/src/service/,
             each platform gives             port/, sys/, origin/
    │ ▲   the app's ABI, shell.json and text.json: commands submitted and
    ▼ │   completed; imports: the snapshot, a sensor started, the text called for
  kernel     decides all, owns no device     pxl-kernel's core!
    │ ▲   xpute's ABI: the files and the report called, opens and closes
    ▼ │   completed; the life and GPU_READY raised
  runtime    xpute's, the kernel's VMM       @xpute/runtime, ts/client/src/runtime/
```

What crosses each edge:

| Edge                                        | Down                                                                                                                                                                                                                                                                                        | Up                                                                                                                                                                                   |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| the shell and services, and kernel, `port/` | a command for each control and the input it routes (`INPUT_*`), each turn's grant (`TURN_GRANT`), a sensor's reading, the tile source's part (`SOURCE_DECLARE`, decided in `sys/fs/`), the profiler's reads, the text answered for | the snapshot, replaced whole when it moved, and the events it is told by, a sensor started or stopped among them, the text it calls for (`TEXT_*`), answered before the call returns |
| kernel and runtime, xpute's ABI             | opens, reads and writes, the stop report                                                                                                                                                                                                                                                    | a file read whole or changed (`FS_CHANGED`), the kernel's life and each turn's pacing (`SYS_TURN`), a GPU answered and the surfaces the shell lends (`GPU_SURFACE_*`)                                                                                                                                 |
| runtime and core, xpute                     | a turn with its deadline, the memory once, the clock and the GPU's admit; the commands on the rings                                                                                                                                                                                            | completions, imports and the GPU stream's records, xpute's stop hook if it stops                                                                                            |

## Reads, and the one request scheduler

A read is a file in xpute's sense
([xpute's imports and files](./xpute/ARCHITECTURE.md#the-file-device)):
the kernel names a root by its number and an object below it by one name,
and the client binds each root to what answers it, once at boot
(`ts/client/src/sys/fs/namespace.ts` `namespace`). The roots are
`spec/client/kernel/sys/io/file.json` `root`'s: the bakes and the assets
read-only, `STORAGE` holding the kernel's own files, each a **storage file** on
the device, and `OBJECT` the placed objects' source, its shards and their top
read and its edits written:

| Root               | The kernel's path                               | What the client binds it to                                                                               |
| ------------------ | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| each bake, by kind | a tile's quadkey; the Earth's `sphere`          | the edge's `/tile/<kind>/<version>/<name>`, under the version the declaration gives it (`sys/fs/tile.ts`) |
| `ASSET`            | a load's id, in decimal                         | the shipped asset that id is, by its hash (`sys/fs/asset.ts`)                                             |
| `STORAGE`          | a storage file's id, in decimal (`storage`)     | the platform's storage, under a key it picks (`sys/fs/storage.ts`)                                        |
| `OBJECT`           | a shard's quadkey; `object.TOP`; `object.EDITS` | for now a loopback over the place's database, which applies the edits (`sys/fs/object.ts`)                |

So the kernel names no URL, version, storage key or platform: a path is a
name, and where it leads is the client's. The client reads the declaration
(The query, the only seam) once at boot, hands the kernel the top zoom, each
kind's format and the shards it serves (`SOURCE_DECLARE`, `sys/io/source.rs`), and
keeps the versions; the world's floor and its service area follow from it.

An open answers that the file is there and how long, that there is none, or that
it failed, each by its own variant (`sys/io/file.rs` `Opened`), and only to the
descriptor's owner. An absent tile is an answer with no body, whose meaning its
owner decides: no ground tile means the parent's ground stands
(`sys/io/fetch.rs`). A body is a heap block, which the command that decodes it
releases unless a store keeps it.

How many reads are out at once is bounded by **credits**, which each
`TURN_GRANT` carries, as many as the platform's network runs at once and at
most `MAX_CREDITS`. One **request scheduler** spends them (`sys/io/request.rs`),
and it promises that a camera that has just moved can always state its demand,
however much old work is still draining. That follows from its shape:

- A producer states only what it wants, each with a rank, replacing last turn's
  list; it never starts, stops or retries an attempt.
- Wants and running attempts are two tables of fixed size. An answer ends its
  want; a failure ends only its attempt.
- A running attempt is never preempted, and stops only when nobody wants it.

A **serve policy** ranks the wants (`sys/io/policy.rs`): by layer in the order a
view builds up, then seen before unseen, nearer before farther, and of tiles as
near the finer first. It changes how soon the view is right, never what it is.
Models and textures spend the same credits (`sys/io/loader.rs`), and so do the
objects' shards, wanted as a layer's tiles are.

`STORAGE` holds the settings, the one storage file, opened by its id
(`sys/io/file.rs` `StorageFile`). A file is bytes to the kernel; a backend over
a platform that keeps text keeps their base64url (`sys/fs/storage.ts`), and
`OBJECT`'s database keeps the bytes. A storage file begins with its slot's
format byte, and each file below `OBJECT` with its own (`file.json` `format`);
the kernel refuses one of another format, one its backend cannot read as bytes,
or one that does not decode, as a driver refuses a superblock it does
not know, and never writes over it. It names each refused file in the snapshot
(`refused_files`); the client sets it aside, empties it and says so
(xpute's `FS_CHANGED`, `sys/fs/refusal.ts`), and the kernel reads it again. A shard
whose file is refused stays unread meanwhile, and no edit stands in it.

## When something goes wrong

A run that does not succeed has **ended**, aborted or failed with an errno, a
detail and the source location (`sys/io/failure.rs` `Ended`): a failed fetch is
asked again later, a failed parse is final, and neither touches what is drawn,
until the server stays down past its last wait and the reads are given up, when
the page shows the blue screen `WORLD_UNAVAILABLE`
(`ts/client/src/shell/cover.ts` `coverOf`). A broken invariant is a **bug**,
and it ends the kernel (`xpute_core::stop`, its record `SYS_ABORT`), and
the shell shows the blue screen with the errno and place as its
stop code, as it does for a GPU lost for good: a page's module traps, and on a
device xpute's stop hook says why and the kernel's thread stops there, the
process going on. Memory pressure is neither; the memory map says how each range meets it.

## The turn

The turn and its deadline are xpute's: the runtime rings, the core works to its
deadline, and the falling edge says when to ring again
([the turn](./xpute/ARCHITECTURE.md#the-turn)). pixelet decides what fills a
turn.

```text
                         the rising edge
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│ the frame task                                                 │
│                                                                │
│   effects  each compares the world with what it last saw       │
│   tick     interaction → visible → content → cosmetic → report │
│   pump     start the reads this turn's credits bought          │
│   stage    the frame's uniforms and its draw list              │
│   profile  what the turn cost                                  │
│                                                                │
│ the fetches' arrival and the object store, on what it leaves   │
└────────────────────────────────────────────────────────────────┘
                                │
                                ▼
                        the falling edge  ──▶  wake_ms
```

Before each turn the client submits one `TURN_GRANT` on the kernel's rings, the
turn's pacing and the reads allowed out (`ts/client/src/port/surface.ts`
`submit_grant`), and the runtime then its `SYS_TURN`, the clock; a turn rung
for a command alone carries neither and draws nothing. The view each frame is
drawn at is the world surface's layout box, in CSS pixels
(`GPU_SURFACE_RESIZE`), and a
pointer is placed on it from its top left (`INPUT_POINTER`). The runtime's turns are paced to the display by
the frame pacing the client boots it with (`ts/client/src/runtime/start.ts`,
`@xpute/runtime/sched/frame_pacing.ts`), its numbers the kernel's
(`spec/client/kernel/sys/sched/pacing.json`), at no less than the interval the
kernel last asked (`TURN_PACE`); an input makes the turns after it
interactive (`ts/client/src/port/input.ts` `send_input`) and hurries none past
that interval. The tick's phases are
the kernel's (`spec/client/kernel/sys/sched/phase.json`), and `visible` holds back
the content's share (`sys/sched/tick.rs` `POLICY`). What moves the eye runs in
`interaction`, and after it what the pointer is over where the eye now stands,
the edit it would make and its ghost; what the eye then sees, demand and the
scene, in `visible`; and the later phases hold what the deadline leaves them
(`turn.rs`).

Once a turn, before the layers observe, the kernel says what moved (`turn.rs`
`changes_step`): the tiles whose answer moved, and the regions the object store
wrote. A tile's identity fixes its content, so a list of identities is all a
reader needs.

## The memory map

The client's state is xpute's
[core memory](./xpute/ARCHITECTURE.md#the-core-memory-placed-once), placed
once and never moved, of the sizes the kernel's machine sets
(`rust/client/kernel/machine.json`). Every address is an offset from its first
byte, so the map holds in a tab and on a device alike. The front, from the
stack to the GPU stream, is the machine's, and the runtime lays it before the
first turn. On a device the stack and the kernel's data are the process's, so
their ranges stand empty and the rest begins at the front's end
(`global_base`).

```text
range               what it holds                    placed by
──────────────────────────────────────────────────────────────────
.stack              the stack, first                 machine.json
the header          where the rings and stream are   machine.json
the rings           their frames, then their slots   machine.json
the GPU stream      the renderer's calls             machine.json
.rodata .data .bss  the kernel's data                the linker
.fixed              each owner's range               door/layout.rs
.heap               the rest                         the allocator
──────────────────────────────────────────────────────────────────
```

The map is `pxl-kernel sys/mem/section.rs`'s. `.stack` is first so an overflow
wraps below zero and traps, the only guard without an MMU. Each range is sized
by what full means to what it holds:

- Must-fit state is sized from a bound the structure proves, at compile time, in
  `.fixed`: each owner states its size, and the door lays the ranges in order
  and hands each its own at boot (`door/layout.rs` `FIXED_BYTES`).
- A **bounded flow**, held only at times or far below its worst, grows on
  `.heap` as it is used and is refused past its cap. The flows at their worst
  together are checked at compile time against the heap a phone is guaranteed
  (`door/layout.rs` `HEAP_FLOWS_BYTES`).
- A **workspace**, memory a step holds only while it works, asks the heap first,
  and a refusal is a wait: the step tries again on a later turn.

The draw list (The frame) is laid out on the heap in columns,
structure-of-arrays, each upload a block of its own. The JavaScript heap holds
only glue.

---

# Making the data

Everything the view reads of the world is a published version of baked tiles,
and every tile in it is a function of what the database held and the code that
baked it. The data goes down one chain, run ahead of the view, each stage
reading only the one before it:

```text
sources ─ingest─▶ db-geo ─bake─▶ set ─publish─▶ bucket ─▶ Worker ─▶ client
Overture,         a shard        leaves         its versions,
GEDTM30,          at a time      folded         a declaration
OSM land                         up             of what is
polygons                                        served
```

db-geo is the providers' data held locally, each table saying which release of
each shard it holds; a bake reads it only where every shard it needs is in, and
names the ingest to run where one is not
([infra/geo/README.md](./infra/geo/README.md)).

## A shard is the unit

Every stage names its work by shards (Sizes and positions), each by its Morton
key, and the **coverage** is the shards the bakes publish
(`infra/geo/coverage.json`, read by `pxl-geo-bake coverage.rs` `Coverage`).

A shard holds everything its leaves read, so a leaf bakes from its shard alone,
and shards taken one at a time are the shards taken together. What any bake
reads past a leaf stays within a distance of two zooms over a leaf
(`coverage.rs` `BAKE_READ_DIST`). That distance and a ground cell for rounding
are the shard's **apron**, what it holds past its edges (`SHARD_APRON`). Which
shards are ingested and baked, and when, is the operator's to say.

## A leaf

A leaf is baked from its shard's rows alone: its bridges and tunnels solved, its
ground painted around their lines, and the two written as one terrain tile
(`pxl-geo-bake terrain/leaf.rs` `bake_leaf`). A leaf is baked only where land, a
building or a way reaches it (`has_ground`), and the sea is wherever OSM's land
polygons do not reach.

Rows become features as Carto maps the OpenStreetMap tag each Overture class
comes from, for the classes the paint policy consumes
(`terrain/pipeline/overture/policy.rs` `LAND`). They are painted in Carto's
order, ties broken by the row's own id, so no row order decides a cell
(`terrain/pipeline/feature.rs` `Rank`). A cell takes the palette entry of what
covers its center as seen from above, so a tunnel paints nothing. Polygons are
filled even-odd, and a way paints each cell whose center it covers by the rule
the client's ways cross a column by (`pxl-engine way.rs` `on_way`).

Bridges and tunnels are solved from the tile and as much of its 1-ring as its
structures reach, and never further, starting from Arnis's rules
(`separation/solve.rs`). A height leans on nothing further than a tile, so two
neighbors solve the same height where they meet. A deck is a line in the terrain
tile.

The elevation is sampled at the leaf's resolution, as heights in leaf cells at
the tile's scale, with a **pad** of samples past each edge for the interpolant
(`elevation.rs` `Raster::tile_grid`; pxl-engine ground.rs `ELE_PAD`). The
separation's footing is the height the GPU draws the cell it lies in, off those
samples (`pxl-engine ground.rs` `height`), so a bridge's end meets the ground
drawn. The tile is written as XTP in the form the client reads
(`pxl-spec bake/tile/terrain.rs` `encode`). Place and division leaves are baked
from their own tables, a shard at a time the same way (`pxl-geo-bake place.rs`,
`division.rs`).

## The pyramid

Every tile above the leaves is folded from the zoom under it by its layer's
one-zoom fold ([The layers](#the-layers)), with no database in the loop
(`pxl-spec bake/fold.rs` `fold_terrain`, `pxl-geo-bake pyramid.rs` `Base`). A
terrain parent reads the ring around its four children too, for its
elevation's pad. The ground's majority is the one the client reads a cell's
palette index by (`pxl-engine ground.rs` `own`). The fold stops at the
coarsest zoom the client reads the layer at (`pxl-spec space/layer/lattice.rs`
`coarsest`).

A leaf with nothing drawn is no object, and a tile the bucket lacks is drawn
from the one over it, so open sea costs nothing. A tile with no object folds as
what it stands for (`pxl-geo-bake terrain/absent.rs` `Absence`): sea inside the
shards baked, and unknown past them, which gives no surface, so the planet's
picture shows under it.

A bake folds again only what its shards change: at each zoom the parents whose
folds read a changed tile, each staged only where its content differs, until a
zoom changes nothing (`pyramid.rs` `fold`).

## A set

A bake keeps one **set** a kind, `out/<kind>/`: every zoom of the pyramid flat
under the tiles' names, with a manifest of the holdings it was made from and the
shards it holds. It changes the set only through a stage beside it, a shard at a
time (`pxl-geo-bake tile/shard.rs` `Stage`):

1. `leaf` stages the named shards' leaves, each shard marked whole with the
   leaves it no longer makes.
2. `fold` folds again what the whole shards change, reading the set as it will
   be (`pyramid.rs` `Staged`).
3. `commit`, only once folded, moves the staged tiles in and takes away what is
   gone.

Each step can stop and run again, and while a commit is begun no leaf or fold
runs. A change of a provider's release, the code or the coverage is a purge: the
set deleted and every shard baked again.

A set holds only what a client can. A terrain tile is refused as it is made
where a brick of it would not fill or build as a client's does
(`pxl-spec bake/tile/terrain.rs` `fill_fit`, `pxl-geo-bake terrain.rs`
`encode_with_fit`). That one pass also says what each brick's tree of ways
takes, and the set keeps it beside its tiles (`pyramid.rs` `FITS`). Whether
those trees fit the client's pool wherever the eye is is a fact of the whole
set, since a clipmap level lies across tiles. The commit answers it before
anything moves, from each tile's fit, the stage's over the set's
(`terrain/pool.rs` `check`). A set with tiles and no fits is refused. So at the pool's cap no brick of a published set is
refused ([What the view holds](#what-the-view-holds);
`pxl-engine way/tree.rs` `POOL_CHILD_GROUPS`).

## Publishing a version

A **version** is a copy of a set uploaded under a new name, its format and the
day (`script/publish.py`). The format is the code's
(`spec/spec/bake/publication.json` `FORMAT`), bumped only by a change to how the
client reads a tile. The copy is every tile at the coarsest zoom read and finer,
less, where a kind is pruned, the tiles no reader asks for: those their parent
says are no tile (`tile/shard.rs` `Stage::publish`). Places and divisions are
pruned. The Earth's picture is baked from its imagery straight into a set, no
shard's, and published the same way (`pxl-geo-bake bin/bake_earth.rs`).

Which version of each kind is served is the bucket's alone, in its declaration
([The query, the only seam](#the-query-the-only-seam)), with any shard served
from another version (`pxl-geo-bake source.rs` `declaration`). Only the publish
script writes it, refusing what a client would refuse (`bin/declare_source.rs`);
what the client hands the kernel of it at boot is [Reads, and the one
request scheduler](#reads-and-the-one-request-scheduler)'s. Going back is serving an older
version again.

The bucket is private and read only by the Worker, which serves a tile under its
kind, version and name. A version never changes meaning, so a tile is cached as
immutable. In development a tile server bakes on demand by the same functions
and answers as the bucket does ([infra/README.md](./infra/README.md)).

## Catalogs and assets

Everything a build ships is decided at build time, as JSON under `spec/`, from
the commands and the asset definitions to the settings schema and the formats.
pxl-tool writes a Rust module from each file, and a TypeScript one where the
client reads it, and `make check-spec` fails where the tree differs from its
JSON (`pxl-tool bin/spec.rs`). Both sides compile them, so none is sent.

Which assets exist is compiled too: each is a definition in `spec/spec/asset/`,
numbered by its id ([What fills a cell](#what-fills-a-cell)). A file a
definition references, a model, a skin or the texture pack, is compiled as an
**asset slot**, so the kernel knows "the cumulus' skin" and never which file it
is. Which file fills each asset slot, and what the ingest measured of it, is the
**catalog**: the one file the client plugs in, through which the kernel finds
every other (`pxl-spec asset/catalog.rs` `Catalog`). Until it lands every asset
slot reads as still loading, and its asset slots bound the files the kernel
loads at once (`pxl-kernel sys/load.rs` `LOAD_FILES`).

The asset ingest takes every file under `asset/content/` into the bucket's form,
keyed by the SHA-256 of its bytes, so moving a file changes no key, and writes
the table that numbers each (`spec/spec/asset/ingest.json`, `pxl-tool
bin/ingest_asset.rs`). The client binds an id to its key
(`ts/client/src/sys/fs/asset.ts`).

The texture pack is one WebP image of every cube's tiles, the file the cube
kind's definition names (`spec/spec/asset/cube.json` `pack`), and one table of
which tile a cube wears on each face; the ingest composes both. A textured
palette entry is drawn from afar as its tile's mean, so the ingest draws each
top toward its palette entry's Carto color (`pxl-tool asset/pack.rs`
`TOWARD_CARTO`).

---

# Prior art

What the tree took from work done before it, or found it had arrived at: each
line names the precedent, where the tree follows it, and what it takes. A link
is given where the tree records the source; the rest are named as the code names
them.

| Precedent                                                                                                                              | Where                                                                                   | What is taken                                                                                                                            |
| -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Google Earth and the web map's tile pyramid                                                                                            | the pyramid (`pxl-spec space/`)                                                         | Structure, storage, tiling and invalidation across zooms                                                                                 |
| Cesium 3D Tiles, `skipLevelOfDetail` and `preloadAncestors`                                                                            | `pxl-kernel sys/io/demand.rs`                                                           | A tile tree whose nodes arrive out of order; a held ancestor shown at once                                                               |
| Morton order                                                                                                                           | tile and shard names (`pxl-spec space/hierarchy/morton.rs`)                             | One base-4 digit a zoom, coarsest first                                                                                                  |
| Web Mercator (EPSG:3857), WGS 84                                                                                                       | `pxl-spec space/coord/`                                                                 | The plane a position follows; the sphere's radius                                                                                        |
| Mapbox's globe                                                                                                                         | `pxl-spec space/coord/globe.rs`                                                         | The air's planet under the eye: its radius as the map is stretched there                                                                 |
| Earth Engine's `MODE` pyramiding                                                                                                       | `pxl-spec bake/fold.rs`                                                                 | A 2 × 2 majority fold of the zoom under it                                                                                               |
| Distant Horizons                                                                                                                       | `pxl-render mesh/paint/palette.rs`                                                      | A distant cell drawn as the mean of what it folds                                                                                        |
| Minecraft's `cube_all`, `cube_bottom_top`                                                                                              | `pxl-render gpu/pack.rs`                                                                | Which face image a cube wears: its own side and bottom where the pack ships them, its top on every face where not                        |
| Minecraft Beta's `replaceBlocksForBiome`                                                                                               | `pxl-content strata.rs` `by_depth`                                                      | A column under its face: the look on top, a filler of soil, sand or water, sandstone under sand, stone under all                         |
| Minecraft's map colors                                                                                                                 | `spec/spec/asset/color.json` underground                                                | The colors of soil, sandstone and stone                                                                                                  |
| Minecraft 1.18's noise caves; Ken Perlin's improved noise (SIGGRAPH 2002)                                                              | `pxl-content cave.rs`                                                                   | Tunnels where two noises are both near zero, caverns where one is high; the fade and the gradients                                       |
| Quake III's `PM_StepSlideMove` ([bg_slidemove.c](https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_slidemove.c)) | `pxl-kernel domain/query/support.rs` `step_slide`                                       | Blocked, step up and slide along the wall as `PM_ClipVelocity` clips; ours has no step height but the ceiling's and settles on the floor |
| Amanatides and Woo's voxel traversal                                                                                                   | `pxl-engine cast.rs` `walk`, `pxl-kernel domain/query/support.rs`                    | The next cell along a ray, by the share of the way to each axis's edge                                                                   |
| Laine and Karras, _Efficient Sparse Voxel Octrees_ (I3D 2010)                                                                          | `pxl-engine cast.rs` `walk`                                                          | Push, advance and pop down an octree; a split passed whole where the ray misses its contour, here a box                                  |
| Laine and Karras's beams; Reshetov et al.                                                                                              | pxl-render `beam.wgsl`, `pxl-engine cast.rs` `Until::Beam`                           | Coarse rays at the corners of a block of pixels, from where they stopped the rays between them walk on                                   |
| Tevs, Ihrke and Seidel's maximum mipmaps                                                                                               | `pxl-engine clipmap.rs` `bounded`                                                    | The lowest and highest drawn under a coarse cell, which a ray above it passes whole                                                      |
| The clipmap                                                                                                                            | `pxl-engine clipmap.rs`, `light.rs` `Light`                                          | A zoom's square about a center, each part held at its place modulo the side, so it follows the center and nothing held moves             |
| Burns and Hunt's visibility buffer (JCGT 2013)                                                                                         | `pxl-render gpu/ray/raster.rs` `Raster`, `frame/shader/raster.wgsl`                     | Each pixel keeps the number of the nearest thing drawn over it, here a sample's box, and reads the rest off that                         |
| OpenStreetMap Carto                                                                                                                    | `pxl-geo-bake terrain/pipeline/overture/policy.rs`, `spec/spec/asset/`                  | The ground's palette and paint order, way widths, place icons, colors and zooms                                                          |
| Mapnik's `text_line.cpp`, CSS 2.1 `line-height: normal`                                                                                | `pxl-render look/marker.rs`                                                             | A marker's line pitch                                                                                                                    |
| OpenMapTiles' schema                                                                                                                   | `spec/spec/asset/marker.json`                                                           | A marker shows from z14, as its `poi` layer does                                                                                         |
| [Arnis](https://github.com/louis-e/arnis)                                                                                              | `pxl-geo-bake separation/solve.rs`, `solve/`                                            | Bridges and tunnels: clearances, ramps, decks, pillars                                                                                   |
| osm2streets, osm2lanes                                                                                                                 | the bake's way rules                                                                    | Lane and buffer widths                                                                                                                   |
| Overture's schema                                                                                                                      | `pxl-spec division.rs`, `pxl-geo-bake terrain/pipeline/overture/between.rs`             | Places and divisions read as the schema defines them                                                                                     |
| Bruneton; Hillaire (EGSR 2020)                                                                                                         | `pxl-render look/atmosphere.rs`                                                         | The atmosphere's tables: transmittance and irradiance; multiple scattering and the sky view                                              |
| Unreal's SkyAtmosphere, OPAC, Cornette–Shanks                                                                                          | `pxl-render look/atmosphere.rs`, `frame/shader/atmos_medium.wgsl`                       | The Earth's scattering numbers, the haze, the Mie phase                                                                                  |
| ACES as Stephen Hill fits it                                                                                                           | `pxl-render frame/shader/finish.wgsl`                                                   | One tone curve for every pass                                                                                                            |
| N8AO                                                                                                                                   | `pxl-render gpu/ao.rs`                                                                  | Screen-space ambient occlusion, ported to WGSL                                                                                           |
| Kasten and Young; Ballesteros                                                                                                          | `pxl-render frame/shader/star.wgsl`, `gpu/star.rs`                                      | Airmass at the horizon; a star's color from B−V                                                                                          |
| Duffett-Smith                                                                                                                          | `pxl-kernel domain/scene/environment/celestial.rs`                                      | The sun's, the moon's and the stars' positions                                                                                           |
| Stöckli et al. (Blue Marble)                                                                                                           | `pxl-geo-bake bin/bake_earth.rs`                                                        | The imagery's bytes read back as reflectance                                                                                             |
| three.js                                                                                                                               | `pxl-kernel domain/scene/character.rs`, `pxl-render frame/shader/atmos_view.wgsl`           | A clip's cross-fade; Lambert as its standard material                                                                                    |
| Howard Hinnant's date algorithm                                                                                                        | `pxl-geo-bake util.rs`                                                                  | A civil date from a day count                                                                                                            |
| Figma                                                                                                                                  | `pxl-kernel`                                                                            | A compiled engine in the page over one memory                                                                                            |
| iOS's interface guidelines, Material's motion, SF Symbols                                                                              | `ts/client/src/shell/ui/theme.ts`, `shell/ui/motion/transition.ts`, `shell/ui/icon.tsx` | Touch targets and type scale, easing, the icons' names                                                                                   |
