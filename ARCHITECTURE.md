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
that runs as xpute's guest, the TypeScript around it, and Rust servers and
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
    │   approach.rs  the easing both heads share                    │
    │   param.rs     the tuned numbers both heads share             │
    │   view/        the trunk: the pose, the lens, demand          │
    │   query/       the trunk: the only seam                       │
    │   layer/       the trunk: the base and the object             │
  ──┼───────────────────────────────────────────────────────────────┼───
  sys/                   what all of it stands on                   │
    ▼   io/          reads, credits, the host's end                 │
        sched/       a turn's edge and its phases
        mem/         the core memory's sections
        boot · load · stop   bringing it up, what it loads, its end

  beside   profiler/: reads anything; no decision reads it
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
zoom, possibly empty. A **tile** is what is fetched and stored as one, and a
layer's tile has the same number of cells a side at every zoom. The **pyramid**
is the tree of all tiles: a tile at z covers the 2 × 2 under it at z + 1. The
same zooms, cut through the height as well, make an octree of nodes, eight under
each, and that octree is what a ray descends (The query, the only seam). A
**shard** is the coarsest cube, a tile as wide as the world is tall with the
world's whole height over it (`pxl-spec-core lib.rs` `SHARD_ZOOM`); the world is
ingested, baked and kept a shard at a time.

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

| Side   | Layer    | Stores                                                         | Shape                             | Folds by                          |
| ------ | -------- | -------------------------------------------------------------- | --------------------------------- | --------------------------------- |
| base   | terrain  | the ground's look and height, and its separation               | a raster tile, 2.5D               | majority (look), average (height) |
| base   | place    | the places Carto shows at the tile's zoom, and each one's zoom | a raster tile, an icon a cell     | what shows from the parent's zoom |
| base   | division | the finest division each cell is in, with its ancestors        | a raster tile, `DIVISION_LEAF` up | majority                          |
| base   | earth    | the planet's picture                                           | a raster tile, top zoom only      | —                                 |
| object | object   | what players place                                             | a sparse octree a shard           | majority                          |

**Terrain** is the ground. A tile holds its ground cells, their elevation, and
its **separation**: the ways at another grade, bridges and tunnels, each a line
with a top and a width (`pxl-spec bake/separation.rs`). It also says which of
its children are tiles (`pxl-spec bake/tile.rs` `Occupancy`), so it is read as
soon as it lands. A ground cell is one byte, a **look** from OpenStreetMap
Carto, numbered in the order Carto paints and drawn as land, water, transport or
a structure (`spec/spec/asset/cube.json`); void and the unknown past the shards
baked give no surface. Heights are worked out on the GPU from the elevation and
read back, never sent (`pxl-spec-core ground.rs` `height`).

**Objects** are client-local: one sparse tree, each leaf an asset's id at its
node and each branch its eight folded (`pxl-kernel
domain/layer/object/store.rs`). A write splits leaves only on its path and joins
eight equal leaves into one, so one picture has one tree. A leaf of `AIR` erases
what a lower layer would show. A leaf may carry an **extension**, such as a
marker's words, kept in a pool for its **class** of ids (`pxl-spec
object/ext.rs` `class_of`), placed and erased with its leaf.

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

| Kind   | Base                                | Object          |
| ------ | ----------------------------------- | --------------- |
| cube   | the ground's look, a cell's surface | a placed cube   |
| decor  | what the ground generates           | a placed decor  |
| marker | a place, Carto's icon and its name  | a placed marker |

The ground generates decor by evaluating, never storing: what stands in a leaf cell
is a function of the cell, a seed and the ground's face, one rule on both sides
(`pxl-spec-core scatter.rs`). Two generated decors never share a cell.

A cell coarser than a leaf is what its zoom holds. The ground's coarse cell
stands at its folded height, its look the majority of those it folds, and its
top cell is filled to the share its height reaches (`pxl-spec-core ground.rs`
`top`). What is placed and the ways fold to the kind most of their fill is
(`overwrite.rs` `fold`).

A coarse cell keeps how much of its cube the ground and what is written over it
fill (`answer.rs` `cover`), in eighths, floored: its **kept eighths**. It is
drawn as its cube shrunk about its center by the cube root of that, so its
volume is its fill (`cast.rs` `kept_bounds`), and a ray stops where it enters
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
pointed at (`pxl-kernel domain/control/interaction.rs` `Picked`).

---

# The eye

## The camera

One **camera** looks, and every controller moves it
(`domain/control/camera/controller.rs` `Camera`). It is an **eye** and the
direction it faces, the camera's **facing**, in spherical coordinates about the
world's Z: θ the polar angle from straight down, φ the azimuth from north toward
east. Nothing else keeps a copy.

The first-person camera turns about its eye; the orbit turns about a point a distance
ahead of the eye along the facing. The point a camera turns about is the
**pivot**, the eye itself in the first-person camera, derived and never kept
(`controller.rs` `pivot`). The character stands there, and a change from one
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
alike (`pxl-spec-core world.rs` `node`, which `pxl-kernel domain/query/world.rs`
`World` answers on the CPU):

1. the ground's grid, each cell a cube shifted up to its height;
2. the caves' air under its face, from a hash pyramid asked only at the cells a
   reader asks for (`pxl-spec-core cave.rs`);
3. the ways off the ground, where each crossing of a line over the column is a
   deck and its pier, a bore, an embankment or a cutting (`pxl-spec-core
   way.rs`);
4. then what is placed, air included.

A ray descends the pyramid, by one function the CPU and the GPU compile alike
(`pxl-spec-core cast.rs` `first`). A node coarser than the zoom its distance
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
with what was handed it (`pxl-spec-core support.rs` `Line`). A reader that asks
about the cells beside one it reached asks them off that line, from the deepest
node holding each (`support.rs` `asset`), never again from the top.

Control asks the same world, one ray at a time, at the zoom the eye's rays draw
there, so what control meets is what is drawn (`pxl-kernel domain/query/ray.rs`
`first_hit`). A walker stands on the first cell that fills down its cell's
columns, at the finest zoom placed there (`domain/query/support.rs`), and the
pointer's ray strikes decor as the box it claims where the rays draw decor as it
is (`domain/query/decor.rs`). None reads a layer.

What the bucket serves is said in its **declaration**: the coarsest zoom
supplied whole, each kind's format and version, and the shards served
(`pxl-geo-bake source.rs` `declaration`; Publishing a version). The world ends
in bedrock at the edge of the **service area**, those shards: a walker and an
edit stay inside it, and an edit goes only over known ground
(`domain/query/limit.rs`).

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

The GPU holds the ground's cells in **windows**. A ray observes a zoom's cells
only within a fixed reach of the eye (`pxl-spec-core window.rs` `RADIUS`), so a
window about a center holds every cell it can observe at its zoom. The eye has
one at every zoom drawn; the pivot two, at the leaves and the zoom over them;
and a place the camera is traveling to one, at the leaves
(`domain/layer/terrain/window.rs`):

- A window is a few **bricks** of cells a side, each held in the entry of the
  window its place names, so the window follows its center a brick at a time
  and no held cell moves. A brick at a zoom is held once however many windows
  want it, and kept while any does.
- A brick is filled on the GPU as its tile lands (`pxl-render
  gpu/ray/ground.rs` `grids`): its cells, a min-max mipmap over them, its
  **levels**, and its ways as a tree in one pool every brick shares
  (pxl-spec-core `way::tree`). All three are read back into the kernel, so the
  CPU's query and the GPU's cast meet the same bridge or tunnel.
- A brick whose tree the pool cannot hold at its cap stays absent while a
  window wants it. Where a brick is not present, a reader reads the coarser
  window's (pxl-spec-core `window::step`).
- Nothing of a tile is kept for a brick still to come: a brick that enters
  asks its tile again, and the browser's cache answers.

```text
  one axis of a window, ACROSS bricks a side (drawn as 8); brick x is
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
eye's windows' bricks.

Decor is generated on the GPU from the same windows the rays read (`pxl-render
gpu/generated.rs`). The candidates of a patch, the cells a decor may stand in, are
thinned a thread a patch, each candidate the thinning retains is generated on a
thread of its own, and a candidate is generated again only where what it reads
changed. Every frame each generated record is taken where the rays ask the zoom
it was generated at and compacted into the instances, a model down to its own
size on screen, an impostor below it and culled below that. Placed decor is handed in as records the GPU draws beside it
(`domain/scene/decor/drawn.rs`).

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

| Concern    | Asks                                          | Bounded by                                                                             | Where                                                     |
| ---------- | --------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| Storage    | a layer's records, in the layer's own shape   | nothing of the view                                                                    | a layer; the bucket                                       |
| Query      | what a node holds, a point, or first on a ray | —; it holds nothing                                                                    | `domain/query.rs`                                         |
| Drawing    | what each pixel's ray meets                   | the screen's rays                                                                      | `domain/scene/cast.rs`, `pxl-render gpu/ray/cast.rs`      |
| Demand     | which records an observation requires         | a zoom's reach about the eye (`reach_tiles`)                                           | `pxl-spec space/layer/demand.rs`, `domain/view/demand.rs` |
| Tile cache | which records are held, and why               | its slots, asserted at compile time                                                    | `domain/layer/tile_cache.rs`                              |
| Scheduling | which waiting work runs now                   | a turn's time and the fetches let out at once ([Reads](#reads-and-the-one-scheduler)) | `sys/io/request.rs`, `sys/io/policy.rs`                   |

Every number a person would tune is named once, with its reason, in a `param.rs`
beside the lowest part that reads it (`pxl-kernel domain/view/param.rs`,
`sys/io/param.rs`). None is a correctness bound: set to its least, the view
still converges, only coarser or later.

---

# Drawing

This part turns the eye's rays and the kernel's list into a frame.

## A ray a pixel

Each drawn pixel is one ray on the GPU, walking the query's descent over the
ground's windows, the caves, the ways and the objects' tree as the GPU holds
them. What a ray stops on is the query's answer; the rasterizer only orders
what the rays found.

Neighboring rays share most of their descent, so the cast walks that part once
an angular cell and hands the rest to the rasterizer:

```text
  one angular cell of the camera grid
  ●─────────────●   beam rays at its corners walk the descent its
  │ ·  ·  ·  ·  │   pixels share (beam_cs); the cell starts at the
  │ ·  ·  ·  ·  │   least of them (cell_cs)
  │ ·  ·  ·  ·  │
  │ ·  ·  ·  ·  │   samples, SAMPLE a side, walk on from there, and the
  ●─────────────●   node each stops on is drawn as a box (sample_cs,
                    sample_box_cs)

  each pixel ──▶ the nearest box its own ray enters (raster.wgsl)
                   ├── a node a sample answered ──▶ takes it
                   ├── none, and no sample met ───▶ the sky
                   │   anything
                   └── else ──────────────────────▶ listed, walked on
                                                    its own (miss_list_cs)
```

The entries are pxl-shader's (`beam.rs`, `raster.rs`, `cast.rs` `cast_cs`). Once
a frame the kernel hands them the eye and the camera grid's layout
(`domain/view/lens.rs` `gpu_cast`), and the cast draws at the display's pixels
times a scale the `resolution` setting picks
(`domain/view/frame/resolution.rs`).

What the rays asked of each node is kept between frames in the **observed
pyramid** (`pxl-spec-core observed.rs`), a table keyed by the node, and
forgotten only where what a node read changed (`pxl-shader observed.rs`
`forget_cs`).

A ray leaves four words (`pxl-spec-core ray.rs`): the asset it met, with its
face, zoom and kept eighths; how far; the looks a coarse cell folds; and which
of the face's edges and corners meet the air or a rising cell. The **screen
pass** paints them with the one function every lit face uses
(`frame/shader/screen_ray.wgsl` `pixel_ray`, `frame/shader/paint.wgsl` `paint`):
its swatch or image, shaded edges, light, and the air before it.

## The shadow map

Shadows are read off the shadow map, the world's depth from the sun in a square
box of the light about the eye (`gpu/shadow.rs`), each texel fixed in the world
and held in a map slot by its index modulo the map's side
(`pxl-spec-core light.rs` `Light`). The sun is an observer as the eye is: one set of
passes, written once over an interface both implement (`pxl-shader observer.rs`
`Observer`), asks the world at the eye's zooms, so a shadow has the detail of
what it falls on.

It casts only what a receiver reads:

```text
  the world, where the eye's rays met it ┐
  decor, where it is placed              ├─▶ marks the tile it stands on
  an actor, where it is drawn            ┘   (light_mark_cs, light_mark_drawn_cs)
                                                   │
                                                   ▼
            a tile is read if it lies sunward of one stood on,
            within the tallest decor's height (light_reach_cs)
                                                   │
                                                   ▼
            the tiles read anew, since what a tile holds last moved,
            are cast; the rest stand
```

Decor and the actors, which move, are drawn into the map each frame, their depth
only. A receiver compares nine taps, each moved by its own plane's slope, and
casters are pushed by the same formula, so a plane does not shadow itself
(`shadow.wgsl` `shadow_visibility`). Beside the shadows, the lighting settings
outline an edge that meets the air, lift a rim, and shade corners or run N8AO
(`spec/client/kernel/domain/view/setting/schema.json`, section `lighting`).

## The frame

Each turn ends with a **draw list** (`pxl-render frame/draw.rs`): entries tagged
by the pipeline that draws them, each with its mesh and uniform, for what the
kernel decides to draw beside the world — the cumulus clouds, the actors, the
location puck that shows where the device is, an edit's ghost box, the markers
and the debug lines. An actor's draw also records where it stands
(`DrawList::stand`), for the shadow map. The rays and the generated decor are not in
it.

The kernel stages the frame (`domain/scene/stage.rs`) and calls the renderer
(`gpu/renderer.rs` `render_frame`), which draws every open surface, the world
and the character picker's display cases, and reports what the frame cost and
whether it wants another turn. Its passes are modules by what they draw
(`gpu/ray/`, `gpu/pass/`):

```text
ahead       impostor atlases, fresh bricks filled, the atmosphere's
            tables, the air's volume
cast        the screen's rays; decor generated; the shadow map; the
            occlusion prepass
world pass  generated decor → rays at their depth → generated impostors
            → sky and stars at the far plane → listed actors,
            cumulus, impostors → high cloud planes → puck, ghosts,
            each toned into the world's color, linear
finish      the world's color encoded onto the canvas once
over        markers → debug lines → display cases
```

The calls are WebGPU's, written through xpute's encoder (`gpu/api.rs`) into one
stream, which the page runs on WebGPU after the turn returns
(`ts/app/web/src/webgpu.ts`) and the device app on wgpu
(`rust/client-bracket/native/src/surface.rs`). A frame where nothing it reads
moved submits nothing, and the canvas keeps its frame.

The compute entries are built by rust-gpu to WGSL and checked in beside the
renderer's shaders (`frame/shader/*.gen.wgsl`, pxl-tool `shader.rs` `ENTRIES`);
the renderer's build validates every shader with naga, and the web app's test
compiles each with Tint, the compiler a browser runs
(`ts/app/web/src/shader.test.ts`).

## Past the pyramid: the sky and the sphere

The pyramid is planar and leaves the poles out, but the sky and the ground past
the terrain are drawn too. The earth's picture is a layer: reflectance with city
lights in alpha (`pxl-spec bake/tile/earth.rs`), held only for the floor's tiles
(`domain/layer/earth.rs` `earth_window`), and answered by a coarser global
picture where none is held. The sun, moon and stars, the scattering and the
clouds are no layer: each is evaluated per pixel from a few parameters.

The sky is drawn at the far plane, so only pixels no ray met run it
(`frame/shader/sky.wgsl`). Where such a ray falls to the land, it shows flat
land shaded from the earth layer and lit as the terrain is (`atmos_air.wgsl`
`beyond`). The air between the eye and what is drawn is Hillaire's
aerial-perspective volume (`gpu/atmosphere.rs` `froxel`), read by each drawn
point at its depth (`atmos_air.wgsl` `air_before`).

The **clouds** are three layers on one wind (`pxl-spec-core cloud.rs`): the
cumulus, models drawn like decor but claiming no cell
(`domain/scene/environment/cloud.rs`), and above it altocumulus and cirrus,
planes read where a ray meets them (`frame/shader/cloud.wgsl` `sheet`).

## Markers and the sheet

Markers are one instanced quad over one texture packed shelf by shelf, the
**sheet** (`pxl-render gpu/sheet/shelf.rs`, `frame/shader/marker.wgsl`). A
**sticker**, a design as the sheet holds it, is found by what it paints, never
by who shows it, so equal designs share one place on the sheet
(`domain/scene/marker/design.rs` `Design`). It is drawn on the GPU only when
what it shows changes: its shapes as signed distances, its text as lines the
platform drew, so shaping, bidirectional text and emoji are each platform's own.

---

# Below the view

This part is what the view stands on, `sys/`, and the top that wires the kernel
to its host: how what it asks for arrives, what it costs in time and memory, and
how a failure leaves what is drawn alone. Nothing in `sys/` names anything above
it: a layer asks io for a tile, and io never asks what the tile is for.

## The kernel and its host

The kernel is the process: it holds the world and decides everything about it,
but owns no device. It is a guest of xpute, and asks its **host** for the rest:
one memory, granted once, and a share of time on each call, a **turn**
([xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md)). The kernel owns the
abstractions and the policy over those grants, as a library OS does.

A **bracket** plugs the kernel into one of xpute's guest forms: the kernel
implements xpute's entry (`pxl-kernel door.rs` `run`), and the bracket hands it
what that form hands any guest (`xpute_guest::entry` `BootInfo`,
`xpute_gpu::admit` `Admit`) and pixelet's place to report what stopped it
(`sys/stop.rs` `LastWords`). `pxl-wasm` makes the kernel a WebAssembly module
the page runs (`ts/app/web/src/guest.ts`); `pxl-native` links it into the device
app under xpute's native host, which runs each call on the kernel's own thread,
since a turn that draws through wgpu outgrows React Native's JavaScript stack
(`rust/client-bracket/native/`). Without a bracket the kernel runs under `cargo
test`.

The kernel records its calls during a turn, and the host runs them after it
returns, so each is done before the next turn begins (`sys/io/host.rs`). What
the host answers from what it holds, a text's lines and its pixels, the kernel
asks by xpute's hostcall instead, answered before the call returns
(`sys/io/text.rs`): a page's host through the module's import, a device's on the
JavaScript thread that waits on the turn. Beside those, only what the bracket
hands over is called directly. An answer comes back as a
number, read once by `answered` into a count or one of xpute's `Errno` members;
a code no member names is the host breaking the protocol.

The **client** (`ts/client`) is the TypeScript both apps share. Its **pixelet's
host** is `ts/client/src/host/`, over the app's bracket: the TypeScript one each
app hands the client once, of what its platform gives
(`ts/client/src/bracket.tsx` `Bracket`), apart from the kernel's. The host
mounts `@xpute/host` on the guest the bracket opened (`host/mount.ts`
`open_host`) and names none of pixelet's numbers: what a `WORLD_FRAME` carries,
what an open or a write means and what the stop report says are handed it by the
client's boot (`ts/client/src/abi/boot.ts`), and what answers the kernel's opens
and writes is `ts/client/src/namespace/`.

The host and the kernel share xpute's two rings, one for what the host submits
and one for what the kernel completes. The **commands** are
`spec/client/kernel/domain/control/cmd.json`'s, from which both sides' readers
and writers are generated, each packet an XTP tree, xpute's relocatable binary
tree. The kernel also posts **events** nobody asked for, such as a sensor to
watch or a new snapshot.

The **shell** (`ts/client/src/shell/`) is the UI: React over the kernel's
**snapshot**, where every control is a command. The snapshot is replaced whole
after a turn only when what it is made of moved (`shell.rs` `Shown`), and
carries only what the kernel owns; what is rounded or timed for presentation is
the shell's. The shell hands the kernel the surfaces to draw into and computes
no coordinate, tile or draw; what it needs of the model is generated from the
same JSON as the Rust. The host, `abi/`, `namespace/` and the shell are one
package, `ts/client`.

What crosses each edge, from the shell down to the kernel:

```text
  ┌────────────────────────────────────┐
  │ shell     React over the snapshot  │
  └────────────────────────────────────┘
     │ a command for       ▲ the snapshot, replaced
     │ each control        │ whole when it moved
     ▼                     │
  ┌────────────────────────────────────┐
  │ client    abi/ numbers, namespace/ │
  │           answers opens and writes │
  └────────────────────────────────────┘
     │ WORLD_FRAME and the ▲ completions and events;
     │ commands, through   │ the opens and writes
     │ the host's doorbell │ the host runs
     ▼                     │
  ┌────────────────────────────────────┐
  │ host      @xpute/host, mounted by  │
  │           ts/client/src/host/      │
  └────────────────────────────────────┘
     │ a turn: the memory  ▲ the calls a turn records;
     │ once, the clock     │ the last words, if it
     │ and the admit       │ stops
     ▼                     │
  ┌────────────────────────────────────┐
  │ bracket   pxl-wasm or pxl-native   │
  │ kernel    the guest: decides all,  │
  │           owns no device           │
  └────────────────────────────────────┘
```

## Reads, and the one scheduler

A read is a file in xpute's sense
([xpute's system calls and files](./xpute/ARCHITECTURE.md#system-calls-and-files)):
the kernel names a root by its number and an object below it by its number,
and the client binds each root to what answers it, once at boot
(`ts/client/src/namespace/namespace.ts` `namespace`). The roots are
`spec/client/kernel/sys/io/host.json` `root`'s, the third holding the kernel's
own files, each a **kept slot** on the device:

| Root               | The kernel's path                                              | What the client binds it to                                                                                  |
| ------------------ | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| each bake, by kind | a tile's number, its subject, in decimal; the Earth's `SPHERE` | the edge's `/tile/<kind>/<version>/<name>`, under the version the declaration gives it (`namespace/tile.ts`) |
| `ASSET`            | a load's number, in decimal                                    | the shipped asset that number is, by its hash (`namespace/asset.ts`)                                         |
| `STORAGE`          | a kept slot's number, in decimal (`kept`)                      | the platform's storage, under a key it picks (`namespace/storage.ts`)                                        |

So the kernel names no URL, version, storage key or platform: a path is a
number, and where it leads is the client's. The client reads the declaration
(The query, the only seam) once at boot, hands the kernel the top zoom, each
kind's format and the shards it serves (`WORLD_SOURCE`, `sys/io/source.rs`), and
keeps the versions; the world's floor and its service area follow from it.

An open answers that the file is there and how long, that there is none, or that
it failed, each by its own variant (`sys/io/file.rs` `Opened`), and only to the
descriptor's owner. An absent tile is an answer with no body, whose meaning its
owner decides: no ground tile means the parent's ground stands
(`sys/io/fetch.rs`). A body is a heap block, which the command that decodes it
releases unless a store keeps it.

How many reads are out at once is bounded by **credits**, which each
`WORLD_FRAME` carries, as many as the platform's network runs at once and at
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
Models and textures spend the same credits (`sys/io/loader.rs`).

The settings and the placed objects are the kept slots, each opened by its
number (`sys/io/file.rs` `Kept`). A kept file begins with its slot's format byte
(`host.json` `format`); the kernel refuses one of another format, as a driver
refuses a superblock it does not know, and never writes over it.

## When something goes wrong

A run that does not succeed has **ended**, aborted or failed with an errno, a
detail and the source location (`sys/io/failure.rs` `Ended`): a failed fetch is
asked again later, a failed parse is final, and neither touches what is drawn,
until the server stays down past its last wait and the reads are given up, when
the page shows the blue screen `WORLD_UNAVAILABLE`
(`ts/client/src/shell/layout.tsx` `endedOf`). A broken invariant is a **bug**,
and it ends the module (`sys/stop.rs` `bug`): on a device the process dies with
the kernel and its last words say why, and in a page the shell shows the blue
screen with the errno and place as its stop code, as it does for a GPU lost for
good. Memory pressure is neither; the memory map says how each range meets it.

## The turn

The turn and its quota are xpute's: the host rings, the guest spends what it
was given, and the falling edge says when to ring again
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

Before each turn the client submits one `WORLD_FRAME` through the host's
doorbell, with what the turn may spend and draw into
(`ts/client/src/abi/input.ts` `submit_frame`); a turn rung for a command alone
carries none and draws nothing. The host's turns are paced to the display by
the frame pacing the client boots it with (`ts/client/src/abi/boot.ts`,
`@xpute/host/sched/frame_pacing.ts`), its numbers the kernel's
(`spec/client/kernel/sys/sched/pacing.json`); an input makes the turns after it
interactive (`ts/client/src/abi/input.ts` `send_input`). The tick's phases are
the kernel's (`spec/client/kernel/turn/phase.json`), and `visible` holds back
the content's share (`sys/sched/tick.rs` `POLICY`). What moves the eye runs in
`interaction`, and after it what the pointer is over where the eye now stands,
the edit it would make and its ghost; what the eye then sees, demand and the
scene, in `visible`; and the later phases hold what the quota leaves them
(`turn.rs`).

Once a turn, before the layers observe, the kernel says what moved (`turn.rs`
`changes_step`): the tiles whose answer moved, and the regions the object store
wrote. A tile's identity fixes its content, so a list of identities is all a
reader needs.

## The memory map

The client's state is xpute's
[core memory](./xpute/ARCHITECTURE.md#the-core-memory-placed-once), placed
once and never moved, of the sizes `spec/client/kernel/sys/mem/mem.json` sets.
Every address is an offset from its first byte, so the map holds in a tab and
on a device alike. On a device the stack and the kernel's data are the
process's, so their ranges stand empty and the rest begins at `GLOBAL_BASE`.

```text
range               what it holds                    placed by
──────────────────────────────────────────────────────────────────
.stack              the stack, first                 the linker
.ring.desc          the rings' descriptors           mem.json
.ring.slot          the rings' payload slots         mem.json
the directory       where the rings and streams are  mem.json
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

The draw list (The frame) is laid out on the heap in lanes, structure-of-arrays,
each upload a block of its own. The JavaScript heap holds only glue.

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
reads past a leaf stays within a reach of two zooms over a leaf (`coverage.rs`
`BAKE_REACH`). That reach and a ground cell for rounding are the shard's
**apron**, what it holds past its edges (`SHARD_APRON`). Which shards are
ingested and baked, and when, is the operator's to say.

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
(`terrain/pipeline/feature.rs` `Rank`). A cell takes the look of what covers its
center as seen from above, so a tunnel paints nothing. Polygons are filled
even-odd, and a way paints each cell whose center it covers by the rule the
client's ways cross a column by (`pxl-spec-core way.rs` `on_way`).

Bridges and tunnels are solved from the tile and as much of its 1-ring as its
structures reach, and never further, starting from Arnis's rules
(`separation/solve.rs`). A height leans on nothing further than a tile, so two
neighbors solve the same height where they meet. A deck is a line in the terrain
tile.

The elevation is sampled at the leaf's resolution, as heights in leaf cells at
the tile's scale, with a **pad** of samples past each edge for the interpolant
(`elevation.rs` `Raster::tile_grid`; pxl-spec-core ground.rs `ELE_PAD`). The
separation's footing is read off those samples by the interpolant the GPU draws
the cells' heights by (`pxl-spec-core ground.rs` `bicubic`), so a bridge's end
meets the ground drawn. The tile is written as XTP in the form the client reads
(`pxl-spec bake/tile/terrain.rs` `encode`). Place and division leaves are baked
from their own tables, a shard at a time the same way (`pxl-geo-bake place.rs`,
`division.rs`).

## The pyramid

Every tile above the leaves is folded from the zoom under it by its layer's
one-zoom fold ([The layers](#the-layers)), with no database in the loop
(`pxl-spec bake/fold.rs` `fold_terrain`, `pxl-geo-bake pyramid.rs` `Base`). A
terrain parent reads the ring around its four children too, for its
elevation's pad. The ground's majority is the one the client reads a cell's look
by (`pxl-spec-core ground.rs` `own`). The fold stops at the coarsest zoom the
client reads the layer at (`pxl-spec space/layer/lattice.rs` `coarsest`).

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
`encode_stored`). That one pass also says what each brick's tree of ways
takes, and the set keeps it beside its tiles (`pyramid.rs` `FITS`). Whether
those trees fit the client's pool wherever the eye is is a fact of the whole
set, since a window lies across tiles. The commit answers it before anything
moves, from what each tile kept, the stage's over the set's (`terrain/pool.rs`
`fits`). So at the pool's cap no brick of a published set is refused
([What the view holds](#what-the-view-holds);
`pxl-spec-core way/tree.rs` `POOL_FAMILIES`).

## Publishing a version

A **version** is a copy of a set uploaded under a new name, its format and the
day (`script/publish.py`). The format is the code's
(`spec/spec/bake/published.json` `FORMAT`), bumped only by a change to how the
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
scheduler](#reads-and-the-one-scheduler)'s. Going back is serving an older
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
**catalog**: the one file the host plugs in, through which the kernel finds
every other (`pxl-spec asset/catalog.rs` `Catalog`). Until it lands every asset
slot reads as still loading, and its asset slots bound the files the kernel
loads at once (`pxl-kernel sys/load.rs` `LOAD_FILES`).

The asset ingest takes every file under `asset/content/` into the bucket's form,
keyed by the SHA-256 of its bytes, so moving a file changes no key, and writes
the table that numbers each (`spec/spec/asset/ingested.json`, `pxl-tool
bin/ingest_asset.rs`). The client binds a number to its key
(`ts/client/src/namespace/asset.ts`).

The texture pack is one WebP image of every cube's tiles, the file the cube
kind's definition names (`spec/spec/asset/cube.json` `pack`), and one table of
which tile a cube wears on each face; the ingest composes both. A textured look
is drawn from afar as its tile's mean, so the ingest draws each top toward its
look's Carto color (`pxl-tool asset/pack.rs` `TOWARD_CARTO`).

---

# Prior art

What the tree took from work done before it, or found it had arrived at: each
line names the precedent, where the tree follows it, and what it takes. A link
is given where the tree records the source; the rest are named as the code names
them.

| Precedent                                                                                                                                                           | Where                                                                                                                               | What is taken                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Google Earth and the web map's tile pyramid                                                                                                                         | the pyramid (`pxl-spec space/`)                                                                                                     | Structure, storage, tiling and invalidation across zooms                                                                                 |
| Cesium 3D Tiles, `skipLevelOfDetail` and `preloadAncestors`                                                                                                         | `pxl-kernel sys/io/demand.rs`                                                                                                       | A tile tree whose nodes arrive out of order; a held ancestor shown at once                                                               |
| Morton order                                                                                                                                                        | tile and shard names (`pxl-spec space/hierarchy/morton.rs`)                                                                         | One base-4 digit a zoom, coarsest first                                                                                                  |
| Web Mercator (EPSG:3857), WGS 84                                                                                                                                    | `pxl-spec space/coord/`                                                                                                             | The plane a position follows; the sphere's radius                                                                                        |
| Mapbox's globe                                                                                                                                                      | `pxl-spec space/coord/globe.rs`                                                                                                     | The air's planet under the eye: its radius as the map is stretched there                                                                 |
| Earth Engine's `MODE` pyramiding                                                                                                                                    | `pxl-spec bake/fold.rs`                                                                                                             | A 2 × 2 majority fold of the zoom under it                                                                                               |
| Distant Horizons                                                                                                                                                    | `pxl-render mesh/paint/palette.rs`                                                                                                  | A distant cell drawn as the mean of what it folds                                                                                        |
| Minecraft's `cube_all`, `cube_bottom_top`                                                                                                                           | `pxl-render gpu/pack.rs`                                                                                                            | Which face image a cube wears: its own side and bottom where the pack ships them, its top on every face where not                        |
| Minecraft Beta's `replaceBlocksForBiome`                                                                                                                            | `pxl-spec-core ground.rs` `Cell::asset`                                                                                             | A column under its face: the look on top, a filler of soil, sand or water, sandstone under sand, stone under all                         |
| Minecraft's map colors                                                                                                                                              | `spec/spec/asset/color.json` underground                                                                                            | The colors of soil, sandstone and stone                                                                                                  |
| Quake III's `PM_StepSlideMove` ([bg_slidemove.c](https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_slidemove.c))                              | `pxl-kernel domain/query/support.rs` `step_slide`                                                                                   | Blocked, step up and slide along the wall as `PM_ClipVelocity` clips; ours has no step height but the ceiling's and settles on the floor |
| Amanatides and Woo's voxel traversal                                                                                                                                | `pxl-spec-core cast.rs` `walk`, `pxl-kernel domain/query/support.rs`                                                                | The next cell along a ray, by the share of the way to each axis's edge                                                                   |
| Laine and Karras, _Efficient Sparse Voxel Octrees_ (I3D 2010)                                                                                                       | `pxl-spec-core cast.rs` `walk`                                                                                                      | Push, advance and pop down an octree; a split passed whole where the ray misses its contour, here a box                                  |
| Laine and Karras's beams; Reshetov et al.                                                                                                                           | `pxl-shader beam.rs`, `pxl-spec-core cast.rs` `Until::Beam`                                                                         | Coarse rays at the corners of a block of pixels, from where they stopped the rays between them walk on                                   |
| Tevs, Ihrke and Seidel's maximum mipmaps                                                                                                                            | `pxl-spec-core window.rs` `bounded`                                                                                                 | The lowest and highest drawn under a coarse cell, which a ray above it passes whole                                                      |
| The clipmap                                                                                                                                                         | `pxl-spec-core window.rs`, `light.rs` `Light`                                                                                           | A zoom's square about a center, each part held at its place modulo the side, so it follows the center and nothing held moves             |
| Burns and Hunt's visibility buffer (JCGT 2013)                                                                                                                      | `pxl-render gpu/ray/raster.rs` `Raster`, `frame/shader/raster.wgsl`                                                                 | Each pixel keeps the number of the nearest thing drawn over it, here a sample's box, and reads the rest off that                         |
| OpenStreetMap Carto                                                                                                                                                 | `pxl-geo-bake terrain/pipeline/overture/policy.rs`, `spec/spec/asset/`                                                              | The ground's palette and paint order, way widths, place icons, colors and zooms                                                          |
| Mapnik's `text_line.cpp`, CSS 2.1 `line-height: normal`                                                                                                             | `pxl-render look/marker.rs`                                                                                                         | A marker's line pitch                                                                                                                    |
| OpenMapTiles' schema                                                                                                                                                | `spec/spec/asset/marker.json`                                                                                                       | A marker shows from z14, as its `poi` layer does                                                                                         |
| [Arnis](https://github.com/louis-e/arnis)                                                                                                                           | `pxl-geo-bake separation/solve.rs`, `solve/`                                                                                        | Bridges and tunnels: clearances, ramps, decks, pillars                                                                                   |
| osm2streets, osm2lanes                                                                                                                                              | the bake's way rules                                                                                                                | Lane and buffer widths                                                                                                                   |
| Overture's schema                                                                                                                                                   | `pxl-spec division.rs`, `pxl-geo-bake terrain/pipeline/overture/between.rs`                                                         | Places and divisions read as the schema defines them                                                                                     |
| Bruneton; Hillaire (EGSR 2020)                                                                                                                                      | `pxl-render look/atmosphere.rs`                                                                                                     | The atmosphere's tables: transmittance and irradiance; multiple scattering and the sky view                                              |
| Unreal's SkyAtmosphere, OPAC, Cornette–Shanks                                                                                                                       | `pxl-render look/atmosphere.rs`, `frame/shader/atmos_medium.wgsl`                                                                   | The Earth's scattering numbers, the haze, the Mie phase                                                                                  |
| ACES as Stephen Hill fits it                                                                                                                                        | `pxl-render frame/shader/finish.wgsl`                                                                                               | One tone curve for every pass                                                                                                            |
| N8AO                                                                                                                                                                | `pxl-render gpu/ao.rs`                                                                                                              | Screen-space ambient occlusion, ported to WGSL                                                                                           |
| Kasten and Young; Ballesteros                                                                                                                                       | `pxl-render frame/shader/star.wgsl`, `gpu/star.rs`                                                                                  | Airmass at the horizon; a star's color from B−V                                                                                          |
| Duffett-Smith                                                                                                                                                       | `pxl-kernel domain/scene/environment/celestial.rs`                                                                                  | The sun's, the moon's and the stars' positions                                                                                           |
| Stöckli et al. (Blue Marble)                                                                                                                                        | `pxl-geo-bake bin/bake_earth.rs`                                                                                                    | The imagery's bytes read back as reflectance                                                                                             |
| three.js                                                                                                                                                            | `pxl-kernel domain/scene/actor.rs`, `pxl-render frame/shader/atmos_view.wgsl`                                                       | A clip's cross-fade; Lambert as its standard material                                                                                    |
| Howard Hinnant's date algorithm                                                                                                                                     | `pxl-geo-bake util.rs`                                                                                                              | A civil date from a day count                                                                                                            |
| Figma                                                                                                                                                               | `pxl-wasm`                                                                                                                          | A compiled engine in the page over one memory                                                                                            |
| iOS's interface guidelines, Material's motion, SF Symbols                                                                                                           | `ts/client/src/shell/ui/theme.ts`, `shell/ui/motion/transition.ts`, `shell/ui/icon.tsx`                                             | Touch targets and type scale, easing, the icons' names                                                                                   |
