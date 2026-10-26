# Architecture — pixelet

pixelet draws the real Earth as blocks you can build on, in a browser tab or a
device app, and what it draws is what is walked on. Both follow from one
function. A pixel's ray, a walker's step, the pointer and an edit all ask the
same question of a **node**, a cube of space at a zoom: what fills it, at the
zoom its distance from the eye asks. One function answers
(`pxl-spec-core world.rs` `node`). It lays the ground's grid, each cell a block
at its height with the caves carved under its face, and then writes the ways off
the ground and what players placed over it.

No reader asks block by block. It descends (`pxl-spec-core cast.rs` `first`),
passing whole a node in which nothing fills and splitting only one that may hold
something. The function and the descent are one source compiled for the CPU and,
through rust-gpu, for the GPU, so the block a pixel shows and the block a foot
stands on are one answer.

How finely the world is drawn is the other half, and it is one rule too: the
view is a grid of angular cells about the eye, and a block is drawn at the zoom
whose block spans at least a cell at its distance (`pxl-spec-core grid.rs`
`cell_at`). What a zoom folds away is not there at that zoom, to draw, to walk
on or to miss, and nothing finer is fetched to keep it.

A kind of thing is drawn wholly on one side. The GPU's rays find the world's
blocks, a player's cubes among them, and the GPU grows the ground's decor and
draws it with a player's, which the kernel hands it as records, each where it is
drawn. Everything else the kernel decides to draw, it lists: the actors, the
cumulus, the puck, the ghosts, the markers. The renderer draws the list and
decides nothing but color. Control is headless: the walk, the pointer and the
camera's clearance ask the same query on the CPU and draw nothing.

pixelet is an application over [xpute](./xpute/ARCHITECTURE.md): a Rust kernel
that runs as xpute's guest, the TypeScript around it, and Rust servers and bakes
that share the kernel's model. The memory, the rings and the turn are xpute's
and described there; this document is what pixelet builds on them.

The kernel is one order of dependency, a DAG rather than a pipeline: an edge may
skip a step but never points up, and a reader reaches what it reads directly.
Its folders are that order (`rust/client/kernel/src/`). The top wires
everything. The domain, the world and the eye, is two heads, the scene and
control, over one trunk, the view, the query and the layers, and neither head
names the other. What all of it stands on, the reads, the turn's mechanics and
the memory, is the system below it. The eye asks what it can tell apart, and the
question goes down from a head through the trunk to the reads that bring a tile
in. Each answer comes back up the same way, and nothing names what is above it.
A leaf sits under its lowest reader, so its edges point down too. The figure is
a map; each name in it is defined where its part begins.

```text
  turn · door · shell   the top: wires everything; nothing below names it
  ──────────────────────────────────────────────────────────────────────
  domain/               the world and the eye, two heads over one trunk

  asks                                                         answers
    │   scene/       a head: what this frame draws                ▲
    │   control/     a head: moves the eye, asks the query        │
    │   approach.rs  the easing both heads share                  │
    │   param.rs     the tuned number both heads share            │
    │   view/        the trunk: the pose, the lens, demand        │
    │   query/       the trunk: the only seam                     │
    │   layer/       the trunk: the base and the object           │
  ──┼─────────────────────────────────────────────────────────────┼─────
  sys/                  what all of it stands on                  │
    ▼   io/          reads, credits, the host's end               │
        sched/       a turn's edge and its phases
        mem/         the core memory's sections
        load.rs      what the kernel loads, which io and sched read

  leaves   under their lowest reader: the settings in view/, the models
           and decor in query/, the markers in scene/, each knob in
           a param.rs beside the lowest part that reads it
  beside   profiler/: reads anything; no decision reads it
```

The parts follow that order. **The world** is what the kernel carries: sizes,
positions, layers and what fills a block. **The eye** goes from the eye down to
the layers. **Drawing** is the last step up, from the eye's rays and the
kernel's list to a frame. **Below the view** is what all of it stands on, `sys/`
and the top that wires it: the reads, the turn, the memory. **Making the data**
is a chain of its own, run ahead of the view, that ends where the view reads.

---

# The world

## Every size is a zoom

Every size in the world is one number, its zoom. The world is 2^24 units round,
and a thing whose side is the world's over 2^z is at zoom z: one zoom finer is
half as wide (`pxl-spec space/coord/zoom.rs`).

| Zoom | Side   | What it sizes                                                                  |
| ---- | ------ | ------------------------------------------------------------------------------ |
| z27  | `1/8u` | a placed object's finest cell                                                  |
| z24  | `1u`   | a ground cell, the unit a position names                                       |
| z15  | `512u` | a leaf tile: the finest terrain is fetched and stored at                       |
| top  |        | the coarsest tile, where the pyramid stops (`pxl-spec-core lib.rs` `TOP_ZOOM`) |

The words for what is sized:

- A **cell** is the granule at a zoom, possibly empty. A **voxel** is a cell
  carrying a material.
- A **tile** is what is fetched and stored as one. A layer's tile has the same
  number of cells a side at every zoom (`pxl-spec space/layer/lattice.rs`).
  Terrain's is 512, so its z24 cells make z15 tiles.
- The **pyramid** is the tree of all tiles: a tile at z covers the 2 × 2 under
  it at z + 1, and its Morton key is a prefix of theirs
  (`space/hierarchy/morton.rs`).
- A **block** is what stands in a cell, grown by the ground or placed: a cube, a
  decor or a marker (`domain/layer/object/store.rs` `Entry`).

A tile's zoom is a web map's, and a z24 cell is a web map's z16 pixel; only the
3857 adapter knows it (`adapter/3857.rs` `cell_zoom_of`).

## A position

A position is a **`Q3`**: three `i32` counting 256ths of a unit, so it is the
same bits on every machine (`pxl-spec space/coord/q3.rs`). Above the fraction,
an axis holds 24 bits across, 24 up the map and 16 in height
(`space/coord/world.rs` `EXTENT`). The shell reads coordinates only through the
kernel.

X wraps as its `i32` does, once round the world, and so does the short way
between two positions (`world.rs` `between`). Y and Z are held at their ends as
a position enters the world. A displacement is never a position.

Web Mercator (EPSG:3857) gives the unit its meaning, about 2.389 meters at the
equator. Its plane leaves out the poles (`adapter/4326.rs` `CLAMP_LAT`), and the
pyramid never leaves it. The planet around it is a sphere, which Drawing covers.

## The layers

The world is stored as **layers**. A layer is a payload and a one-zoom fold: a
tile at z is folded from the tiles at z + 1 under it, by the layer's own rule
(`domain/layer.rs` `LATTICE`). Which tiles are fetched is never a layer's
choice.

The layers split on what changes. The **base** is baked from the sources:
terrain, place, division and earth. A base tile is fixed by its name within a
version. The **object** layer is what people author, and changes under its keys.
The line is not dimension or motion.

| Side   | Layer    | Stores                                                    | Shape                         | Folds by                          |
| ------ | -------- | --------------------------------------------------------- | ----------------------------- | --------------------------------- |
| base   | terrain  | the ground's look and height, and its separation          | a raster tile, 2.5D           | majority (look), average (height) |
| base   | place    | the places Carto shows at the tile's zoom, each from when | a raster tile, an icon a cell | what shows from the parent's zoom |
| base   | division | the finest division each cell is in, with its ancestors   | a raster tile, from z12       | majority                          |
| base   | earth    | the planet's picture                                      | a raster tile, top zoom only  | —                                 |
| object | object   | what players place                                        | a sparse octree a shard       | majority                          |

**Terrain** is the ground. A tile holds ground cells, their elevation, and its
**separation**: the ways at another grade, such as bridges and tunnels, each a
line with a top and a width (`pxl-spec bake/separation.rs`). A tile also says
which of its children exist, so it is read as soon as it lands.

A ground cell is one byte, a **look** from OpenStreetMap Carto, numbered in the
order Carto paints (`spec/spec/asset/cube.json`). Each look is drawn as land,
water, transport or a structure. `0` is void and `255` the unknown past the
coverage, and neither gives a surface.

Heights are made on the GPU and read back, never sent (`pxl-spec-core ground.rs`
`height`). Every rounding in pixelet and xpute is half to even, and check-rules
(pxl-tool `bin/check_rules.rs`) refuses any other.

**Objects** are client-local. Their store is one sparse tree, each leaf a
catalog value at its node and each branch its eight children folded
(`pxl-kernel domain/layer/object/store.rs`). A write splits leaves only on its
path, joins eight equal leaves into one, and folds its path again. So one
picture has one tree. A leaf of `AIR` erases what a lower layer would show.

A leaf may carry an **extension**, such as a marker's words, kept in a pool per
**class** of ids (`pxl-spec block/ext.rs` `class_of`). It is placed and erased
with its leaf, which is never joined or split. The store persists in a kept slot
whose format byte is its layout's version (`host.json` `format`).

**Places** are tiles named like the ground tiles under them, each holding what
Carto draws there at that web map zoom (`pxl-spec bake/tile/place.rs`). What
Carto draws for each Overture category is one table from its rules
(`spec/spec/bake/place_theme.json`). A cell one icon wide keeps one place, so
each place carries the zoom it shows from, and a parent is its children with
what shows from its zoom on (doc/minutes/20261009-places-follow-carto.md).
Places are a setting.

The **earth** layer is the planet's picture, baked at its top zoom only
(`pxl-spec bake/tile/earth.rs`).

## What fills a block

What stands in one cell is a block. It has two independent axes:

- Its **kind** is its asset's, how it is drawn: a cube, a decor model, or a
  marker facing the viewer. The object layer stores it as its asset's id, the
  kind over its shape (`spec/spec/asset/id.json` `SHAPE_BITS`).
- Its **source** is which side holds it: the base, whose ground grows it, or the
  object layer.

So decor is no layer. A grown tree and a placed tree are one kind from two
sources, arriving as one record (`domain/layer/instance.rs` `Instance`).

| Kind   | Base                                | Object          |
| ------ | ----------------------------------- | --------------- |
| cube   | the ground's look, a cell's surface | a placed cube   |
| decor  | what the ground grows               | a placed decor  |
| marker | a place, Carto's icon and its name  | a placed marker |

The ground grows decor by evaluating, never storing: what stands in a leaf cell
is a function of the cell, a seed and the ground's face
(`domain/query/scatter.rs` `grown_at`). Two grown decors never share a cell.
Seeds, densities and colors are compiled into both sides, and a variant's model
is a slot the catalog fills (Catalogs and assets).

---

A block coarser than a leaf cell is what its zoom holds. The ground's coarse
cell stands at its folded height, its look the majority of those it folds
([doc/minutes/20261005-the-ground-is-a-carto-palette.md](./doc/minutes/20261005-the-ground-is-a-carto-palette.md)).
Its top block is filled to the share its height reaches (`ground.rs` `top`).
What is placed and the ways fold to the kind most of their fill is
(`overwrite.rs` `fold`).

A coarse block's fill is how much of its cube the ground and what is written
over it cover (`answer.rs` `cover`). It keeps that fill in eighths, floored: its
**kept eighths**. It is drawn as its cube shrunk about its center by the cube
root of that, so its volume is its fill (`cast.rs` `kept_bounds`).

```text
   eight voxels, one solid            the box that replaces them

        ┌───┬───┐                          ┌───────┐
        │   │   │                          │       │
        ├───┼───┤   fill = 1/8             │  ┌─┐  │
        │▓▓▓│   │   ∛(1/8) = 1/2  ──────▶  │  │▓│  │
        └───┴───┘                          │  └─┘  │
                                           └───────┘
                                            half the side, an eighth the volume
```

The box holds to a few rules:

- Nothing is drawn where a block keeps none, and the block under a shrunk top is
  ground no cave takes.
- A ray stops where it enters the box and passes it where it misses. The eye's,
  the sun's and the pointer's rays do alike (`cast.rs` `entry`).
- A shrunk block closes no edge of a block beside it.
- The ray carries the kept eighths, so the pass that colors the screen draws the
  box (`ray.rs` `KEPT_SHIFT`).
- A leaf's block keeps its cube whole, so nothing walked or edited at the leaves
  changes.

So flat ground far away reads as separate cubes rather than a sheet: every axis
is treated alike. What is folded is a summary, so nothing in a folded cell is
struck, hidden, cut by a preview or pointed at (`domain/control/interaction.rs`
`Picked`).

---

# The eye

This part is how the view decides what to hold and draw: only what an observer
can tell apart. A direction answers with the first thing it meets, and how far
that is chooses how finely it is drawn. So the view's work is bounded by the
screen, not the world: a city or two hundred towers behind a ridge cost what the
screen they cover costs.

## The camera

One **camera** looks, and every controller moves it
(`domain/control/camera/controller.rs` `Camera`). It is an **eye** and a
**look**, in spherical coordinates about the world's Z. θ is the polar angle
from straight down and φ the azimuth from north toward east. Nothing else keeps
a copy.

- The free camera is first person: it turns about its eye and walks it.
- The orbit is third person: it turns about a point a distance ahead of the eye
  along the look, and that distance is the only state it adds.

The point a camera turns about is the **focus**: the eye itself in first person,
the point ahead in the orbit. It is derived, never kept, and it is where the
**companion**, the character, stands. A change of person moves the camera about
the focus, never the focus.

The **anchor** is the eye's longitude and latitude, which the sky is seen from
(`domain/view/frame/pose.rs` `anchor`). Generic code reads the eye, the look and
the focus, never a controller's own words.

The orbit's controls are written only through one funnel
(`domain/control/camera/orbit.rs` `ask`). There terrain may ask whether a state
puts the eye inside the ground (`domain/control/camera/orbit/terrain_gate.rs`).
The camera setting answers: through ignores terrain, stop takes a control as far
as it can, and climb raises the drawn pitch instead.

A pan, the companion and the orbit's climb move by one rule
(`domain/query/support.rs` `step_slide`). Where the camera grid draws the ground
coarser than its own cells, the world is a map, and the focus goes where it is
moved. Where the grid draws the leaves, the focus walks them a cell at a time, a
step-slide after Quake III's. It climbs what blocks it while its cell is open
overhead, and slides along an axis a ceiling stops. With no jump, only the
ceiling bounds a climb.

A walker meets its cells as they fold at its zoom, so what folds away is not
met. It stops at the edge of a leaf not yet held. Until its footing is known, it
is drawn on the finest ground known there.

The **pose** is the eye and the turn from the world's axes to the camera's. The
world stays in its own axes, in doubles; what turns is the observer. Every draw
uploads its point less the eye, subtracted in doubles before it is narrowed.

The pointer is apart from the camera: the tool held and what a tap does belong
to `domain/control/interaction.rs`, and what is under it to `pointer.rs` alone,
a marker a tap would act on first, then what the eye's ray meets first.

## The camera grid

The camera grid sets how finely each part of the view is drawn. Each of its
angular cells is the least the observer promises to tell apart: a solid angle
about the eye, the same wherever it lies. A screen only calibrates it, to about
twelve drawn pixels at the optical axis. Inside a cell nothing is promised, as
long as nothing leaks into a neighbouring cell beyond what the area measure
allows.

That gives the one rule of detail: what is drawn is what the eye's rays reach. A
ray goes down the pyramid and stops at the first block that fills at the zoom
its distance asks. That block is what its pixels show; nothing no ray stops on
is drawn. The GPU casts one ray a drawn pixel (Drawing).

Height is not special: the distance is to the block met, every axis alike. The
rule is not yet closed by area: a flat top seen grazing is drawn finer than it
shows (doc/backlog.md, P1).

**τ** is one factor on the distance each zoom splits at
(`pxl-spec space/camera/grid.rs` `Tau`), and the ground's tile zoom for a
distance follows from it (`space/layer/lattice.rs` `z_at`). τ = 1 is the finest
cell, stated at the finest device pixel ratio, so what is sized at τ = 1 holds
at every ratio. τ is the screen's alone (`domain/view/lens.rs`).

## The query, the only seam

The query is the only place a layer's shape is known. The terrain and the object
store answer one question of a node, a tile or a cube, at a cell zoom the asker
states, in the form they share (`domain/layer/query.rs` `Query`); places,
divisions and the Earth answer none.

- Its **bound** is nothing known, empty, or inside a conservative box.
- A descent carries a **cursor** from a node to its children, so each step is
  answered from its parent and never again from the top.
- Each **answer** says what it lacked: exact, coarser with the tile it needed,
  or nothing and that tile (`domain/layer/answer.rs` `Answer`).

The query fetches and keeps nothing, so reading costs what is asked, not what a
tile holds.

The **world** composes the layers into one answer (`domain/query/world.rs`
`World`). What is in a block is one rule, for drawing and for control alike
(`pxl-spec-core world.rs` `node`):

1. the ground's grid, each cell a block shifted up to its height;
2. the caves' air under its face, from a hash pyramid asked only at the blocks a
   reader asks for (`pxl-spec procedural.rs`);
3. the ways off the ground, where each crossing of a line over the column is a
   deck and its pier, a bore, an embankment or a cutting
   (`pxl-spec-core ways.rs`);
4. then what is placed, air included.

A ray asked of the world reads that rule at the zoom the eye's rays draw there
(`domain/query/ray.rs` `first_hit`), so what control meets is what is drawn. A
walker casts the same ray up or down its cell's columns, at the finest zoom
placed there, so it stands on what is placed (`domain/query/support.rs`).

The world ends in bedrock, laid as a bowl: a bottom under the ground, and a wall
just inside the **service area**, the shards the source serves. Walking and
editing stay inside it, and an edit goes only over known ground, between the
bedrock and a ceiling (`domain/query/limit.rs` `World::editable`). The camera is
no walker: it goes anywhere a position can be, past the area over the Earth
(`step_slide`'s `walled`).

A ray descends the pyramid
([a ray asks the world whether a node is empty](./doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md)).
A node coarser than the zoom its distance asks is not what is drawn there. So it
is asked only whether anything under it fills at the zooms the ray asks; what
those zooms fold away is not there to miss.

```
a node coarser than the zoom its distance asks
   +-- nothing under it fills ---------> passed whole
   +-- something may, or not held -----> split; children asked in
                                         the order the ray meets
                                         them, each handed what its
                                         parent found
a block at the zoom its distance asks
   +-- fills --------------------------> met: its pixels show it
   +-- not known ----------------------> the ray stops there
```

One query answers both (`domain/query/node.rs` `World::node`), and each source
answers from what it keeps:

- the ground, by the bound the GPU made over its held cells, or where a cave may
  take it;
- the ways, by a bound of their own beside the ground's;
- the caves, by their hash node, hollow and drawing nothing again under it;
- what is placed, by nothing held under the node in the objects' tree.

The pointer's ray also strikes decor against its model, but only where the rays
draw decor as it is, so what is struck is what is drawn
(`domain/query/decor.rs`).

The pointer, the camera's clearance, the walk and the view's observations all
ask the world; none reads a layer. The descent is one function
(`pxl-spec-core cast.rs`), which the GPU's cast compiles (Drawing). The CPU asks
it one ray at a time, for control, never for the screen.

## Windows and squares

The GPU holds the ground's cells too, but only where a ray can reach them. A ray
observes a zoom's cells only within a fixed reach of the eye
(`pxl-spec-core window.rs`). So a **window** about the eye holds every cell it
can observe at that zoom.

A window is a few **squares** of cells a side, each held in a slot its place
names. It follows its centre a square at a time, and no held cell moves. The eye
has one at every zoom drawn, the focus two, and a destination being gone to one
(`domain/layer/terrain/window.rs`). Windows at one zoom want a square in the
same slot, so a square is held once, in a **berth** of that slot, however many
windows want it, and kept while any does. The focus's two windows are at the
eye's finest zooms, so where they overlap the eye's they take no berth more. A
window takes its own berth where it is free, but after two windows have
overlapped and parted, one may hold its squares in the other's.

A square is filled on the GPU as its tile lands. Compute passes make each cell's
**column**, its ground's top and the runs its ways write over it along z
(`heights_cs`, `ways_cs`). They fold a bound over the columns into **levels**
(`bound_cs`). Both are read back, so the CPU's query and the cast meet the same
bridge or tunnel.

Its ways are built into a tree of families, a node's eight children each, from
one pool every square shares, on the GPU, and its families read back into the
CPU's mirror of the pool. Both grow as the trees take families, up to a cap
(`domain/layer/terrain/param.rs` `WAYS_POOL_BYTES`). A square whose tree the
pool at its cap cannot hold, or past a square's own most, or whose families lie
past the mirror's cap, stays absent while a window wants it. One whose families
the heap refuses the mirror waits until the heap gives a page back, and is
filled again.

Where a square is not present, both read the coarser window's. A square that
enters asks its tile again, and the browser's cache answers.

## Demand and residency

Demand is what the view asks for, and residency what it keeps. **Demand** is
what an observation requires, the ground at the resolution its distance chooses,
before it is any tile; the layer's storage maps it to tiles
(`domain/view/demand.rs` `observe`).

Demand is a function of the eye alone: at each zoom, every tile within the band
of distance that zoom is drawn at (`domain/view/observation.rs` `ByEye`). It
never depends on what the rays met. A tile it requires that is not held is a
miss. Every layer is asked this way, the place layer too, and holds what it is
asked for in a residency of its own.

The **floor** is a law: every observation has an answer. It is kept by holding
the coarsest zoom the source supplies whole, all around the eye
(`domain/layer/residency/floor.rs`).

**Residency** holds a tile hard for the floor, for demand while it is demanded,
or as cache; only the hard set is correctness. Its capacity is asserted on its
**frames**, the slots tiles are held in (`Residency::FRAMES`): 76 for terrain,
649 for places, 9 for divisions and 4 for the Earth. A tile that arrives takes a
free frame, or else the farthest one nothing holds hard and the view does not
read. A region goes from unresolved to coarse to as fine as it is observed, and
never back.

The view is loading until something has reached the screen and every tile
demanded shows, exact or coarser (`domain/layer/residency.rs`
`Residency::shown`). A version is never rewritten, so a tile never changes under
its key; what changes is which tile answers a node. **Nothing is prefetched**: a
view converges to exact on its misses alone.

## What stands over the squares

What stands beside the ground, decor and the markers, is gathered over the eye's
windows' squares. The GPU grows decor in every window's squares and draws each
where the rays ask the zoom it was grown at.

The markers read the squares as one partition of the ground
(`domain/view/observation.rs` `squares`). A square splits into its four while
its nearest point is nearer than its cells' split distance, as a ray's descent
does, and each square left is read at its window's zoom, between the heights the
world stands at in it. The partition never depends on what a ray met.

Decor is grown and drawn on the GPU each frame, a thread a candidate, never
stored (`pxl-render gpu/grown.rs`). Placed decor is handed in beside it as
records (`domain/scene/decor/drawn.rs`). A kind is drawn as a model down to its
own size on screen, and as a sprite below it.

## Markers

A **marker** is what is drawn flat to the camera, a billboard, from a design on
the sheet: a place, a pin someone put up, or the chat's line. Each source reads
its own, and one arrangement draws them together
(`domain/scene/marker/arrangement.rs`). The places and the pins are read over
the windows' squares, each standing at its cell's center on the surface drawn
there; the chat's line is read from no square, and hangs where its speaker, the
person, holds it, over its head.

A place is the place layer's marker (`domain/scene/marker/place.rs`). Each
square in view reads its place tiles, and each place shows what Carto shows at
the zoom the camera grid's cell where it stands is: a dot or an icon, named from
its kind's label zoom (`domain/scene/marker/place.rs` `point_of`). A place
stands on what a ray down its cell meets first at the detail drawn there, and
where nothing is drawn there is no place. A tap on one is the place's before the
ground's.

---

## Each concern's bound

Taken together, the view is six concerns between the world and the screen, each
with its own bound:

| Concern    | Asks                                          | Bounded by                                   | Where                                                     |
| ---------- | --------------------------------------------- | -------------------------------------------- | --------------------------------------------------------- |
| Storage    | a layer's records, in the layer's own shape   | nothing of the view                          | a layer; the bucket                                       |
| Query      | what a node holds, a point, or first on a ray | —; it holds nothing                          | `domain/query.rs`                                         |
| Drawing    | what each pixel's ray meets                   | the screen's rays                            | `domain/scene/cast.rs`, `pxl-render gpu/ray/cast.rs`      |
| Demand     | which records an observation requires         | 2 × 2 ground tiles a zoom, 8 × 8 place tiles | `pxl-spec space/layer/demand.rs`, `domain/view/demand.rs` |
| Residency  | which records are held, and why               | its frames, asserted at compile time         | `domain/layer/residency.rs`                               |
| Scheduling | which waiting work runs now                   | the turn's quota and the credits             | `sys/io/request.rs`, `sys/io/policy.rs`                   |

Every number a person would tune, such as τ's target, the screen's cells and
decor's tiers, is named once, with its reason, in a `param.rs` beside the lowest
part that reads it (`pxl-kernel domain/view/param.rs`, `sys/io/param.rs`). None
is a correctness bound: set to its least, the view still converges, only coarser
or later.

---

# Drawing

This part turns the eye's rays and the kernel's list into a frame: the GPU draws
the world from the rays and the decor it grows and is handed, the kernel lists
everything else, and the renderer executes both and decides nothing but color.

## A ray a pixel

Each drawn pixel is one ray on the GPU. It walks the same descent the CPU runs,
over the ground's windows, the caves, the ways and the objects' tree as the GPU
holds them. The block it stops on is the pixel's
([the GPU casts a ray a pixel](./doc/minutes/20261014-the-gpu-casts-a-ray-a-pixel.md)).
Once a frame the kernel hands the GPU the eye and the camera grid's layout
(`domain/view/lens.rs` `gpu_cast`), so every pass reads one grid.

Neighbouring rays share most of their descent, so the cast walks that part once
and hands the rest to the rasterizer, in five steps (`pxl-shader beam.rs`
`beam_cs`; [a cell casts a beam](./doc/minutes/20261022-a-cell-casts-a-beam.md)):

1. A **beam ray** at each corner of a camera grid cell walks the descent the
   cell's pixels share, and stops before a node too narrow to share.
2. Each cell starts at the least of the beam rays about it.
3. **Samples**, four a cell a side, walk on to the blocks they stop at.
4. Each block a sample stopped at is drawn as a box
   (`pxl-render gpu/ray/raster.rs`, `frame/shader/raster.wgsl`). It draws only
   the face its sample's ray came in through, and a pixel keeps that face only
   where its own ray enters the block through it. The depth test keeps the
   nearest, so each pixel holds the nearest sampled block its ray enters.
5. The pixel takes that block (`pxl-shader cast.rs` `cast_cs`), and walks itself
   only where it holds none, or where the nearest is a node a sample could not
   answer.

The block can come from any sample on the screen, not only the ones about the
pixel, and a block no sample stopped at is not drawn. A pixel whose ray first
meets such a block shows the drawn one behind it, a cost
[the raster picks](./doc/experiment/20261026-the-raster-picks.md) measured
against each pixel's own walk.

What the rays asked of each node is kept between frames in the **observed
pyramid** (`pxl-spec-core observed.rs`), forgotten only over squares that moved.
A pixel needing a node it lacks walks the world again in the same pass.

A ray leaves four words (`pxl-spec-core ray.rs`): the block it met, with its
face, zoom and kept eighths; how far; the looks a coarse block folds; and which
of the face's edges and corners meet the air or a rising block.

The **screen pass** reads those words (`frame/shader/screen_ray.wgsl`
`pixel_ray`) and paints the block with the one function every lit face uses
(`frame/shader/paint.wgsl` `paint`): its swatch or image, shaded edges, light,
and the air before it. Its depth is set back by a share of the face's slope
across a pixel, so what lies on a face, like the location puck, wins with no
bias of its own.

The one mesh left is the **ghost**, the box an edit is about to place
(`pxl-render mesh/ghost.rs`). The CPU draws nothing of the world; it runs the
query for control and decides what the GPU is handed.

## The render resolution

The render resolution sets how many pixels the cast draws
(`pxl-kernel domain/view/frame/resolution.rs`). It is the display's device
pixels per CSS pixel, capped, times a scale the `resolution` setting picks:
full, one device pixel a CSS pixel, or, by default, auto.

Auto judges moving frames by their GPU time against the paced interval. It steps
down at once when over budget, and up only after a run of windows the higher
step would fit; without GPU timing it counts missed frames instead. The camera
grid is laid by the display's ratio, not the drawn one, so a block's zoom and
what is fetched never follow the load.

## Light on the blocks

Three settings shade a block beyond its light
(`spec/client/kernel/domain/view/setting/schema.json`):

- **Edges** draw a line along every crease, an edge meeting the air or a rising
  block. It has one width in units at every block size, so only perspective
  thins it.
- **Corners** either darken a face along each edge and corner a block rises
  across, tinted warm (`screen_ray.wgsl` `creases`), or run N8AO over the rays'
  depth and the grown decor (`gpu/ao.rs`).
- **Shadows** read the sun's map, below.

The **sun's map** is a square box of the light about the eye (`gpu/shadow.rs`).
Its texels are fixed in the world, each in a slot by its index modulo the map's
side (`pxl-spec-core sun.rs` `Sun`), so the map follows the eye a texel at a
time, and a texel that stays is not cast again.

A slot's ray runs along the sun through the query, again only where the box
moved onto it, the eye asks its stop at another zoom, or a node it passes
changed. A step of the sun casts the whole map. Decor and the actors are drawn
into it each frame.

A receiver compares nine taps, each moved by its own plane's slope
(`shadow.wgsl` `sun_lit`). Casters are pushed by the same formula, so a plane
does not shadow itself.

## The frame

Each turn ends with a **draw list** (`pxl-render frame/draw.rs`), one for the
kernel's life, kept beside the models whose meshes it uploads
(`domain/query/model.rs` `draw_list`): entries tagged by the pipeline that draws
them, each with its block and uniform. It holds the cumulus (instanced as decor
is, or as an **impostor** sprite where a model is drawn too small to be worth
it), the actors, the location puck, an edit's ghosts, the markers and the debug
lines. The rays and the GPU's decor are not in it; they are the cast and the
grown decor (`gpu/grown.rs`).

The kernel writes the frame's uniforms into one lane of doubles
(`frame/stage.rs`) and calls the renderer (`gpu/renderer.rs` `render_frame`)
with the list, the models, the markers' sheet, the earth's tiles, the windows'
squares and the cast. It draws every open surface, the world and the character
picker's display case, and reports the cost and whether it wants another turn.
Its passes are modules by what they draw: the rays, cast over the squares the
GPU fills (`gpu/ray/`), and each pass beside the world, the decor, impostors,
models, markers, the puck, the gizmos and the display case (`gpu/pass/`).

A frame runs in this order:

```text
ahead       impostor sheets, fresh tiles, the atmosphere's tables,
            the air's volume
cast        the screen's rays; the sun's map; the occlusion prepass
world pass  grown decor → rays at their depth → grown impostors
            → sky and stars at the far plane → listed actors,
            cumulus, impostors → high cloud planes → puck, ghosts
over        markers → debug lines → display cases
```

The calls are WebGPU's, written through xpute's encoder (`gpu/api.rs`) into one
stream in the kernel's memory. The page runs it on WebGPU after the turn returns
(`ts/app/web/src/webgpu.ts`); the device app runs it on wgpu inside its turn, on
the kernel's thread (`rust/client-bracket/native/src/surface.rs`). Every GPU
object is an xpute handle, and a test links no GPU.

A frame stands where nothing it reads moved. The rays are cast again only when
the eye, the windows, the shards or the overwrites moved, or a square came in
fresh; decor alone casts no ray. Where everything else also reads as before,
nothing is submitted and the canvas keeps its frame.

So a still view draws nothing, and a clock steps the sky by a display level and
the clouds by a pixel at the nearest cloud. A frame too large for the stream is
dropped whole, and the next turn draws again.

Shaders and pipelines are `pxl-render`'s (`frame/shader/`). The compute entries
are `pxl-shader`'s, a module each by what it computes (`beam.rs`, `raster.rs`,
`cast.rs`, `observed.rs`, `sun.rs`, `ground.rs`, `ways.rs`, `decor.rs`), each keeping the
name the build reads it by (pxl-tool `shader.rs` `ENTRIES`), built to WGSL and
checked in beside the rest (`*.gen.wgsl`). The renderer's build validates each
shader with naga and takes each pipeline's binding stages from it (pxl-render
`build.rs` `assemble`), and the web app's test compiles each generated one with
Tint, the compiler a browser runs (`ts/app/web/src/shader.test.ts`). What a
shader shares with the host has no padding, and a uniform block is read a field
at a time, never rebuilt as a struct; a phone's Vulkan and Safari's WebGPU each
broke on one. A fact the GPU needs has one home, such as the frame uniform's
cell angle.

Depth is reversed float, near at 1, so far coplanar faces do not fight. On
device loss the uploads' bytes are still in memory and are uploaded again from
the same addresses.

## Past the pyramid: the sky and the sphere

The pyramid is planar and leaves the poles out, but the sky and the ground past
the terrain must be drawn too. A thing becomes a layer only if there is
something to hold:

- **Stored data** has extent and resolutions, so it is a layer, refined by the
  eye's rule. The earth's picture is one.
- **An evaluated model** is computed per pixel from a few parameters: the sun,
  moon and stars from the clock and the anchor, the scattering from its tables,
  the clouds from their field.

The earth layer is reflectance with city lights in alpha, a ground cell a texel
(`pxl-spec bake/tile/earth.rs`). Only the floor's tiles are held
(`domain/layer/earth.rs` `earth_window`), so it adds no demand. Where none is
held, a global picture eight times coarser answers by latitude and longitude.

The sky is drawn at the far plane, so only pixels no ray met run it
(`frame/shader/sky.wgsl`). Where such a ray falls to the land, it shows flat
land past the terrain, shaded from the earth layer and lit as the terrain is, so
the edge is no seam (`atmos_air.wgsl` `beyond`). The planet is only the air's
medium, a centre and a radius.

The air between the eye and what is drawn comes from the **air's volume**,
Hillaire's aerial-perspective volume (`gpu/atmosphere.rs` `froxel`). A compute
pass marches each of a grid of screen columns once, by stretches between
froxels spaced alike in every column by depth cubed, joined from the eye, so
every froxel is one integral's to its depth. A froxel holds the light the air
scatters per share of what it takes, which the air's thickness cancels from.
A drawn point reads that by a cubic B-spline in all three axes, and scales it
by its own ray's share taken, exact from the transmittance table
(`atmos_air.wgsl` `air_before`), so neither the columns nor the slices show.

The **clouds** are three layers on one wind (`pxl-spec-core cloud.rs`). The
cumulus is models drawn like decor but claiming no cell, so a tower stands
through it (`domain/scene/environment/cloud.rs`). Each sits by a hash in a
lattice of its own, a model, a sprite or nothing by its size on screen, and
thinned as drawn, a quarter of them each octave farther (`THIN_FROM`).

Above it, altocumulus and cirrus are planes read where a ray meets them
(`frame/shader/cloud.wgsl` `sheet_t`), laid over the world without depth. From
below they fill only the sky and from above they cover everything, so a peak
above one is knowingly wrong. Every layer fades out as the eye climbs toward
where the clouds are gone (`domain/scene/environment/cloud.rs` `clouds_shown`).

## Markers and the sheet

A marker is drawn flat to the camera, where its source stands it (Markers).

Markers are one instanced quad (`domain/scene/marker/set.rs`) over one texture
packed shelf by shelf, the **sheet** (`pxl-render gpu/sheet/shelf.rs`). The pass
(`frame/shader/marker.wgsl`) tests several points of each marker against the
world's depth and drops the marker when most are hidden. It writes depth from an
upright twin of the quad, so markers sort against the world along its up. The
chat's bubble is a marker too, which the person holds over its head and says
whether it shows (`domain/scene/character/companion.rs` `line_hangs`).

A **sticker**, a design as the sheet holds it, is found by its design, what it
paints, never by who shows it: equal words, colors and look share one slot, let
go once nothing shown or waiting uses it (`domain/scene/marker/design.rs`
`Design`). What shows it is named apart, by its holder's key, which the old
drawing kept until the new one is painted goes by
(`domain/scene/marker/arrangement.rs` `Key`); where a tap on it goes is each
instance's own (`Instance::tap`). A sticker is drawn on the GPU only when what
it shows changes (`gpu/sheet.rs`): its shapes as signed distances, its text as
lines the platform drew. Asking the platform a line at a time leaves shaping,
bidirectional text and emoji to each platform's own engine
([text by the line](./doc/minutes/20260926-the-sheet-on-the-gpu-and-text-by-the-line.md)).

Pins, the markers people put up, are an object kind
(`spec/spec/asset/marker.json`) drawn by the same pass, whether or not places
are on. Where several share a cell, the cell shows their count on a pin's pill
in its colors turned over, counted afresh each time the cube is read. Unlike
places, pins are one kind, so their number means something. Each keeps its own
words with its entry (`domain/layer/object/ext.rs` `Ext::MarkerText`).

---

## Debug views

Five debug settings show what the view does, drawn from what the frame already
holds.

- `view`: where the camera and the pointer are, in a panel.
- `profiler`: the profiler's panel (Below the view).
- `world_gizmo`: the focus, its cell and rings about it
  (`domain/scene/indicator/frame_gizmo.rs`), and over the ground the eye's
  windows' squares by how they stand, with their tiles and shards
  (`coverage_gizmo.rs`).
- `camera_gizmo`: the camera grid over the screen, rings a cell apart cut into
  cells of one solid angle (`frame/shader/camera_grid.wgsl`), and each pixel's
  block tinted by the share of a cell its face shows the eye (`screen_lit.wgsl`
  `cell_share`).
- `ray_steps`, in a development build: each pixel coloured by the nodes its own
  ray walked, grey where it took the block the raster kept.

---

# Below the view

This part is what the view stands on, `sys/`, and the top that wires the kernel
to its host: how what it asks for arrives, what it costs in time and memory, and
how a failure leaves what is drawn alone. Nothing in `sys/` names anything above
it: a layer asks io for a tile, and io never asks what the tile is for.

## The kernel, its host, its brackets and the client

This section says where the kernel runs and what plugs it in.

The kernel is the process: it holds the world and decides everything about it,
but owns no device. Like any guest of xpute, it asks its **host** for the rest.
The host grants it one memory once and a share of time on each call, and runs
what the call recorded after it returns. One such call is a **turn**. That
mechanism is xpute's and names nothing of pixelet
([xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md)). The kernel runs like a
library OS over its host: the host grants memory, time and access to the
outside, and the kernel owns the abstractions and the policy built over them.
The split follows the exokernel/library-OS division, and xpute does not enforce
those grants. The rings borrow their structure from virtio and io_uring, and the
roots and files borrow Plan 9's file namespace.

The kernel's **bracket** plugs the kernel into a platform. The kernel implements
xpute's entry, `intr` (`door.rs` `intr`), and what the bracket's form hands it
there is xpute's too: the memory, where its own ranges may begin, the clock and
the allocator's reading (`xpute_guest::entry` `BootInfo`), which the kernel
keeps from its first turn (`sys/boot.rs`); xpute installs the clock itself as
that turn begins. Whether a frame can be taken now is the GPU host's answer, a
function (`xpute_gpu::admit` `Admit`): on a page every frame (`every`), on a
device while its host admits one. Where to report what stopped the kernel is
pixelet's (`sys/stop.rs` `LastWords`); the bracket hands both with the entry.
Without a bracket, the kernel runs under `cargo test`. The mechanism under each
is xpute's, booted with pixelet's numbers (mem.json's), so a bracket holds those
numbers, the kernel as the entry and what is pixelet's own.

- `pxl-wasm` makes the kernel a WebAssembly module, linked and exported as
  xpute's module form does one (`xpute_guest::wasm` `link`, `wasm_abi!`), which
  the page runs over a reserved memory, playing its recorded GPU stream on
  WebGPU (`ts/app/web/src/guest.ts`).
- `pxl-native` links the kernel into the device app as a guest of xpute's native
  host (`xpute/rust/host`), which reserves its memory, runs each call on the
  kernel's own thread, keeps its blocks in its memory and exports its entry and
  memory as `xpute_intr` and `xpute_mem` (`xpute_host::native` `native_abi!`).
  Beside it xpute's GPU host holds the device, binds a view's window to a
  surface and lets it go, and calls what asks a turn when a completion lands,
  exported as `xpute_gpu_open`, `xpute_gpu_surface_bind`,
  `xpute_gpu_surface_unbind` and `xpute_gpu_wake_on`
  (`xpute_gpu::host::native` `native_abi!`). pixelet's part is its numbers,
  its last words, what a view hands over (an iPhone's layer, an Android
  window) and the stream played on wgpu into a view's surface, whose host
  admits the frames (`rust/client-bracket/native/src/surface.rs`). The app
  reaches the library as `PxlNative`, Swift's module and Kotlin's object
  alike, through one C header (`include/pxl_native.h`), Android's under the
  names JNI looks for (`src/android.rs`). The kernel's thread is for
  React Native's sake: its JavaScript thread has a 1 MiB stack, which a turn
  that draws through wgpu outgrows, and the app calls in from more than one
  thread.

The brackets are named for the guest's form, xpute's word (`wasm`, `native`),
and the apps for the host's platform, React Native's (`web`, `native`). Native
means the same on both: the device's own machine and system rather than a
browser's sandbox, so the web app carries the wasm guest and the native app the
native one.

Each app hands the client, once, a bracket of its own, what its platform gives:
the guest, the GPU, storage, text, the network and the reads it runs at once,
the sensors, its own long frames and the surfaces (`ts/client/src/bracket.tsx`
`Bracket`, made in each app's `src/bracket.ts`). Neither app names the other,
and neither app's TypeScript reaches the client but through that contract, its
entry mounting the shell aside, so none names the kernel's numbers: what a place
needs of the client is handed it there (the guest's plan, a surface's and the
input's drivers, where to tell of a GPU and of being online), and what it hears
it says in the platform's words (`SensorHeard`, `GpuHeard`, a pointer's phase
and kind), which the client takes in: `ts/client/src/abi/` numbers them for the
kernel and keeps the GPU's state (`abi/gpu.ts`), and `namespace/online.ts` keeps
the network's, which its reads decide by.

**Pixelet's host** is its common host side, `ts/client/src/host/`, plus the
client's bracket an app hands it (`ts/app/*/src/bracket.ts`), built on
`@xpute/host`. `ts/client/src/host/` is the same TypeScript over either bracket.
It mounts xpute's host on the guest the bracket opened (`host/mount.ts`
`open_host`, `@xpute/host/host.ts` `mount`), one name, `HOST`, through which the
guest and its doorbell are reached (`HOST.guest`, `HOST.doorbell`): the
doorbell, whose every turn runs the kernel's calls as it ends, the system calls
answered through xpute's `Sys`, and the strobe that rings the kernel each frame
it grants. What a frame submits, what an open, a write or a program call means,
and what the stop report says are handed in by the client's boot
(`ts/client/src/abi/boot.ts`); `ts/client/src/host/` names none of pixelet's
numbers: the Plan it mounts with is built in `abi/boot.ts` from
`abi/mem.spec.ts`. The kernel's generated vocabulary, the commands' senders, the
snapshot and the boot are in `ts/client/src/abi/`; what answers the kernel's
opens and writes is in `ts/client/src/namespace/`.

The **shell** (`ts/client/src/shell/`) is the UI: React over the snapshot the
kernel writes, where every control is a command. It hands the kernel the
surfaces to draw into, each named by a handle. It computes no coordinate, tile
or draw; what it needs of the model is generated from the same JSON as the Rust.
The host, `abi/`, `io/` and the shell are one package, `ts/client`. Which
technology draws each piece of the shell on each platform is in
[ts/client/src/shell/ui/README.md](./ts/client/src/shell/ui/README.md).

## The host's end

This section is how the kernel's calls to the outside are made and answered.

The kernel records its calls during a turn, and the host runs them after it
returns, so each is done before the next turn begins (`sys/io/host.rs`). Only
what the bracket hands over is called directly: the clock, because a turn
measures its own budget, and the allocator's reading (`xpute_guest::entry`
`BootInfo`), whether a frame can be taken now (`xpute_gpu::admit`), and where to
report what stopped the kernel (`door.rs` `intr`'s last words). An answer comes
back as a number, read once by `answered` into a count or one of xpute's `Errno`
members. A code no member names is the host breaking the protocol.

What the host tells of itself comes with its commands: whether the build is a
development one and which sensors it has in `WORLD_BOOT`, and the wall clock
with each `WORLD_FRAME`.

## Reads, and the one scheduler

This section is how a read is named, answered and paced.

A read is a file in xpute's sense
([xpute's system calls and files](./xpute/ARCHITECTURE.md#system-calls-and-files)):
the kernel names a root by its number and an object below it by its number, the
path's one name, and the client binds each root to what each number is
(`ts/client/src/namespace/roots.ts`), `@xpute/host` carrying the open and the
write there. The binds are one table, bound once at boot and sealed
(`ts/client/src/namespace/roots.ts` `ROOTS`). The roots are
`spec/client/kernel/sys/io/host.json` `root`'s:

| Root               | The kernel's path                                              | What the client binds it to                                                                                  |
| ------------------ | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| each bake, by kind | a tile's number, its subject, in decimal; the Earth's `SPHERE` | the edge's `/tile/<kind>/<version>/<name>`, under the version the declaration gives it (`namespace/tile.ts`) |
| `ASSET`            | a load's number, in decimal                                    | the shipped asset that number is, by its hash (`namespace/asset.ts`)                                         |
| `STORAGE`          | a kept slot's number, in decimal (`kept`)                      | the platform's storage, under a key it picks (`namespace/storage.ts`)                                        |

So the kernel names no URL, no version, no storage key and no platform: a path
is a number. Where a file lies, what key or address it has, which version serves
a tile, and what to do with a file the kernel refuses are the client's. The
client reads the source's declaration once: it hands the kernel the top zoom,
each kind's format and the shards (`WORLD_SOURCE`), and keeps the versions,
which it shows in the settings too. A kept file starts with its slot's `format`,
which the kernel checks as a driver checks a superblock, and a file of another
format is refused and never written over; the client sets it aside and plugs in
an empty slot (`ts/client/src/namespace/storage.ts`).

Only the client knows a file's address and protocol (`sys/io/file.rs`). The type
says which of three answers came back:

| `Opened`              | What it means                        | The descriptor |
| --------------------- | ------------------------------------ | -------------- |
| `Bytes(size)`         | the file is there, `size` bytes long | the owner's    |
| `Absent`              | no such file                         | closed         |
| `Failed { errnum … }` | anything else                        | closed         |

Each descriptor has an owner, and an answer reaches only that owner.

The world's reads fetch tiles by their bake and subject (`sys/io/fetch.rs`
`start`). An absent tile is an answer with no body, and its owner decides what
it means: no ground tile means the parent's ground stands. An empty read is an
error (`EBADMSG`). A body is a heap block, which the command that decodes it
releases unless a store keeps it; terrain's and the Earth's, whose packets run
just past a mebibyte, are slots of their longest packet's pages, seven to a run
the heap gives and takes back once it is empty, rather than the buddy's block of
twice that (`sys/io/body.rs`). The slots out at once, on a credit or kept, are
at most `sys/io/param.rs` `BODY_SLOTS`, a moving eye's typical number. Terrain
keeps a packet only while a window wants one of its squares, and keeps at most
`BODY_SLOTS` less the most credits granted and the Earth's four frames
(`domain/layer/terrain/tile.rs` `most_kept`), so a body on a credit finds a slot
even while no frame is drawn to give kept packets back; a body past the slots is
refused, and its want waits for one given back.

How many reads are out at once is bounded by **credits**, which each frame
carries, as many as the place's network runs at once
(`ts/client/src/bracket.tsx` `origin`'s `credits`: a browser's six, five through
Android's OkHttp), and the kernel takes at most 16 of (`sys/io/param.rs`
`MAX_CREDITS`). One **request scheduler** spends them (`sys/io/request.rs`). It
promises that a camera that has just moved can always state its demand, however
much old work is still draining.

That follows from its shape. A producer states only what it wants, each with a
rank, replacing last turn's list. It never starts, stops or retries an attempt.
Wants and running attempts are two tables of fixed size. An answer ends its
want; a failure ends only its attempt. An object that failed is not asked again
while it is wanted, and a body refused room waits until what refused it, the
heap or the body slots, gives some back. A network or server failure holds every
start, longer with each server failure in a row, and after `RETRY_ATTEMPTS` (6)
of them the reads are given up. A running attempt is never preempted, and stops
only when nobody wants it.

A **serve policy** ranks the wants (`sys/io/policy.rs`). Terrain comes first,
then the assets, then places and the Earth; divisions, and any want asked last
(`Order::LAST`), come after everything. Within a layer, seen comes before unseen
and nearer before farther; among equally near tiles the finer comes first, so
the leaf under the eye is not asked after the coarse tiles over it. The policy
changes how soon the view is right, never what it is.

Models and textures spend the same credits (`sys/io/loader.rs`). Each is fetched
and parsed at most once, retried if it did not arrive and final if it does not
parse. An image's file is read into the heap and handed to the renderer, which
copies it into a texture through the GPU stream (`COPY_EXTERNAL_IMAGE`, the host
decoding it); once the texture lands the file goes, and a device lost reads it
again (`images_lost`).

The settings, but for those a session holds alone (`kept` false: the camera
mode, the places' markers), and the placed objects are kept on the device, each
in a slot the kernel opens by its number (`host.json` `kept`, `sys/io/file.rs`
`Kept`). A kept file begins with its slot's format byte; the kernel refuses one
of another format, as a driver refuses a superblock it does not know, and names
it in the snapshot. A kept file the kernel cannot read is never written over.
Which storage key a slot is, and what becomes of a refused file, are the
client's: it sets the file aside under a key of its own, empties the slot and
says so (`WORLD_KEPT_EMPTIED`), and carries over what was kept before the slots,
once (`ts/client/src/namespace/storage.ts`). Text is asked of the platform a
line at a time, widths then pixels (`sys/io/text.rs`).

## When something goes wrong

There are two kinds of trouble, and they never mix.

A run that does not succeed has **ended**, either aborted or failed with an
errno, a detail and the source location (`sys/io/failure.rs` `Ended`). A failed
fetch is asked again later, and a failed parse is final. Neither touches what is
drawn.

A broken invariant is a **bug**, and it ends the module. `bug!` reports the
errno and place to the host (`sys/stop.rs` `bug`). On a device the process dies
with the kernel, so its last words say why, as numbers. In a page the host
outlives the kernel, and the shell shows a blue screen with the errno and place
as its stop code (`ts/client/src/shell/halt.tsx` `BlueScreen`). It does the same
when the GPU is lost for good or reads are given up.

Memory pressure is neither. Must-fit state is sized from a bound the structure
proves. A bounded flow is full by its owner's policy, and the flows at their
worst together are checked at compile time against the heap a phone is
guaranteed (`door/layout.rs` `HEAP_FLOWS_BYTES`): a body is at most its kind's
packet (pxl-spec `bake::tile::max_packet_bytes`), and one longer is refused
before the heap is asked. A workspace asks first, and waits when refused.

## The turn

The turn and its quota are xpute's: the host rings, the guest spends what it was
given, and the falling edge says when to ring again
([the turn](./xpute/ARCHITECTURE.md#the-turn)). Pixelet decides what fills a
turn.

```text
                         the rising edge
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│ the frame task                                                 │
│                                                                │
│   effects  each compares the world with what it last saw:      │
│            navigation, the bootstrap, the orientation, the     │
│            following, the destination, decor's models, the     │
│            object store, an edit, the hover, the puck, the     │
│            settings                                            │
│   tick     interaction → visible → content → cosmetic → report │
│   pump     start the reads this turn's credits bought          │
│   stage    the frame's uniforms and its draw list              │
│   profile  what the turn cost                                  │
│                                                                │
│ two tasks beside it, and one a load file being read, polled    │
│ on a share kept past the frame                                 │
│                                                                │
│   fetch · the object store · arrival                           │
└────────────────────────────────────────────────────────────────┘
                                │
                                ▼
                        the falling edge  ──▶  wake_ms
```

Before each turn the strobe grants a frame, one `WORLD_FRAME` is submitted
(`ts/client/src/abi/input.ts` `submit_frame`): the viewport, the display's
pacing, whether the turn is interactive, how many reads are allowed out, and the
wall clock. A turn rung for a command alone carries none, and draws nothing. The
quota's values are the kernel's (`sys/sched/tick.rs` `QUANTUM`). The kernel's
boot writes them, and where the rings are, into the kernel's **directory**,
which the host reads at boot (`host/mount.ts`, `@xpute/host/host.ts` `mount`).
The tasks keep a share of the paced frame past the frame task, whatever it
overran (`sys/sched/edge.rs` `tasks_after_frame`).

The phases are xpute's, in the order that is their priority:

| Phase         | What pixelet runs in it                                                                                                                   |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `interaction` | the controller, input, the camera, the environment, the companion, the character's preview                                                |
| `visible`     | demand, the scene's pass, handed the camera's focus, which lays the world gizmo first; `content_share` of the budget is held back from it |
| `content`     | nothing; the share held back from `visible` goes unspent here, and the tasks keep one of their own past the frame                         |
| `cosmetic`    | nothing yet                                                                                                                               |
| `report`      | nothing; the profiler records the turn after the stage (`turn.rs` `world_frame`)                                                          |

The frame is a task, so the order above is its own and not the executor's.

Once a turn, before the layers observe, the kernel says what moved (`turn.rs`
`changes_step`): the tiles whose answer moved, and the regions the object store
wrote. A tile's identity fixes its content, so a list of identities is all a
reader needs.

## The channel and the snapshot

This section is what crosses between the host and the kernel, and how the shell
reads the kernel.

The host and the kernel share xpute's two rings of small frames with page-sized
payload slots. One carries what the host submits. The other carries what the
kernel completes, and holds all one turn leaves; a turn short of room leaves the
rest of the submissions for the next.

The **commands** are listed in `spec/client/kernel/domain/control/cmd.json`,
from which both sides' readers and writers are generated, and the generator
refuses a number given twice (pxl-tool `spec/cmd.rs`). A packet is an XTP
tree, xpute's relocatable binary tree, and anything larger than a slot crosses
in a heap block. The kernel
also posts **signals** nobody asked for: a sensor to watch or let go
(`SENSOR_START`, `SENSOR_STOP`) and a new snapshot (`WORLD_SNAPSHOT`). Which
sensors to watch is the kernel's: it watches one only while the page is seen and
the world view is up, as the shell tells it (`WORLD_VISIBILITY`, `WORLD_VIEW`),
and hears a watch that ended in failure as `SENSOR_FAILED`, the location card's
press as `INPUT_LOCATE`. A read to open and its other calls go in its host
stream (`sys/io/host.rs`).

The shell reads the kernel through one **snapshot**, replaced whole after a turn
only when what it is made of moved (`shell.rs` `Shown`). It carries only what
the kernel owns:

- the focus, whether the view shows ground there yet, and whether it has
  wandered from the device or past the service area;
- the state of the device's location and heading, the camera's azimuth, whether
  it follows the device, and whether it is dragged or zooming;
- the companion's readiness, the tool held and a marker waiting for its words,
  the display cases, and the settings by their numbers once their slot is read;
- the meters a pixel spans at the focus, the divisions it is in, whether the
  pointer is over a marker, the kept slots it refused, and a server's stall.

What is rounded or timed for presentation, such as a scale bar's length or a
toast, is the shell's.

## The memory map

This section is where every range of the client's state lives.

The client's state is xpute's
[core memory](./xpute/ARCHITECTURE.md#the-core-memory-placed-once), placed once
and never moved. In a tab it is a `WebAssembly.Memory` of `DESKTOP_BYTES`, or
`PHONE_BYTES` where the pointer is coarse, since a phone's JavaScriptCore counts
the whole reservation against the tab (`mem.json`, `ts/app/web/src/device.ts`
`handheld`). On a device it is always one region of `PHONE_BYTES` the library
takes, and the kernel's data is the process's, so its ranges begin at
`GLOBAL_BASE` (`pxl-native lib.rs`). Every address is an offset from the first
byte, so the map holds on both.

```text
address      range               bytes                  placed by
──────────────────────────────────────────────────────────────────────
0            .stack              1 MiB                  wasm-ld
1 MiB        .ring.desc          12 KiB + 20 KiB        mem.json
1.5 MiB      .ring.slot          2 MiB + 4 MiB          mem.json
8 MiB−4KiB   the directory       4 KiB                  mem.json
8 MiB        .rodata .data .bss  ~2.5 MiB               wasm-ld
D            .fixed              each owner's range     door/layout.rs
D + .fixed   .heap               the rest               the allocator
end          PHONE_BYTES or DESKTOP_BYTES
──────────────────────────────────────────────────────────────────────
             D: __data_end rounded up to a page, about 10.5 MiB;
                GLOBAL_BASE on a device
             sizes as mem.json sets them
```

The map is `pxl-kernel sys/mem/section.rs`'s. `.stack` is first so an overflow
wraps below zero and traps, the only guard without an MMU. The rings and the
directory sit at addresses from `spec/client/kernel/sys/mem/mem.json`, which
also tells the linker where its data may begin.

`.fixed` holds what the kernel places once, sizes at compile time and never
frees, and only what is mostly in use whenever it runs or is the host's
reservation: the terrain's room, the resident divisions' cells, the renderer's
live uploads, the GPU and host streams. Each owner states its size, and the door
lays the ranges in order and hands each its own at boot (`door/layout.rs`
`FIXED_BYTES`). The terrain's and the divisions' ranges are read through a
`Section`, which checks every access against its end; the others are handed
their first byte, which the renderer and the streams write from.

`.heap` is everything else, under xpute's slab over buddy, including what
another format sizes, such as a glTF or a PNG. A refused request is a wait, not
a failure: the asker tries again on a later turn, the mirror's chunk once the
heap gives a page back, its square filled again. What is held only at times, or
far below its worst most of the time, is a heap flow with a cap, grown as it is
used and refused past the cap, each counted at its worst, in the buddy's blocks,
against the heap a phone is guaranteed (`door/layout.rs` `HEAP_FLOWS_BYTES`):
the bodies on their way, the CPU's mirror of the ways' pool and the squares'
records read back (`domain/layer/terrain/mirror.rs`,
`pxl-render gpu/ray/ground.rs` `GroundBuffers`), and the profiler's rings while
its setting is on.

What the renderer reads each frame is the kernel's **draw list**, records of
uploads, rewrites, draws and frees (`pxl-render frame/draw.rs`), its uploads'
and frees' lanes in `.fixed.uploads`. What an upload names is a heap block laid
out in lanes, structure-of-arrays: a model's positions, normals, UVs, joints and
weights each its own. The JavaScript heap holds only glue.

## Settings and the profiler

This section covers the settings, which sit under the view
(`domain/view/setting/`), the lowest part that reads them, and are read by every
part above it, and the profiler, which reads anything.

Every setting is one entry of one schema
(`spec/client/kernel/domain/view/setting/schema.json`). The store keeps only the
user's overrides, by number: each setting and each option has one in the schema,
given out once, and the kept file is a format byte and `(number, value)` pairs
(`domain/view/setting/store.rs`). An entry marked `dev` is offered only by a
development build.

The **profiler** (`profiler/`) is reached by production only where it records.
It keeps what each frame and task cost in rings laid out by
`spec/client/kernel/profiler/profiler.json`, which the client reads in place and
decodes (`ts/client/src/abi/profiler.ts`), and the shell prints as a report,
beside the place's own long frames (`ts/client/src/bracket.tsx` `longFrame`), or
exports as a Chrome trace. Nothing schedules by what it records.

---

# Making the data

Everything the view reads of the world is a published version of baked tiles,
made from what the database held and the code that baked it. This part follows
the data down one chain, each stage reading only the one before it.

```text
provider release ──ingest──▶ db-geo (+ catalog) ──leaf bake──▶ z15 leaves
                                                                   │
                                                          fold, no DB
                                                                   │
client residency ◀── bucket (Worker) ◀── publish ◀── version (z15 … top)
```

## A shard is the unit

Every stage names its work by shards, never by a hand-drawn box. A **shard** is
a z8 tile of the pyramid, named by its Morton key, and the world is ingested,
baked and grown a shard at a time. The coverage is the set of shards the bakes
publish (`infra/geo/coverage.json`).

A shard's **leaves** are its z15 tiles, the ones baked from data, and the shard
holds everything they read. What a leaf reads past its edge stays within one
**reach**, a z13 tile (`pxl-geo-bake coverage.rs` `BAKE_REACH`). The reach plus
a ground cell for rounding is the shard's **apron**. How the databases hold
shards is [infra/geo/README.md](./infra/geo/README.md).

## The database

The geo database holds the providers' data, and its catalog records which shards
are in at which release. Three ingests fill it: the Overture types, a table
each; the elevation, one raster; and the coast, whole. The Overture ingest reads
the shards of the coverage, so it is a binary of pxl-geo-bake
(`bin/ingest_overture.rs`); the other two are scripts under `infra/geo/` and
compile nothing. An Overture feature is one row however many shards it meets.

A run adds the shards it is given; a whole run fills a new table and swaps it in
([infra/README.md](./infra/README.md) §2). The operator decides what to run. The
elevation is warped onto one grid of blocks, so a block two aprons share is
stored once. What the database holds, per table its release and counts plus the
elevation, is its **holdings**. A bake starts only when every shard it needs is
held.

## A leaf

A leaf is baked from its shard's rows alone (`terrain/leaf.rs` `bake_leaf`).
Each cell gets a **look**, the class of ground it is drawn as.

Rows become features by Carto's mapping of their OpenStreetMap tags, for the
classes the paint policy consumes (`overture/policy.rs` `LAND`). They are
painted in Carto's order, ties broken by the source's id
(`terrain/pipeline/feature.rs` `Rank`). A cell takes what covers its center as
seen from above, so a tunnel is left off
([the ground is a Carto palette](./doc/minutes/20261005-the-ground-is-a-carto-palette.md)).

The coastline is OSM's land polygons, and the sea is wherever they do not reach.
A leaf is baked only where land, a building or a way reaches it.

Bridges and tunnels are solved from a tile and as much of its 1-ring as its
structures reach, and never further, starting from Arnis's rules
(`separation/solve.rs`, the bridges `solve/bridge.rs`, the tunnels
`solve/tunnel.rs`;
[as Arnis solves them](./doc/minutes/20261006-bridges-and-tunnels-as-arnis-solves-them.md)).
A height leans on nothing further than a tile, so neighbouring tiles agree where
they meet; Arnis is the reference, the ring the bound. A deck is a line in the
terrain tile, not an object
([ways off the ground](./doc/minutes/archive/20261001-ways-off-the-ground.md)).

Polygons are filled even-odd and lines with a thickness kernel. Elevation is
signed 16-bit cell units under one Mercator Z scale per tile, so the bake, the
terrain and the object layer agree on a unit of height. A tile is written as
XTP, xpute's tree, in the form the client reads.

The elevation is sampled at the leaf's resolution, its pad included. The apron
reaches past every pad, so a zero there is the source's sea, never a missing
shard. One interpolant reads the cells' heights and the separation's footing off
the samples (`pxl-spec-core ground.rs` `bicubic`). The bake reads the footing
and the GPU the cells, which the CPU reads back.

## The pyramid

Every tile above the leaves is folded from the four under it, with no database
in the loop (`pxl-spec bake/fold.rs` `fold_terrain`). The ground folds by
majority, one rule with the client's (`ground.rs` `own`), and the elevation by
the average of the samples. The place layer keeps what shows from the parent's
zoom on.

A leaf with nothing drawn is never baked, and a missing tile is drawn from the
one over it, so open ocean costs nothing. A missing tile folds as sea inside the
coverage and as unknown past it, where the planet's picture shows.

A bake folds again only what its shards change, zoom by zoom, until a zoom
changes nothing (`pyramid.rs` `fold`).

## A version

A bake keeps one **set** per kind: every zoom of the pyramid flat under the
tiles' names, with a manifest of its holdings. It changes the set a shard at a
time (`tile/shard.rs`):

1. The named shards' leaves are staged beside the set.
2. Only what they change is folded again.
3. The staged tiles are moved in once folded.

Each step can stop and run again, and a change of source, code or coverage is a
purge by command. A **version** is a set uploaded under a name of its format and
the day, immutable once written (`spec/spec/bake/published.json`). A pruned
layer leaves out the tiles no reader asks for.

## The dev tile server

In development the same bake runs on demand, every layer the same way
(`demand/pyramid.rs`). A leaf is baked when first asked for, and a tile above is
folded at most `--folds` zooms over the leaves. The cache lasts until a purge.
With `--shelf` the server serves each kind's set, so a bake is seen before it is
published. The make targets are named like a bakery's bread:

| Target      | Tiles                         |
| ----------- | ----------------------------- |
| `dev-oven`  | baked on demand               |
| `dev-shelf` | the last bakes                |
| `dev`       | from the published deployment |

## Publishing

A version reaches the client through one private bucket that only the Worker
reads ([deployment hosting](./doc/minutes/20260816-deployment-hosting.md)). The
client asks for the published version's tiles by name.

## Catalogs and assets

Everything a build ships is decided at build time, as JSON under `spec/`, from
the commands and catalogs to the settings schema and the published versions.
`pxl-tool` writes a Rust module from each file, and a TypeScript one where the
shell reads it; `check-spec` fails when the tree disagrees.

Which assets exist is compiled on both sides: the world's rule, which the CPU,
the GPU, the bakes and the stores agree on. Which file each references is the
one catalog the host plugs in, read through the asset root like any file
(`pxl-spec asset/catalog.rs`). Where bytes cross, as a tile or a saved blob, the
layout is the model's, and the version is in a tile's URL, which the client
picks from the declaration, or a kept file's format byte.

- **Cubes and decors** are numbers in `spec/spec/asset/`, and the object store
  writes the number.
- **A file reference** is a path under `asset/content/` in the spec that makes
  it, compiled as a slot: the kernel knows "the cumulus' skin", never which file
  it is. The catalog fills each slot with a file's number below the asset root
  and what the ingest measured of it, and until it is read every slot reads as
  loading. Its count bounds the files the kernel loads at once, so no memory
  waits on the catalog.
- **Asset keys** come from the table `pxl-tool`'s ingest writes
  (`spec/spec/asset/ingested.json`), which numbers each file, the host binding a
  number to its key. Each file is keyed by the SHA-256 of its bytes, so moving
  it changes no key.
- **The texture pack** is one table of which image a cube wears on which face,
  and one WebP image, the file the cube kind's definition names
  (`spec/spec/asset/cube.json` `pack`), which the ingest composes and ships as a
  row of the table like any other. A textured look is drawn from afar as its
  tile's mean, so the ingest draws each top toward its look's color
  (`pxl-tool asset/pack.rs` `TOWARD_CARTO`).

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
| Minecraft Beta's `replaceBlocksForBiome` ([doc/minutes/archive/20261011-the-ground-under-its-face.md](./doc/minutes/archive/20261011-the-ground-under-its-face.md)) | `pxl-spec-core ground.rs` `Cell::block`                                                                                             | A column under its face: the look on top, a filler of soil, sand or water, sandstone under sand, stone under all                         |
| Minecraft's map colors                                                                                                                                              | `spec/spec/asset/color.json` underground                                                                                            | The colors of soil, sandstone and stone                                                                                                  |
| Quake III's `PM_StepSlideMove` ([bg_slidemove.c](https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_slidemove.c))                              | `pxl-kernel domain/query/support.rs` `step_slide`                                                                                   | Blocked, step up and slide along the wall as `PM_ClipVelocity` clips; ours has no step height but the ceiling's and settles on the floor |
| Amanatides and Woo's voxel traversal                                                                                                                                | `pxl-kernel domain/query/ray.rs`, `domain/query/support.rs`                                                                         | The next cell along a ray, by the share of the way to each axis's edge                                                                   |
| OpenStreetMap Carto                                                                                                                                                 | `pxl-geo-bake terrain/pipeline/overture/policy.rs`, `spec/spec/asset/`                                                              | The ground's palette and paint order, way widths, place icons, colors and zooms                                                          |
| Mapnik's `text_line.cpp`, CSS 2.1 `line-height: normal`                                                                                                             | `pxl-render look/marker.rs`                                                                                                         | A marker's line pitch                                                                                                                    |
| OpenMapTiles' schema                                                                                                                                                | `spec/spec/asset/marker.json`                                                                                                       | A marker shows from z14, as its `poi` layer does                                                                                         |
| [Arnis](https://github.com/louis-e/arnis)                                                                                                                           | `pxl-geo-bake separation/solve.rs`, `solve/`                                                                                        | Bridges and tunnels: clearances, ramps, decks, pillars                                                                                   |
| osm2streets, osm2lanes                                                                                                                                              | the bake's way rules ([doc/minutes/archive/20261001-ways-off-the-ground.md](./doc/minutes/archive/20261001-ways-off-the-ground.md)) | Lane and buffer widths                                                                                                                   |
| Overture's schema                                                                                                                                                   | `pxl-spec division.rs`, `pxl-geo-bake terrain/pipeline/overture/between.rs`                                                         | Places and divisions read as the schema defines them                                                                                     |
| Bruneton; Hillaire (EGSR 2020)                                                                                                                                      | `pxl-render look/atmosphere.rs`                                                                                                     | The atmosphere's tables: transmittance and irradiance; multiple scattering and the sky view                                              |
| Unreal's SkyAtmosphere, OPAC, Cornette–Shanks                                                                                                                       | `pxl-render look/atmosphere.rs`, `frame/shader/atmos_medium.wgsl`                                                                   | The Earth's scattering numbers, the haze, the Mie phase                                                                                  |
| ACES as Stephen Hill fits it                                                                                                                                        | `pxl-render frame/shader/finish.wgsl`                                                                                               | One tone curve for every pass                                                                                                            |
| N8AO                                                                                                                                                                | `pxl-render gpu/ao.rs`                                                                                                              | Screen-space ambient occlusion, ported to WGSL                                                                                           |
| Kasten and Young; Ballesteros                                                                                                                                       | `pxl-render frame/shader/star.wgsl`, `gpu/star.rs`                                                                                  | Airmass at the horizon; a star's color from B−V                                                                                          |
| Duffett-Smith                                                                                                                                                       | `pxl-kernel domain/scene/environment/celestial_position.rs`                                                                         | The sun's, the moon's and the stars' positions                                                                                           |
| Stöckli et al. (Blue Marble)                                                                                                                                        | `pxl-geo-bake bin/bake_earth.rs`                                                                                                    | The imagery's bytes read back as reflectance                                                                                             |
| three.js                                                                                                                                                            | `pxl-kernel domain/scene/actor.rs`, `pxl-render frame/shader/atmos_view.wgsl`                                                       | A clip's cross-fade; Lambert as its standard material                                                                                    |
| Howard Hinnant's date algorithm                                                                                                                                     | `pxl-geo-bake util.rs`                                                                                                              | A civil date from a day count                                                                                                            |
| Figma                                                                                                                                                               | [doc/minutes/20260912-why-the-destination-is-wasm.md](./doc/minutes/20260912-why-the-destination-is-wasm.md)                        | A compiled engine in the page over one memory                                                                                            |
| iOS's interface guidelines, Material's motion, SF Symbols                                                                                                           | `ts/client/src/shell/ui/theme.ts`, `shell/ui/motion/transition.ts`, `shell/ui/icon.tsx`                                             | Touch targets and type scale, easing, the icons' names                                                                                   |
