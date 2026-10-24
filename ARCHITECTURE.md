# Architecture — pixelet

pixelet draws the real Earth as blocks you can build on, in a browser tab or a device app, and what it draws is what is walked on. Both follow from one function. A pixel's ray, a walker's step, the pointer and an edit all ask the same question of a **node**, a cube of space at a zoom: what fills it, at the zoom its distance from the eye asks. One function answers (`pxl-spec-core world.rs` `node`). It lays the ground's grid, each cell a block at its height with the caves carved under its face, and then writes the ways off the ground and what players placed over it.

No reader asks block by block. It descends (`pxl-spec-core cast.rs` `first`), passing whole a node in which nothing fills and splitting only one that may hold something. The function and the descent are one source compiled for the CPU and, through rust-gpu, for the GPU, so the block a pixel shows and the block a foot stands on are one answer.

How finely the world is drawn is the other half, and it is one rule too: the view is a grid of angular cells about the eye, and a block is drawn at the zoom whose block spans at least a cell at its distance (`pxl-spec-core grid.rs` `cell_at`). What a zoom folds away is not there at that zoom, to draw, to walk on or to miss, and nothing finer is fetched to keep it.

A kind of thing is drawn wholly on one side. The GPU's rays find the world's blocks, and the GPU grows the decor the ground grows, each where it is drawn. Everything else the kernel decides to draw, it lists: what a player placed, the actors, the puck, the ghosts, the badges. The renderer draws the list and decides nothing but color. Control is headless: the walk, the pointer and the camera's clearance ask the same query on the CPU and draw nothing.

pixelet is an application over [xpute](./xpute/ARCHITECTURE.md): a Rust kernel that runs as xpute's guest, the TypeScript around it, and Rust servers and bakes that share the kernel's model. The memory, the rings and the turn are xpute's and described there; this document is what pixelet builds on them.

The kernel is one order of dependency, a DAG rather than a pipeline: an edge may skip a step but never points up, and a reader reaches what it reads directly. The eye asks what it can tell apart, and the question goes down through the scene, the cubes about the eye, the view, the query and the layers to the reads that bring a tile in. Each answer comes back up the same way, and nothing names what is above it. The figure is a map; each name in it is defined where its part begins.

```text
  turn · door · shell   wire everything; nothing below names them
  ──────────────────────────────────────────────────────────────
  control ──▶ view      moves the eye; the turn hands on the rest

  asks                                                     answers
    │   scene  what this frame draws             scene/       ▲
    │   chunk  a cube about the eye, by address  chunk.rs     │
    │   view   the pose, the lens, demand        view/        │
    │   query  the only seam                     query.rs     │
    │   layer  the base and the object           layer/       │
    ▼   io     reads, credits, the host's end    io/          │

  leaves   settings, models, decor, billboards, under their readers
  beside   the profiler: reads anything; no decision reads it
```

The parts follow that order. **The world** is what the kernel carries: sizes, positions, layers and what fills a block. **The eye** goes from the eye down to the layers. **Drawing** is the last step up, from the eye's rays and the kernel's list to a frame. **Below the view** is what all of it stands on: the reads, the turn, the memory. **Making the data** is a chain of its own, run ahead of the view, that ends where the view reads.

---

# The world

## Every size is a zoom

Every size in the world is one number, its zoom. The world is 2^24 units round, and a thing whose side is the world's over 2^z is at zoom z: one zoom finer is half as wide (`pxl-spec space/coord/zoom.rs`).

| Zoom | Side   | What it sizes                                                                  |
| ---- | ------ | ------------------------------------------------------------------------------ |
| z27  | `1/8u` | a placed object's finest cell                                                  |
| z24  | `1u`   | a ground cell, the unit a position names                                       |
| z15  | `512u` | a leaf tile: the finest terrain is fetched and stored at                       |
| top  |        | the coarsest tile, where the pyramid stops (`pxl-spec-core lib.rs` `TOP_ZOOM`) |

The words for what is sized:

- A **cell** is the granule at a zoom, possibly empty. A **voxel** is a cell carrying a material.
- A **tile** is what is fetched and stored as one. A layer's tile has the same number of cells a side at every zoom (`pxl-spec space/layer/lattice.rs`). Terrain's is 512, so its z24 cells make z15 tiles.
- The **pyramid** is the tree of all tiles: a tile at z covers the 2 × 2 under it at z + 1, and its Morton key is a prefix of theirs (`space/hierarchy/morton.rs`).
- A **chunk** is a cube of cells about the eye, which gathers what stands in the world.
- A **block** is a thing placed, never a size.

A tile's zoom is a web map's, and a z24 cell is a web map's z16 pixel; only the 3857 adapter knows it (`adapter/3857.rs` `cell_zoom_of`).

## A position

A position is a **`Q3`**: three `i32` counting 256ths of a unit, so it is the same bits on every machine (`pxl-spec space/coord/q3.rs`). Above the fraction, an axis holds 24 bits across, 24 up the map and 16 in height (`space/coord/world.rs` `EXTENT`). The shell reads coordinates only through the kernel.

X wraps as its `i32` does, once round the world, and so does the short way between two positions (`world.rs` `between`). Y and Z are held at their ends as a position enters the world. A displacement is never a position.

Web Mercator (EPSG:3857) gives the unit its meaning, about 2.389 meters at the equator. Its plane leaves out the poles (`adapter/4326.rs` `CLAMP_LAT`), and the pyramid never leaves it. The planet around it is a sphere, which Drawing covers.

## The layers

The world is stored as **layers**. A layer is a payload and a one-zoom fold: a tile at z is folded from the tiles at z + 1 under it, by the layer's own rule (`layer.rs` `LATTICE`). Which tiles are fetched is never a layer's choice.

The layers split on what changes. The **base** is baked from the sources: terrain, place, division and earth. A base tile is fixed by its name within a version. The **object** layer is what people author, and changes under its keys. The line is not dimension or motion.

| Side   | Layer    | Stores                                                    | Shape                         | Folds by                          |
| ------ | -------- | --------------------------------------------------------- | ----------------------------- | --------------------------------- |
| base   | terrain  | the ground's look and height, and its separation          | a raster tile, 2.5D           | majority (look), average (height) |
| base   | place    | the places Carto shows at the tile's zoom, each from when | a raster tile, an icon a cell | what shows from the parent's zoom |
| base   | division | the finest division each cell is in, with its ancestors   | a raster tile, from z12       | majority                          |
| base   | earth    | the planet's picture                                      | a raster tile, top zoom only  | —                                 |
| object | object   | what players place                                        | a sparse octree a shard       | majority                          |

**Terrain** is the ground. A tile holds ground cells, their elevation, and its **separation**: the ways at another grade, such as bridges and tunnels, each a line with a top and a width (`pxl-spec bake/separation.rs`). A tile also says which of its children exist, so it is read as soon as it lands.

A ground cell is one byte, a **look** from OpenStreetMap Carto, numbered in the order Carto paints (`spec/spec/block/cube.json`). Each look is drawn as land, water, transport or a structure. `0` is void and `255` the unknown past the coverage, and neither gives a surface.

Heights are made on the GPU and read back, never sent (`pxl-spec-core ground.rs` `height`). Every rounding in pixelet and xpute is half to even, and check:rules refuses any other.

**Objects** are client-local. Their store is one sparse tree, each leaf a catalog value at its node and each branch its eight children folded (`pxl-kernel layer/object/store.rs`). A write splits leaves only on its path, joins eight equal leaves into one, and folds its path again. So one picture has one tree. A leaf of `AIR` erases what a lower layer would show.

A leaf may carry an **extension**, such as a marker's words, kept in a pool per **class** of ids (`pxl-spec block/ext.rs` `class_of`). It is placed and erased with its leaf, which is never joined or split. The store persists in a kept slot whose format byte is its layout's version (`host.json` `format`).

**Places** are tiles named like the ground tiles under them, each holding what Carto draws there at that web map zoom (`pxl-spec bake/tile/place.rs`). What Carto draws for each Overture category is one table from its rules (`spec/spec/bake/place_theme.json`). A cell one icon wide keeps one place, so each place carries the zoom it shows from, and a parent is its children with what shows from its zoom on (doc/minutes/20261009-places-follow-carto.md). Places are a setting.

The **earth** layer is the planet's picture, baked at its top zoom only (`pxl-spec bake/tile/earth.rs`).

## What fills a block

What stands in one cell is a block. It has two independent axes:

- Its **kind** is how it is drawn: a cube, a decor model, or a marker facing the viewer. An id is its kind over its shape (`spec/spec/block/id.json` `SHAPE_BITS`).
- Its **source** is which side holds it: the base, whose ground grows it, or the object layer.

So decor is no layer. A grown tree and a placed tree are one kind from two sources, arriving as one record (`layer/instance.rs` `Instance`).

| Kind   | Base                                | Object          |
| ------ | ----------------------------------- | --------------- |
| cube   | the ground's look, a cell's surface | a placed cube   |
| decor  | what the ground grows               | a placed decor  |
| marker | a place, Carto's icon and its name  | a placed marker |

The ground grows decor by evaluating, never storing: what stands in a leaf cell is a function of the cell, a seed and the ground's face (`query/scatter.rs` `grown_at`). Two grown decors never share a cell. Models, seeds, densities and colors are catalog entries compiled into both sides (Catalogs and assets).

---

A block coarser than a leaf cell is what its zoom holds. The ground's coarse cell stands at its folded height, its look the majority of those it folds ([doc/minutes/20261005-the-ground-is-a-carto-palette.md](./doc/minutes/20261005-the-ground-is-a-carto-palette.md)). Its top block is filled to the share its height reaches (`ground.rs` `top`). What is placed and the ways fold to the kind most of their fill is (`overwrite.rs` `fold`).

A coarse block's fill is how much of its cube the ground and what is written over it cover (`answer.rs` `cover`). It keeps that fill in eighths, floored: its **kept eighths**. It is drawn as its cube shrunk about its center by the cube root of that, so its volume is its fill (`cast.rs` `kept_bounds`).

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

- Nothing is drawn where a block keeps none, and the block under a shrunk top is ground no cave takes.
- A ray stops where it enters the box and passes it where it misses. The eye's, the sun's and the pointer's rays do alike (`cast.rs` `entry`).
- A shrunk block closes no edge of a block beside it.
- The ray carries the kept eighths, so the pass that colors the screen draws the box (`ray.rs` `KEPT_SHIFT`).
- A leaf's block keeps its cube whole, so nothing walked or edited at the leaves changes.

So flat ground far away reads as separate cubes rather than a sheet: every axis is treated alike. What is folded is a summary, so nothing in a folded cell is struck, hidden, cut by a preview or pointed at (`control/interaction.rs` `Picked`).

---

# The eye

This part is how the view decides what to hold and draw: only what an observer can tell apart. A direction answers with the first thing it meets, and how far that is chooses how finely it is drawn. So the view's work is bounded by the screen, not the world: a city or two hundred towers behind a ridge cost what the screen they cover costs.

## The camera

One **camera** looks, and every controller moves it (`control/camera/controller.rs` `Camera`). It is an **eye** and a **look**, in spherical coordinates about the world's Z. θ is the polar angle from straight down and φ the azimuth from north toward east. Nothing else keeps a copy.

- The free camera is first person: it turns about its eye and walks it.
- The orbit is third person: it turns about a point a distance ahead of the eye along the look, and that distance is the only state it adds.

The point a camera turns about is the **focus**: the eye itself in first person, the point ahead in the orbit. It is derived, never kept, and it is where the **companion**, the character, stands. A change of person moves the camera about the focus, never the focus.

The **anchor** is the eye's longitude and latitude, which the sky is seen from (`view/frame/pose.rs` `anchor`). Generic code reads the eye, the look and the focus, never a controller's own words.

The orbit's controls are written only through one funnel (`control/camera/orbit.rs` `ask`). There terrain may ask whether a state puts the eye inside the ground (`control/camera/orbit/terrain_gate.rs`). The camera setting answers: through ignores terrain, stop takes a control as far as it can, and climb raises the drawn pitch instead.

A pan, the companion and the orbit's climb move by one rule (`query/support.rs` `step_slide`). Where the camera grid draws the ground coarser than its own cells, the world is a map, and the focus goes where it is moved. Where the grid draws the leaves, the focus walks them a cell at a time, a step-slide after Quake III's. It climbs what blocks it while its cell is open overhead, and slides along an axis a ceiling stops. With no jump, only the ceiling bounds a climb.

A walker meets its cells as they fold at its zoom, so what folds away is not met. It stops at the edge of a leaf not yet held. Until its footing is known, it is drawn on the finest ground known there.

The **pose** is the eye and the turn from the world's axes to the camera's. The world stays in its own axes, in doubles; what turns is the observer. Every draw uploads its point less the eye, subtracted in doubles before it is narrowed.

The pointer is apart from the camera: the tool held, what is under it and what a tap does belong to `control/interaction.rs`.

## The camera grid

The camera grid sets how finely each part of the view is drawn. Each of its angular cells is the least the observer promises to tell apart: a solid angle about the eye, the same wherever it lies. A screen only calibrates it, to about twelve drawn pixels at the optical axis. Inside a cell nothing is promised, as long as nothing leaks into a neighbouring cell beyond what the area measure allows.

That gives the one rule of detail: what is drawn is what the eye's rays reach. A ray goes down the pyramid and stops at the first block that fills at the zoom its distance asks. That block is what its pixels show; nothing no ray stops on is drawn. The GPU casts one ray a drawn pixel (Drawing).

Height is not special: the distance is to the block met, every axis alike. The rule is not yet closed by area: a flat top seen grazing is drawn finer than it shows (doc/backlog.md, P1).

**τ** is one factor on the distance each zoom splits at (`pxl-spec space/camera/grid.rs` `Tau`), and the ground's tile zoom for a distance follows from it (`space/layer/lattice.rs` `z_at`). τ = 1 is the finest cell, stated at the finest device pixel ratio, so what is sized at τ = 1 holds at every ratio. τ is the screen's alone (`view/lens.rs`).

## The query, the only seam

The query is the only place a layer's shape is known. Every layer answers one question of a node, a tile or a cube, at a cell zoom the asker states (`layer/query.rs` `Query`).

- Its **bound** is nothing known, empty, or inside a conservative box.
- A descent carries a **cursor** from a node to its children, so each step is answered from its parent and never again from the top.
- Each **answer** says what it lacked: exact, coarser with the tile it needed, or nothing and that tile (`layer/answer.rs` `Answer`).

The query fetches and keeps nothing, so reading costs what is asked, not what a tile holds.

The **world** composes the layers into one answer (`query/world.rs` `World`). What is in a block is one rule, for drawing and for control alike (`pxl-spec-core world.rs` `node`):

1. the ground's grid, each cell a block shifted up to its height;
2. the caves' air under its face, from a hash pyramid asked only at the blocks a reader asks for (`pxl-spec procedural.rs`);
3. the ways off the ground, where each crossing of a line over the column is a deck and its pier, a bore, an embankment or a cutting (`pxl-spec-core ways.rs`);
4. then what is placed, air included.

A ray asked of the world reads that rule at the zoom the eye's rays draw there (`query/ray.rs` `first_hit`), so what control meets is what is drawn. A walker casts the same ray up or down its cell's columns, at the finest zoom placed there, so it stands on what is placed (`query/support.rs`).

The world ends in bedrock, laid as a bowl: a bottom under the ground, and a wall just inside the **service area**, the shards the source serves. Walking and editing stay inside it, and an edit goes only over known ground, between the bedrock and a ceiling (`query/limit.rs` `World::editable`).

A ray descends the pyramid ([a ray asks the world whether a node is empty](./doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md)). A node coarser than the zoom its distance asks is not what is drawn there. So it is asked only whether anything under it fills at the zooms the ray asks; what those zooms fold away is not there to miss.

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

One query answers both (`query/node.rs` `World::node`), and each source answers from what it keeps:

- the ground, by the bound the GPU made over its held cells, or where a cave may take it;
- the ways, by a bound of their own beside the ground's;
- the caves, by their hash node, hollow and drawing nothing again under it;
- what is placed, by nothing held under the node in the objects' tree.

The pointer's ray also strikes decor against its model, but only where the rays draw decor as it is, so what is struck is what is drawn (`query/decor.rs`).

The pointer, the camera's clearance, the walk and the view's observations all ask the world; none reads a layer. The descent is one function (`pxl-spec-core cast.rs`), which the GPU's cast compiles (Drawing). The CPU asks it one ray at a time, for control, never for the screen.

## Windows and squares

The GPU holds the ground's cells too, but only where a ray can reach them. A ray observes a zoom's cells only within a fixed reach of the eye (`pxl-spec-core window.rs`). So a **window** about the eye holds every cell it can observe at that zoom.

A window is a few **squares** of cells a side, each held in a slot its place names. It follows its centre a square at a time, and no held cell moves. The eye has one at every zoom drawn, the focus two, and a destination being gone to one (`layer/terrain/window.rs`).

A square is filled on the GPU as its tile lands. Compute passes make each cell's **column**, its ground's top and the runs its ways write over it along z (`heights_cs`, `ways_cs`). They fold a bound over the columns into **levels** (`bound_cs`). Both are read back, so the CPU's query and the cast meet the same bridge or tunnel.

Where a square is not present, both read the coarser window's. A square that enters asks its tile again, and the browser's cache answers.

## Demand and residency

Demand is what the view asks for, and residency what it keeps. **Demand** is what an observation requires, the ground at the resolution its distance chooses, before it is any tile; the layer's storage maps it to tiles (`view/demand.rs` `observe`).

Demand is a function of the eye alone: at each zoom, every tile within the band of distance that zoom is drawn at (`view/observation.rs` `ByEye`). It never depends on what the rays met. A tile it requires that is not held is a miss. Every layer is asked this way, the place layer too, and holds what it is asked for in a residency of its own.

The **floor** is a law: every observation has an answer. It is kept by holding the coarsest zoom the source supplies whole, all around the eye (`layer/residency/floor.rs`).

**Residency** holds a tile hard for the floor, for demand while it is demanded, or as cache; only the hard set is correctness. Its capacity is asserted on its **frames**, the slots tiles are held in (`Residency::FRAMES`): 76 for terrain, 649 for places, 9 for divisions and 4 for the Earth. A tile that arrives takes a free frame, or else the farthest one nothing holds hard and the view does not read. A region goes from unresolved to coarse to as fine as it is observed, and never back.

The view is loading until something has reached the screen and every tile demanded shows, exact or coarser (`layer/residency.rs` `Residency::shown`). A version is never rewritten, so a tile never changes under its key; what changes is which tile answers a node. **Nothing is prefetched**: a view converges to exact on its misses alone.

## The eye's cubes

What stands beside the ground, decor and the marks, is gathered a chunk at a time. A chunk is a cube of the camera grid's cells 32 a side, addressed by zoom and x, y and z (`chunk.rs`). It is never fetched or held; only its address exists.

**The eye's cubes** are one partition of the ground (`view/observation.rs` `cubes`). A square of ground splits into four while its nearest point is nearer than its zoom's split distance, as a ray's descent does. Each square left holds the cubes the world stands in there. The partition never depends on what a ray met.

Decor is grown and drawn on the GPU each frame, a thread a candidate, never stored (`pxl-render gpu/grown.rs`). Placed decor is handed in beside it as records (`scene/decor/drawn.rs`). A kind is drawn as a model down to its own size on screen, and as a sprite below it.

## Marks

**Marks** are the places and markers, gathered over the eye's cubes. Each stands at a cell's center, at the height of the surface drawn there.

A place is the place layer's mark (`scene/place.rs`). Each cube in view reads its place tiles and shows what Carto shows at the zoom drawn there: a dot or an icon, named from its kind's label zoom (`billboard/badge.rs`). A place stands on what a ray down its cell meets first, and where nothing is drawn there is no place. A tap on one is the place's before the ground's.

---

## Each concern's bound

Taken together, the view is six concerns between the world and the screen, each with its own bound:

| Concern    | Asks                                          | Bounded by                                   | Where                                              |
| ---------- | --------------------------------------------- | -------------------------------------------- | -------------------------------------------------- |
| Storage    | a layer's records, in the layer's own shape   | nothing of the view                          | a layer; the bucket                                |
| Query      | what a node holds, a point, or first on a ray | —; it holds nothing                          | `query.rs`                                         |
| Drawing    | what each pixel's ray meets                   | the screen's rays                            | `scene/cast.rs`, `pxl-render gpu/renderer.rs`      |
| Demand     | which records an observation requires         | 2 × 2 ground tiles a zoom, 8 × 8 place tiles | `pxl-spec space/layer/demand.rs`, `view/demand.rs` |
| Residency  | which records are held, and why               | its frames, asserted at compile time         | `layer/residency.rs`                               |
| Scheduling | which waiting work runs now                   | the turn's quota and the credits             | `io/request.rs`, `io/policy.rs`                    |

Every number a person would tune, such as τ's target, the screen's cells and decor's tiers, is named once in `pxl-kernel param.rs`, grouped by the policy it tunes, with its reason.

---

# Drawing

This part turns the eye's rays and the kernel's list into a frame: the GPU draws the world from the rays, the kernel lists everything else, and the renderer executes both and decides nothing.

## A ray a pixel

Each drawn pixel is one ray on the GPU. It walks the same descent the CPU runs, over the ground's windows, the caves, the ways and the objects' tree as the GPU holds them. The block it stops on is the pixel's ([the GPU casts a ray a pixel](./doc/minutes/20261014-the-gpu-casts-a-ray-a-pixel.md)). Once a frame the kernel hands the GPU the eye and the camera grid's layout (`view/lens.rs` `gpu_cast`), so every pass reads one grid.

Neighbouring rays share most of their descent, so the cast walks that part once, in four steps (`pxl-shader lib.rs` `beam_cs`; [a cell casts a beam](./doc/minutes/20261022-a-cell-casts-a-beam.md)):

1. A **beam ray** at each corner of a camera grid cell walks the descent the cell's pixels share, and stops before a node too narrow to share.
2. Each cell starts at the least of the beam rays about it.
3. **Samples**, four a cell a side, walk on to the blocks they stop at.
4. A pixel takes the nearest sample block about it that its own ray enters, and walks itself only where none is.

A face thinner than the samples' spacing can be missed, which [the 20261024 minute](./doc/minutes/20261024-a-proxy-only-starts-the-ray.md) leaves open.

What the rays asked of each node is kept between frames in the **observed pyramid** (`pxl-spec-core observed.rs`), forgotten only over squares that moved. A pixel needing a node it lacks walks the world again in the same pass.

A ray leaves four words (`pxl-spec-core ray.rs`): the block it met, with its face, zoom and kept eighths; how far; the looks a coarse block folds; and which of the face's edges and corners meet the air or a rising block.

The **screen pass** reads those words (`frame/shader/screen_rays.wgsl` `pixel_ray`) and paints the block with the one function every lit face uses (`frame/shader/paint.wgsl` `paint`): its swatch or image, shaded edges, light, and the air before it. Its depth is set back by a share of the face's slope across a pixel, so what lies on a face, like the location puck, wins with no bias of its own.

The one mesh left is the **ghost**, the box an edit is about to place (`pxl-render mesh/ghost.rs`). The CPU draws nothing of the world; it runs the query for control and decides what the GPU is handed.

## The render resolution

The render resolution sets how many pixels the cast draws (`pxl-kernel view/frame/resolution.rs`). It is the display's device pixels per CSS pixel, capped, times a scale the `resolution` setting picks: full, one device pixel a CSS pixel, or, by default, auto.

Auto judges moving frames by their GPU time against the paced interval. It steps down at once when over budget, and up only after a run of windows the higher step would fit; without GPU timing it counts missed frames instead. The camera grid is laid by the display's ratio, not the drawn one, so a block's zoom and what is fetched never follow the load.

## Light on the blocks

Three settings shade a block beyond its light (`spec/client/kernel/setting/schema.json`):

- **Edges** draw a line along every crease, an edge meeting the air or a rising block. It has one width in units at every block size, so only perspective thins it.
- **Corners** either darken a face along each edge and corner a block rises across, tinted warm (`screen_rays.wgsl` `creases`), or run N8AO over the rays' depth and the grown decor (`gpu/ao.rs`).
- **Shadows** read the sun's map, below.

The **sun's map** is a square box of the light about the eye (`gpu/shadow.rs`). Its texels are fixed in the world, each in a slot by its index modulo the map's side (`pxl-spec-core sun.rs` `Sun`), so the map follows the eye a texel at a time, and a texel that stays is not cast again.

A slot's ray runs along the sun through the query, again only where the box moved onto it, the eye asks its stop at another zoom, or a node it passes changed. A step of the sun casts the whole map. Decor and the actors are drawn into it each frame.

A receiver compares nine taps, each moved by its own plane's slope (`shadow.wgsl` `sun_lit`). Casters are pushed by the same formula, so a plane does not shadow itself.

## The frame

Each turn ends with a **draw list** (`pxl-render frame/draw.rs`): entries tagged by the pipeline that draws them, each with its block and uniform. It holds decor (instanced, or as an **impostor** sprite where a model is drawn too small to be worth it), the actors, the location puck, an edit's ghosts, the badges and the debug lines. The rays are not in it; they are the cast.

The kernel writes the frame's uniforms into one lane of doubles (`frame/stage.rs`) and calls the renderer (`gpu/renderer.rs` `render_frame`) with the list, the models, the badges' images, the earth's tiles, the windows' squares and the cast. It draws every open surface, the world and the character picker's display case, and reports the cost and whether it wants another turn.

A frame runs in this order:

```text
ahead       impostor sheets, fresh tiles, the atmosphere's tables,
            the air's volume
cast        the screen's rays; the sun's map; the occlusion prepass
world pass  grown decor → rays at their depth → grown impostors
            → sky and stars at the far plane → listed actors, decor,
            cumulus, impostors → high cloud planes → puck, ghosts
over        badges → debug lines → display cases
```

The calls are WebGPU's, written through xpute's encoder (`gpu/api.rs`) into one stream in the kernel's memory. The page runs it on WebGPU after the turn returns (`apps/web/src/webgpu.ts`); the device app runs it on wgpu inside its turn, on the kernel's thread (`crates/client-bracket/native/src/surface.rs`). Every GPU object is an xpute handle, and a test links no GPU.

A frame stands where nothing it reads moved. The rays are cast again only when the eye, the windows, the shards or the overwrites moved, or a square came in fresh; decor alone casts no ray. Where everything else also reads as before, nothing is submitted and the canvas keeps its frame.

So a still view draws nothing, and a clock steps the sky by a display level and the clouds by a pixel at the nearest cloud. A frame too large for the stream is dropped whole, and the next turn draws again.

Shaders and pipelines are the kernel's (`frame/shader/`), and the tests compile each shader with naga and check its pipeline against it. What a shader shares with the host has no padding, and a uniform block is read a field at a time, never rebuilt as a struct; a phone's Vulkan and Safari's WebGPU each broke on one. A fact the GPU needs has one home, such as the frame uniform's cell angle.

Depth is reversed float, near at 1, so far coplanar faces do not fight. On device loss the blocks are still in memory and are uploaded again from the same addresses.

## Past the pyramid: the sky and the sphere

The pyramid is planar and leaves the poles out, but the sky and the ground past the terrain must be drawn too. A thing becomes a layer only if there is something to hold:

- **Stored data** has extent and resolutions, so it is a layer, refined by the eye's rule. The earth's picture is one.
- **An evaluated model** is computed per pixel from a few parameters: the sun, moon and stars from the clock and the anchor, the scattering from its tables, the clouds from their field.

The earth layer is reflectance with city lights in alpha, a ground cell a texel (`pxl-spec bake/tile/earth.rs`). Only the floor's tiles are held (`layer/earth.rs` `earth_window`), so it adds no demand. Where none is held, a global picture eight times coarser answers by latitude and longitude.

The sky is drawn at the far plane, so only pixels no ray met run it (`frame/shader/sky.wgsl`). Where such a ray falls to the land, it shows flat land past the terrain, shaded from the earth layer and lit as the terrain is, so the edge is no seam (`atmos_air.wgsl` `beyond`). The planet is only the air's medium, a centre and a radius.

The air between the eye and what is drawn comes from the **air's volume**, Hillaire's aerial-perspective volume (`gpu/atmosphere.rs` `froxel`). A compute pass marches a grid of screen columns through froxels, which cover only the air each ray crosses, spaced by depth cubed so the near view gets most. A drawn point mixes the four columns about it, each read at the point's own distance (`atmos_air.wgsl` `air_before`).

The **clouds** are three layers on one wind (`pxl-render look/cloud.rs`). The cumulus is models drawn like decor but claiming no cell, so a tower stands through it (`scene/environment/cloud.rs`). Each sits by a hash in a lattice of its own, a model, a sprite or nothing by its size on screen.

Above it, altocumulus and cirrus are planes read where a ray meets them (`frame/shader/clouds.wgsl` `sheet_t`), laid over the world without depth. From below they fill only the sky and from above they cover everything, so a peak above one is knowingly wrong. Every layer fades out as the eye climbs toward where the clouds are gone.

## Badges and the sheet

A badge stands at a cell's center and is drawn flat to the camera; its records come from the view (Marks).

Badges are one instanced quad (`billboard/field.rs`) over one texture packed shelf by shelf, the **badge sheet** (`pxl-render gpu/sheet/shelf.rs`). The pass (`frame/shader/badge.wgsl`) tests several points of each badge against the world's depth and drops the badge when most are hidden. It writes depth from an upright twin of the quad, so badges sort against the world along its up. Chat bubbles share the pass.

A sheet entry is drawn on the GPU only when what it shows changes (`gpu/sheet.rs`): its shapes as signed distances, its text as lines the platform drew. Asking the platform a line at a time leaves shaping, bidirectional text and emoji to each platform's own engine ([text by the line](./doc/minutes/20260926-the-sheet-on-the-gpu-and-text-by-the-line.md)).

Markers people put up are an object kind drawn by the same pass, whether or not places are on. Where several share a cell, the cell shows their count, counted afresh each time the cube is read. Unlike places, markers are one kind, so their number means something. All markers share one text until each keeps its own ([doc/backlog.md](./doc/backlog.md)).

---

## Debug views

Five debug settings show what the view does, drawn from what the frame already holds.

- `view`: where the camera and the pointer are, in a panel.
- `profiler`: the profiler's panel (Below the view).
- `world_gizmo`: the focus, its cell and rings about it (`scene/indicator/frame_gizmo.rs`), and over the ground the eye's windows' squares by how they stand, with their tiles and shards (`coverage_gizmo.rs`).
- `camera_gizmo`: the camera grid over the screen, rings a cell apart cut into cells of one solid angle (`frame/shader/camera_grid.wgsl`), and each pixel's block tinted by the share of a cell its face shows the eye (`screen_lit.wgsl` `cell_share`).
- `ray_steps`, in a development build: each pixel coloured by the nodes its own ray walked, grey where it took a sample's block.

---

# Below the view

This part is what the view stands on: how what it asks for arrives, what it costs in time and memory, and how a failure leaves what is drawn alone. None of it names anything above it: a layer asks io for a tile, and io never asks what the tile is for.

## The kernel, its host, its brackets and the client

This section says where the kernel runs and what plugs it in.

The kernel is the process: it holds the world and decides everything about it, but owns no device. Like any guest of xpute, it asks its **host** for the rest. The host grants it one memory once and a share of time on each call, and runs what the call recorded after it returns. One such call is a **turn**. That mechanism is xpute's and names nothing of pixelet ([xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md)).

A **bracket** plugs the kernel into a platform: it hands over the memory, the clock, the allocator's reading, whether a frame can be taken now, and where to report what stopped the kernel (`door.rs` `Bracket`). Without one, the kernel runs under `cargo test`.

- `pxl-wasm` makes the kernel a WebAssembly module, which the page runs over a reserved memory, playing its recorded GPU stream on WebGPU (`apps/web/src/guest.ts`).
- `pxl-native` links the kernel into the device app, runs each call on its own thread, and plays the stream on wgpu into a view's surface (`crates/client-bracket/native/src/surface.rs`).

Each app's TypeScript hands the client, once, what its platform gives: the guest, the GPU, storage, text, the network, the sensors and the surfaces (`apps/client/src/bracket.tsx` `Bracket`). Neither app names the other.

**Pixelet's end** of the kernel (`apps/client/src/kernel/`) is the same TypeScript over either bracket. It wires xpute's doorbell over the bracket (`mem.ts`), submits each frame and rings the kernel (`sched.ts`), forwards input as commands, answers the kernel's system calls (`host.ts`), and does the reads the kernel asks for into blocks the kernel chose.

The **shell** (`apps/client/src/shell/`) is the UI: React over the snapshot the kernel writes, where every control is a command. It hands the kernel the surfaces to draw into, each named by a handle. It computes no coordinate, tile or draw; what it needs of the model is generated from the same JSON as the Rust. Pixelet's end and the shell are one package, `apps/client`. Which technology draws each piece of the shell on each platform is in [apps/client/src/shell/ui/README.md](./apps/client/src/shell/ui/README.md).

## The host's end

This section is how the kernel's calls to the outside are made and answered.

The kernel records its calls during a turn, and the host runs them after it returns, so each is done before the next turn begins (`io/host.rs`). Only the bracket's own functions are called directly: the clock, because a turn measures its own budget, the allocator's reading, whether a frame can be taken now, and where to report what stopped it (`door.rs` `Bracket`). An answer comes back as a number, read once by `answered` into a count or one of xpute's `Errno` members. A code no member names is the host breaking the protocol.

What the host tells of itself comes with its commands: whether the build is a development one and which sensors it has in `WORLD_BOOT`, and the wall clock with each `WORLD_FRAME`.

## Reads, and the one scheduler

This section is how a read is named, answered and paced.

A read is a file in xpute's sense, reached as Plan 9 reaches one ([xpute's system calls and files](./xpute/ARCHITECTURE.md#system-calls-and-files)): the kernel names a root by its number and an object below it by its number, the path's one name, and pixelet's end of the kernel binds each root to what each number is, carried there by xpute's host (`apps/client/src/kernel/boot.ts`, `bind_root`). The roots are `spec/client/kernel/abi/host.json` `root`'s:

| Root               | The kernel's path                                              | What pixelet's end binds it to                                                                                 |
| ------------------ | -------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| each bake, by kind | a tile's number, its subject, in decimal; the Earth's `SPHERE` | the edge's `/tile/<kind>/<version>/<name>`, under the version the declaration gives it (`kernel/io/object.ts`) |
| `ASSET`            | a load's number, in decimal                                    | the shipped asset that number is, by its hash                                                                  |
| `STORAGE`          | a kept slot's number, in decimal (`kept`)                      | the platform's storage, under a key it picks                                                                   |

So the kernel names no URL, no version, no storage key and no platform: a path is a number. Where a file lies, what key or address it has, which version serves a tile, and what to do with a file the kernel refuses are pixelet's end's. The kernel reads the source's declaration for its zooms, shards and formats; its versions are read in pixelet's end alone, which shows them in the settings too. A kept file starts with its slot's `format`, which the kernel checks as a driver checks a superblock, and a file of another format is refused and never written over; the shell sets it aside and plugs in an empty slot (`apps/client/src/kernel/io/kept.ts`).

Only pixelet's end knows a file's address and protocol (`io/file.rs`). The type says which of three answers came back:

| `Opened`             | What it means                        | The descriptor |
| -------------------- | ------------------------------------ | -------------- |
| `Bytes(size)`        | the file is there, `size` bytes long | the owner's    |
| `Absent`             | no such file                         | closed         |
| `Failed { errno … }` | anything else                        | closed         |

Each descriptor has an owner, and an answer reaches only that owner.

The world's reads fetch tiles by their bake and subject (`io/fetch.rs` `start`). An absent tile is an answer with no body, and its owner decides what it means: no ground tile means the parent's ground stands. An empty read is an error (`EBADMSG`). A body is a heap block, which the command that decodes it releases unless a store keeps it; terrain's and the Earth's, whose packets run just past a mebibyte, are slots of their longest packet's pages, seven to a run the heap gives, rather than the buddy's block of twice that (`io/body.rs`).

How many reads are out at once is bounded by **credits**, which pixelet's end sends with each frame, a constant 6 (`sched.ts` `NET_CREDITS`), and the kernel takes at most 16 of (`param::MAX_CREDITS`). One **request scheduler** spends them (`io/request.rs`). It promises that a camera that has just moved can always state its demand, however much old work is still draining.

That follows from its shape. A producer states only what it wants, each with a rank, replacing last turn's list. It never starts, stops or retries an attempt. Wants and running attempts are two tables of fixed size. An answer ends its want; a failure ends only its attempt. An object that failed is not asked again while it is wanted, and a body the heap refused room waits until the heap gives some back. A network or server failure holds every start, longer with each server failure in a row, and after `RETRY_ATTEMPTS` (6) of them the reads are given up. A running attempt is never preempted, and stops only when nobody wants it.

A **serve policy** ranks the wants (`io/policy.rs`). Terrain comes first, then the assets, then places and the Earth; divisions, and any want asked last (`Order::LAST`), come after everything. Within a layer, seen comes before unseen and nearer before farther; among equally near tiles the finer comes first, so the leaf under the eye is not asked after the coarse tiles over it. The policy changes how soon the view is right, never what it is.

Models and textures spend the same credits (`io/load.rs`). Each is fetched and parsed at most once, retried if it did not arrive and final if it does not parse. An image's file is read into the heap and handed to the renderer, which copies it into a texture through the GPU stream (`COPY_EXTERNAL_IMAGE`, the host decoding it); once the texture lands the file goes, and a device lost reads it again (`images_lost`).

The settings and the placed objects are kept on the device, each in a slot the kernel opens by its number (`host.json` `kept`, `io/file.rs` `Kept`). A kept file begins with its slot's format byte; the kernel refuses one of another format, as a driver refuses a superblock it does not know, and names it in the snapshot. A kept file the kernel cannot read is never written over. Which storage key a slot is, and what becomes of a refused file, are the host's: it sets the file aside under a key of its own, empties the slot and says so (`WORLD_KEPT_EMPTIED`), and carries over what was kept before the slots, once (`apps/client/src/kernel/io/kept.ts`). Text is asked of the platform a line at a time, widths then pixels (`io/text.rs`).

## When something goes wrong

There are two kinds of trouble, and they never mix.

A run that does not succeed has **ended**, either aborted or failed with an errno, a detail and the source location (`io/failure.rs` `Ended`). A failed fetch is asked again later, and a failed parse is final. Neither touches what is drawn.

A broken invariant is a **bug**, and it ends the module. `bug!` reports the errno and place to the host (`host_bug`). On a device the process dies with the kernel, so its last words say why, as numbers. In a page the host outlives the kernel, and the shell shows a blue screen with the errno and place as its stop code (`apps/client/src/shell/halt.tsx` `BlueScreen`). It does the same when the GPU is lost for good or reads are given up.

Memory pressure is neither. Must-fit state is sized from a bound the structure proves. A bounded flow is full by its owner's policy, and the flows at their worst together are checked at compile time against the heap a phone is guaranteed (`door/layout.rs` `HEAP_FLOWS_BYTES`): a body is at most its kind's packet (pxl-spec `bake::tile::max_packet_bytes`), and one longer is refused before the heap is asked. A workspace asks first, and waits when refused.

## The turn

The turn and its quota are xpute's: the host rings, the guest spends what it was given, and the falling edge says when to ring again ([the turn](./xpute/ARCHITECTURE.md#the-turn)). Pixelet decides what fills a turn.

```text
                         the rising edge
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│ the frame task                                                 │
│                                                                │
│   effects  each compares the world with what it last saw:      │
│            navigation, the bootstrap, the orientation, the     │
│            following, the destination, decor's assets, the     │
│            object store, an edit, the hover, the puck, the     │
│            settings                                            │
│   tick     interaction → visible → content → cosmetic → report │
│   pump     start the reads this turn's credits bought          │
│   stage    the frame's uniforms and its draw list              │
│   profile  what the turn cost                                  │
│                                                                │
│ three tasks beside it, and one a load file being read, polled  │
│ on a share kept past the frame                                 │
│                                                                │
│   fetch · decor assets · the object store · arrival            │
└────────────────────────────────────────────────────────────────┘
                                │
                                ▼
                        the falling edge  ──▶  wake_ms
```

Before each turn the strobe grants a frame, pixelet's end submits one `WORLD_FRAME` (`apps/client/src/kernel/sched.ts`): the viewport, the display's pacing, whether the turn is interactive, how many reads are allowed out, and the wall clock. A turn rung for a command alone carries none, and draws nothing. The quota's values are the kernel's (`sched/tick.rs` `QUANTUM`). The kernel's boot writes them, and where the rings are, into the kernel's **directory**, which the host reads at boot (`mem.ts`, `sched.ts`). The tasks keep a share of the paced frame past the frame task, whatever it overran (`sched/edge.rs` `tasks_after_frame`).

The phases are xpute's, in the order that is their priority:

| Phase         | What pixelet runs in it                                                                                                                   |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `interaction` | the controller, input, the camera, the environment, the companion, the character's preview                                                |
| `visible`     | demand, the scene's pass, handed the camera's focus, which lays the world gizmo first; `content_share` of the budget is held back from it |
| `content`     | nothing; the share held back from `visible` goes unspent here, and the tasks keep one of their own past the frame                         |
| `cosmetic`    | nothing yet                                                                                                                               |
| `report`      | nothing; the profiler records the turn after the stage (`turn.rs` `world_frame`)                                                          |

The frame is a task, so the order above is its own and not the executor's.

Once a turn, before the layers observe, the kernel says what moved (`turn.rs` `changes_step`): the tiles whose answer moved, and the regions the object store wrote. A tile's identity fixes its content, so a list of identities is all a reader needs.

## The channel and the snapshot

This section is what crosses between the host and the kernel, and how the shell reads the kernel.

The host and the kernel share xpute's two rings of small messages with page-sized payload slots. One carries what pixelet's end submits. The other carries what the kernel completes, and holds all one turn leaves; a turn short of room leaves the rest of the submissions for the next.

The **commands** are listed in `spec/client/kernel/abi/cmd.json`, from which both sides' readers and writers are generated. A packet is an XTP tree, xpute's relocatable binary tree, and anything larger than a slot crosses in a heap block. The kernel also posts **signals** nobody asked for: a sensor to watch or let go (`SENSOR_START`, `SENSOR_STOP`) and a new snapshot (`WORLD_SNAPSHOT`). A read to open and its other calls go in its host stream (`io/host.rs`).

The shell reads the kernel through one **snapshot**, replaced whole after a turn only when what it is made of moved (`shell.rs` `Shown`). It carries only what the kernel owns:

- the focus, whether the view shows ground there yet, and whether it has wandered from the device or past the service area;
- the state of the device's location and heading, the camera's azimuth, whether it follows the device, and whether it is dragged or zooming;
- the companion's readiness, the tool held and a marker waiting for its words, the display cases, and the settings by their numbers;
- the meters a pixel spans at the focus, the divisions it is in, whether the pointer is over a badge, the kept slots it refused, and a server's stall.

What is rounded or timed for presentation, such as a scale bar's length or a toast, is the shell's.

## The memory map

This section is where every range of the client's state lives.

The client's state is xpute's [core memory](./xpute/ARCHITECTURE.md#the-core-memory-placed-once), placed once and never moved. In a tab it is a `WebAssembly.Memory` of `DESKTOP_BYTES`, or `PHONE_BYTES` where the pointer is coarse, since a phone's JavaScriptCore counts the whole reservation against the tab (`mem.json`, `apps/web/src/device.ts` `HANDHELD`). On a device it is always one region of `PHONE_BYTES` the library takes, and the kernel's data is the process's, so its ranges begin at `GLOBAL_BASE` (`pxl-native memory.rs`). Every address is an offset from the first byte, so the map holds on both.

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

The map is `pxl-kernel mem/section.rs`'s. `.stack` is first so an overflow wraps below zero and traps, the only guard without an MMU. The rings and the directory sit at addresses from `spec/client/kernel/mem/mem.json`, which also tells the linker where its data may begin.

`.fixed` holds what the kernel places once, sizes at compile time and never frees: the terrain's room, the resident divisions' cells, the renderer's live blocks, the GPU and host streams, the profiler's rings. Each owner states its size, and the door lays the ranges in order and hands each its own at boot (`door/layout.rs` `FIXED_BYTES`). The terrain's, the divisions' and the profiler's ranges are read through a `Section`, which checks every access against its end; the others are handed their first byte, which the renderer and the streams write from.

`.heap` is everything else, under xpute's slab over buddy, including what another format sizes, such as a glTF or a PNG. A refused request is a wait, not a failure: the asker tries again on a later turn.

What the renderer reads each frame is the kernel's **draw list**, records of uploads, rewrites, draws and frees (`pxl-render frame/draw.rs`), its uploads' and frees' lanes in `.fixed.blocks`. What an upload names is a block laid out in lanes, structure-of-arrays: a model's positions, normals, UVs, joints and weights each its own. The JavaScript heap holds only glue.

## Settings and the profiler

This section covers the settings, which anything reads, and the profiler, which reads anything.

Every setting is one entry of one schema (`spec/client/kernel/setting/schema.json`). The store keeps only the user's overrides, by number: each setting and each option has one in the schema, given out once, a retired one kept in `$retired`, and the kept file is a format byte and `(number, value)` pairs (`setting/store.rs`). An entry marked `dev` is offered only by a development build.

The **profiler** (`profiler/`) is reached by production only where it records. It keeps what each frame and task cost in rings laid out by `spec/client/kernel/profiler/profiler.json`, which the shell reads in place, prints as a report or exports as a Chrome trace. Nothing schedules by what it records.

---

# Making the data

Everything the view reads of the world is a published version of baked tiles, made from what the database held and the code that baked it. This part follows the data down one chain, each stage reading only the one before it.

```text
provider release ──ingest──▶ db-geo (+ catalog) ──leaf bake──▶ z15 leaves
                                                                   │
                                                          fold, no DB
                                                                   │
client residency ◀── bucket (Worker) ◀── publish ◀── version (z15 … top)
```

## A shard is the unit

Every stage names its work by shards, never by a hand-drawn box. A **shard** is a z8 tile of the pyramid, named by its Morton key, and the world is ingested, baked and grown a shard at a time. The coverage is the set of shards the bakes publish (`infra/geo/coverage.json`).

A shard's **leaves** are its z15 tiles, the ones baked from data, and the shard holds everything they read. What a leaf reads past its edge stays within one **reach**, a z13 tile (`pxl-geo-bake coverage.rs` `BAKE_REACH`). The reach plus a ground cell for rounding is the shard's **apron**. How the databases hold shards is [infra/geo/README.md](./infra/geo/README.md).

## The database

The geo database holds the providers' data, and its catalog records which shards are in at which release. Three ingests fill it, none compiling the model: the Overture types, a table each; the elevation, one raster; and the coast, whole (`infra/geo/overture/ingest.rs` and its siblings). An Overture feature is one row however many shards it meets.

A run adds the shards it is given; a whole run fills a new table and swaps it in ([infra/README.md](./infra/README.md) §2). The operator decides what to run. The elevation is warped onto one grid of blocks, so a block two aprons share is stored once. What the database holds, per table its release and counts plus the elevation, is its **holdings**. A bake starts only when every shard it needs is held.

## A leaf

A leaf is baked from its shard's rows alone (`terrain/leaf.rs` `bake_leaf`). Each cell gets a **look**, the class of ground it is drawn as.

Rows become features by Carto's mapping of their OpenStreetMap tags, for the classes the paint policy consumes (`overture/policy.rs` `LAND`). They are painted in Carto's order, ties broken by the source's id (`terrain/pipeline/feature.rs` `Rank`). A cell takes what covers its center as seen from above, so a tunnel is left off ([the ground is a Carto palette](./doc/minutes/20261005-the-ground-is-a-carto-palette.md)).

The coastline is OSM's land polygons, and the sea is wherever they do not reach. A leaf is baked only where land, a building or a way reaches it.

Bridges and tunnels are solved as Arnis solves them, structure by structure, from a tile and the neighbours its structures reach (`separation/arnis.rs`, [as Arnis solves them](./doc/minutes/20261006-bridges-and-tunnels-as-arnis-solves-them.md)). A deck is a line in the terrain tile, not an object ([ways off the ground](./doc/minutes/archive/20261001-ways-off-the-ground.md)).

Polygons are filled even-odd and lines with a thickness kernel. Elevation is signed 16-bit cell units under one Mercator Z scale per tile, so the bake, the terrain and the object layer agree on a unit of height. A tile is written as XTP, xpute's tree, in the form the client reads.

The elevation is sampled at the leaf's resolution, its pad included. The apron reaches past every pad, so a zero there is the source's sea, never a missing shard. One interpolant reads the cells' heights and the separation's footing off the samples (`pxl-spec-core ground.rs` `bicubic`). The bake reads the footing and the GPU the cells, which the CPU reads back.

## The pyramid

Every tile above the leaves is folded from the four under it, with no database in the loop (`pxl-spec bake/fold.rs` `fold_terrain`). The ground folds by majority, one rule with the client's (`ground.rs` `own`), and the elevation by the average of the samples. The place layer keeps what shows from the parent's zoom on.

A leaf with nothing drawn is never baked, and a missing tile is drawn from the one over it, so open ocean costs nothing. A missing tile folds as sea inside the coverage and as unknown past it, where the planet's picture shows.

A bake folds again only what its shards change, zoom by zoom, until a zoom changes nothing (`pyramid.rs` `fold`).

## A version

A bake keeps one **set** per kind: every zoom of the pyramid flat under the tiles' names, with a manifest of its holdings. It changes the set a shard at a time (`tile/shard.rs`):

1. The named shards' leaves are staged beside the set.
2. Only what they change is folded again.
3. The staged tiles are moved in once folded.

Each step can stop and run again, and a change of source, code or coverage is a purge by command. A **version** is a set uploaded under a name of its format and the day, immutable once written (`spec/spec/bake/published.json`). A pruned layer leaves out the tiles no reader asks for.

## The dev tile server

In development the same bake runs on demand, every layer the same way (`demand/pyramid.rs`). A leaf is baked when first asked for, and a tile above is folded at most `--folds` zooms over the leaves. The cache lasts until a purge. With `--shelf` the server serves each kind's set, so a bake is seen before it is published. The tasks are named like a bakery's bread:

| Task        | Tiles                         |
| ----------- | ----------------------------- |
| `dev:oven`  | baked on demand               |
| `dev:shelf` | the last bakes                |
| `dev`       | from the published deployment |

## Publishing

A version reaches the client through one private bucket that only the Worker reads ([deployment hosting](./doc/minutes/20260816-deployment-hosting.md)). The client asks for the published version's tiles by name.

## Catalogs and assets

Everything a build ships is decided at build time, as JSON under `spec/`, from the commands and catalogs to the settings schema and the published versions. `pxl-tool` writes a Rust module from each file, and a TypeScript one where the shell reads it; `check:spec` fails when the tree disagrees.

A catalog is never sent, because both sides compile it. Where bytes cross, as a tile or a saved blob, the layout is the model's, and the version is in a tile's URL, which pixelet's end picks from the declaration, or a kept file's format byte.

- **Cubes and decors** are numbers in `spec/spec/block/`, and the object store writes the number.
- **Models** are named by a path under `asset/content/` in the catalog that uses them.
- **Asset keys** come from the table `pxl-tool`'s ingest writes (`pxl-spec asset/ingested.spec.rs`). Each file is keyed by the SHA-256 of its bytes, so moving it changes no key.
- **The texture pack** is one table of which image a cube wears on which face, and one WebP image. A textured look is drawn from afar as its tile's mean, so the ingest draws each top toward its look's color (`pxl-tool asset/pack.rs` `TOWARD_CARTO`).

# Prior art

What the tree took from work done before it, or found it had arrived at: each line names the precedent, where the tree follows it, and what it takes. A link is given where the tree records the source; the rest are named as the code names them.

| Precedent                                                                                                                                                           | Where                                                                                                                               | What is taken                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Google Earth and the web map's tile pyramid                                                                                                                         | the pyramid (`pxl-spec space/`)                                                                                                     | Structure, storage, tiling and invalidation across zooms                                                                                 |
| Cesium 3D Tiles, `skipLevelOfDetail` and `preloadAncestors`                                                                                                         | `pxl-kernel io/demand.rs`                                                                                                           | A tile tree whose nodes arrive out of order; a held ancestor shown at once                                                               |
| Morton order                                                                                                                                                        | tile and shard names (`pxl-spec space/hierarchy/morton.rs`)                                                                         | One base-4 digit a zoom, coarsest first                                                                                                  |
| Web Mercator (EPSG:3857), WGS 84                                                                                                                                    | `pxl-spec space/coord/`                                                                                                             | The plane a position follows; the sphere's radius                                                                                        |
| Mapbox's globe                                                                                                                                                      | `pxl-spec space/coord/globe.rs`                                                                                                     | The air's planet under the eye: its radius as the map is stretched there                                                                 |
| Earth Engine's `MODE` pyramiding                                                                                                                                    | `pxl-spec bake/fold.rs`                                                                                                             | A 2 × 2 majority fold of the zoom under it                                                                                               |
| Distant Horizons                                                                                                                                                    | `pxl-render mesh/paint/palette.rs`                                                                                                  | A distant cell drawn as the mean of what it folds                                                                                        |
| Minecraft's `cube_all`, `cube_bottom_top`                                                                                                                           | `pxl-render gpu/layers.rs`                                                                                                          | Which face image a cube wears: its own side and bottom where the pack ships them, its top on every face where not                        |
| Minecraft Beta's `replaceBlocksForBiome` ([doc/minutes/archive/20261011-the-ground-under-its-face.md](./doc/minutes/archive/20261011-the-ground-under-its-face.md)) | `pxl-spec-core ground.rs` `Cell::block`                                                                                             | A column under its face: the look on top, a filler of soil, sand or water, sandstone under sand, stone under all                         |
| Minecraft's map colors                                                                                                                                              | `spec/spec/block/color.json` underground                                                                                            | The colors of soil, sandstone and stone                                                                                                  |
| Quake III's `PM_StepSlideMove` ([bg_slidemove.c](https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_slidemove.c))                              | `pxl-kernel query/support.rs` `step_slide`                                                                                          | Blocked, step up and slide along the wall as `PM_ClipVelocity` clips; ours has no step height but the ceiling's and settles on the floor |
| Amanatides and Woo's voxel traversal                                                                                                                                | `pxl-kernel query/ray.rs`, `query/support.rs`                                                                                       | The next cell along a ray, by the share of the way to each axis's edge                                                                   |
| OpenStreetMap Carto                                                                                                                                                 | `pxl-geo-bake terrain/pipeline/overture/policy.rs`, `spec/spec/block/`                                                              | The ground's palette and paint order, way widths, place icons, colors and zooms                                                          |
| Mapnik's `text_line.cpp`, CSS 2.1 `line-height: normal`                                                                                                             | `pxl-render gpu/pass/badges.rs`                                                                                                     | A label's line pitch                                                                                                                     |
| OpenMapTiles' schema                                                                                                                                                | `spec/spec/block/marker.json`                                                                                                       | A marker shows from z14, as its `poi` layer does                                                                                         |
| [Arnis](https://github.com/louis-e/arnis)                                                                                                                           | `pxl-geo-bake separation/arnis.rs`                                                                                                  | Bridges and tunnels solved structure by structure: clearances, ramps, decks, pillars                                                     |
| osm2streets, osm2lanes                                                                                                                                              | the bake's way rules ([doc/minutes/archive/20261001-ways-off-the-ground.md](./doc/minutes/archive/20261001-ways-off-the-ground.md)) | Lane and buffer widths                                                                                                                   |
| Overture's schema                                                                                                                                                   | `pxl-spec division.rs`, `pxl-geo-bake terrain/pipeline/overture/between.rs`                                                         | Places and divisions read as the schema defines them                                                                                     |
| Bruneton; Hillaire (EGSR 2020)                                                                                                                                      | `pxl-render look/atmosphere.rs`                                                                                                     | The atmosphere's tables: transmittance and irradiance; multiple scattering and the sky view                                              |
| Unreal's SkyAtmosphere, OPAC, Cornette–Shanks                                                                                                                       | `pxl-render look/atmosphere.rs`, `frame/shader/atmos_medium.wgsl`                                                                   | The Earth's scattering numbers, the haze, the Mie phase                                                                                  |
| ACES as Stephen Hill fits it                                                                                                                                        | `pxl-render frame/shader/finish.wgsl`                                                                                               | One tone curve for every pass                                                                                                            |
| N8AO                                                                                                                                                                | `pxl-render gpu/ao.rs`                                                                                                              | Screen-space ambient occlusion, ported to WGSL                                                                                           |
| Kasten and Young; Ballesteros                                                                                                                                       | `pxl-render frame/shader/stars.wgsl`, `gpu/stars.rs`                                                                                | Airmass at the horizon; a star's color from B−V                                                                                          |
| Duffett-Smith                                                                                                                                                       | `pxl-kernel scene/environment/celestial_position.rs`                                                                                | The sun's, the moon's and the stars' positions                                                                                           |
| Stöckli et al. (Blue Marble)                                                                                                                                        | `pxl-geo-bake bin/bake_earth.rs`                                                                                                    | The imagery's bytes read back as reflectance                                                                                             |
| three.js                                                                                                                                                            | `pxl-kernel scene/actor.rs`, `pxl-render frame/shader/atmos_view.wgsl`                                                              | A clip's cross-fade; Lambert as its standard material                                                                                    |
| Howard Hinnant's date algorithm                                                                                                                                     | `pxl-geo-bake util.rs`                                                                                                              | A civil date from a day count                                                                                                            |
| Figma                                                                                                                                                               | [doc/minutes/20260912-why-the-destination-is-wasm.md](./doc/minutes/20260912-why-the-destination-is-wasm.md)                        | A compiled engine in the page over one memory                                                                                            |
| iOS's interface guidelines, Material's motion, SF Symbols                                                                                                           | `apps/client/src/shell/ui/theme.ts`, `shell/ui/motion/transition.ts`, `shell/ui/icon.tsx`                                           | Touch targets and type scale, easing, the icons' names                                                                                   |
