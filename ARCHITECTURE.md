# Architecture — pixelet

[pixelet](./README.md) turns geographic data and player-built blocks into one voxel world and draws it in a browser tab or a device app. It is an application written over [xpute](./xpute/ARCHITECTURE.md): a Rust kernel that runs as xpute's guest, the TypeScript around it — its host, which is xpute's, and its shell, the UI — and Rust servers and bakes that share the kernel's model. The memory, the rings and the turn are xpute's and described there; this document is what pixelet builds on them.

The whole kernel is one chain, and every part below is a step of it. The eye asks what it can tell apart; that question goes down — through the chunks drawn for it, the view that decides what is observed, the query that answers for every layer, the layers that store the world, to the reads that bring a tile in — and each answer comes back up the same way. Nothing on the chain names what is above it.

```text
  turn · door · shell   wire everything; nothing below names them
  ──────────────────────────────────────────────────────────────
  control ──▶ view      moves the eye; the turn hands on the rest

  asks                                                     answers
    │   scene  what this frame draws             scene/       ▲
    │   chunk  what is drawn, from the query     chunk/       │
    │   view   the pose, sight, demand           view/        │
    │   query  the only seam                     query.rs     │
    │   layer  the base and the object           layer/       │
    ▼   io     reads, credits, the host's end    io/          │

  leaves   settings, models, decor, billboards, under their readers
  beside   the profiler: reads anything; no decision reads it
```

The parts follow the chain. **The model** is what the chain carries: what a size and a position are, and what each layer stores. **The view** is the chain from the eye down to the layers. **Below the view** is what the chain stands on: the reads, the turn, the memory. **Drawing what the view chose** is the last step up, from the list the kernel makes to a frame. **Making the data** is a second chain, run ahead of the view, that ends where the first one reads.

---

# The model

## Every size is a zoom

The X lane of a position is one world of 2^24 units, so a thing the world over 2^z wide is at zoom z — z is 24 less the log of its side in units, a bigger zoom is finer, and one zoom finer is half as wide (`pxl-spec space/coord/zoom.rs`). Sizes had been said three ways, a zoom for a tile, a scale for a cell and a fixed tier for the rest, and the conversions between them were where facts went double; one number for all of them leaves nothing to convert ([every size is a zoom](./decision/20261006-every-size-is-a-zoom.md)).

| Zoom | Side   | What it sizes                                            |
| ---- | ------ | -------------------------------------------------------- |
| z27  | `1/8u` | a placed object's finest cell                            |
| z24  | `1u`   | a ground cell, the unit a position names                 |
| z15  | `512u` | a leaf tile: the finest terrain is fetched and stored at |
| z5   |        | the coarsest tile, where the pyramid stops for now       |

The words for what is sized build on one another. A **cell** is the granule at a zoom, named and possibly empty: a **pixel** is a cell carrying a 2D sample, a **voxel** a cell carrying a material. A **tile** is what is fetched and stored as one: a layer's tile holds a count of its cells a side that is the layer's own and the same at every zoom (`pxl-spec space/layer/lattice.rs` `Lattice`), so a tile's zoom is its cells' less a constant — terrain's tile is 512 cells, its z24 cells making z15 tiles. The tiles of every zoom make one tree, the **pyramid**: a tile at z covers the 2 × 2 under it at z + 1, and its name, a 2D Morton key a digit a zoom (`space/hierarchy/morton.rs`), is a prefix of theirs. A **chunk** is what is built and drawn as one, the view's unit, and the view defines it. A **block** is a thing placed, never a size. A lattice address goes to its cell at any zoom and back by the same two operations (`zoom.rs` `idx`, `from_idx`). A tile's zoom is a web map's tile zoom, and a z24 cell a web map's z16 pixel, which only the 3857 adapter says (`web_zoom_of`).

## A position — PVF3

A position is **PVF3**, a Packed 3-Vector with Fixed-point arithmetic: 96 bits of fixed point, in two words, which unit cell and where inside it. A point and a displacement share the form, but only a displacement adds to a point.

```text
IVec, one 64-bit word                 FVec, one 32-bit word
┌───────────┬───────────┬────────┐    ┌──────┬──────┬──────┬──────┐
│     X     │     Y     │   Z    │    │      │  X   │  Y   │  Z   │
│  24 bits  │  24 bits  │   16   │    │unused│  8   │  8   │  8   │
│   63..40  │   39..16  │  15..0 │    │31..24│23..16│ 15..8│  7..0│
└───────────┴───────────┴────────┘    └──────┴──────┴──────┴──────┘
signed; X is cyclic, so the world     unsigned residues:
wraps at the longitude seam           1/256 of a unit on each axis
```

Each field is a **lane** of its word. The arithmetic is integer — a lane is computed as `i64` and packed back — so a coordinate is the same bits on every machine. There is one implementation of the packing, and it stays one because the shell reads coordinates through the kernel rather than decoding them itself.

The Web Mercator adapter (EPSG:3857) gives the unit its meaning: `1u` is a z24 cell's side, about 2.389 Mercator meters at the equator. Distortion and the tile-constant vertical scale live in the adapter. PVF3 is a plane: it follows 3857 and leaves the poles out (`space/coord/adapter/4326.rs` `CLAMP_LAT`), and the pyramid never leaves it. The planet around it is a sphere, a second domain and not a copy of this one; what is drawn there is Drawing's.

## The layers

The world is stored as layers, each in its own shape, and a layer is a payload and a one-zoom fold and nothing else: what a tile at z holds is folded from the tiles at z + 1 under it, by the layer's own rule, and its cells are the layer's lattice (`layer.rs` `LATTICE`). Which tiles are wanted, held and fetched is the view's and the reads', never a layer's.

The layers fall on one line, what changes. The **base** is the basemap baked from the sources — terrain, place, division and earth: a tile's content is fixed by its name within a version, and changes only when another version answers it. The **object** layer is what people author, and changes under its keys. That is the line between them, not dimension: the base holds 3D things too, and an object may never move.

| Side   | Layer    | Stores                                                    | Shape                         | Folds by                          |
| ------ | -------- | --------------------------------------------------------- | ----------------------------- | --------------------------------- |
| base   | terrain  | the ground's look and height, and its separation          | a raster tile, 2.5D           | majority (look), average (height) |
| base   | place    | the places Carto shows at the tile's zoom, each from when | a raster tile, an icon a cell | what shows from the parent's zoom |
| base   | division | the finest division each cell is in, with its ancestors   | a raster tile, from z12       | majority                          |
| base   | earth    | the planet's picture                                      | a raster tile, top zoom only  | —                                 |
| object | object   | what players place                                        | a sparse octree a shard       | majority                          |

**Terrain** is the base's ground: a **surface**, which is 2.5D, and its **separation** above and below it, which is not. A tile is a raster of ground and one of elevation, padded so that bicubic sampling can reach past its edge, and the tile's separation. A ground cell is one byte, a look from OpenStreetMap Carto: `forest`, `residential_road`, `building`, each Carto's color, numbered in the order Carto paints (`spec/spec/block/cube.json`). `0` is void and `255` unknown — the ground past the coverage, which no source here describes, and what memory filled with `0xFF` reads as — and neither gives a surface. Each look carries the kind the renderer draws it as: land, water, transport or a structure. Its **separation** is the ways at another grade than the surface — a bridge, a tunnel and the approaches between — each kept as a line with a top along it, a width and the type its surface shows (`pxl-spec bake/separation.rs`): sparse, and nothing on a tile that has none. Beside the surface a tile carries its **index**: the lowest and highest anything under it stands and which of its children exist, so it is read the moment it is resident, with nothing over it required. How a tile is made is Making the data's.

**Objects** are client-local. The store (`pxl-kernel layer/object/store.rs`) keeps only its record: each leaf's catalog value at its node, in one sparse tree a shard, over its 3D roots (`pxl-spec space/hierarchy/sparse_tree.rs`), whose branches hold nothing but the way down. What a node at any zoom shows — a leaf's value, or its eight folded by majority (`fold_cell`) — is read from the leaves over and under it, never kept. Besides the catalog's ids the store knows three values: `VOID`, nothing written; `AIR`, a leaf that erases what a lower layer would show; and `UNKNOWN`. A leaf may also carry an **extension**, what its id holds beyond itself — a marker's words — kept in a pool of its **class**, the ids of one kind whose extensions have one shape (`spec/spec/block/ext.json`, `pxl-spec block/ext.rs` `class_of`); the leaf names its record by its place in that pool (`layer/object/ext.rs`), so a record is placed and erased with its leaf, and a leaf carrying one is never joined with its neighbours. A write splits a leaf over its node, frees what is under it, and joins eight equal leaves into their parent, so one picture has one tree; a write is `<key, id>` (`space/hierarchy/morton.rs` `key_of`). It persists to the platform's storage — `localStorage` in a tab, `UserDefaults` on an iPhone, `SharedPreferences` on Android — under a key that carries the layout's version; a layout that changes takes a new key and leaves the old one unread.

**Places** are tiles of the same name and zoom as the ground tile they lie over, each holding what OpenStreetMap Carto draws of it at the web map's zoom of the same number, the leaf every zoom past it through Carto's last (`pxl-spec bake/tile/place.rs`). A place's kind is Overture's `basic_category` (`spec/spec/bake/place_category.json`), and what Carto draws for a kind — a dot or its icon, its name, their colors, from which zoom — is one table taken from Carto's own rules (`spec/spec/bake/place_theme.json`); a kind with no row is not drawn, nor carried. Where icons would meet, a cell an icon wide keeps one place, the first by the zoom its kind shows from and then its id, so each place carries one number, the zoom it is shown from, what is shown only grows with the zoom, and a parent is its children with what shows from its zoom on (decision/20261009-places-follow-carto.md). A child is a tile only where it holds more (`pxl-spec bake/tile/terrain.rs` `Occupancy`). The layer is a setting.

The **earth** layer is the planet's picture, baked at its top zoom only (`pxl-spec bake/tile/earth.rs`); what it answers past the pyramid is Drawing's.

## What is drawn as one thing

What stands in one cell is a **block**, drawn on two axes that do not depend on each other. Its **kind** is how it is drawn — a cube, a decor model, a marker drawn flat to the camera — and it is the block catalog's: an id is its kind over its shape (`spec/spec/block/id.json`, `SHAPE_BITS`), so a test for decor is a test of the kind. Its **source** is which side holds it: the base, whose ground grows it, or the object layer, where a player placed it. So decor is no layer: a tree the ground grows and a tree a player places are one kind from two sources, and both arrive as the same record, `layer/instance.rs` `Instance` — its id, where it stands, its source and what its class carries (a decor's variant, heading and scale). The ground grows decor by evaluating, never storing: what stands in a leaf cell is a function of the cell, its look and a seed (`layer/terrain/scatter.rs` `grown_at`), and a region scattered at the leaves is its cells scattered alone (`pxl-spec procedural.test.rs`).

| Kind   | Base                                | Object          |
| ------ | ----------------------------------- | --------------- |
| cube   | the ground's look, a cell's surface | a placed cube   |
| decor  | what the ground grows               | a placed decor  |
| marker | a place, Carto's icon and its name  | a placed marker |

Every cube's color, every decor kind's models, seed and density are catalog entries, compiled into both sides and never sent (Catalogs and assets).

---

# The view

**The world is stored in its own shape, and the view keeps only what an observer can tell apart.** Looking farther shows nothing more: what reaches the screen is its area, a direction answers with the first thing it meets, and how far that is chooses how finely it is drawn. So what the view holds and does is bounded by the screen, not by the world under it — a mountain range, a city or two hundred towers stacked behind a ridge cost what the screen they cover costs ([what is drawn is what the screen covers](./decision/20260928-what-is-drawn-is-what-the-screen-covers.md)).

## The camera

What looks is one camera (`control/camera/controller.rs` `Camera`): an **eye** and a **look** in spherical coordinates about the world's Z, θ = polar from straight down and φ = azimuth from north toward east, which every controller moves and nothing else keeps a copy of. The free camera (`free.rs`) is first person: it turns about its eye and walks it. The orbit (`orbit.rs`) is third person: it turns about a point `dist` ahead of the eye along the look, and its distance is the only state it adds. That point is the **focus** — the eye itself first person, the tracked eye while a tracker holds the pose (`INPUT_POSE`) — derived from the camera and never kept (`focus`). It is where the character stands, half a cell over the floor as a cell's center stands over its ground (`Cell::center`). A change of person moves the camera about the focus and never the focus; a navigation command puts the camera so the focus is where it says, once the ground there is known exactly (`control/navigation/destination.rs`). The **anchor** is where the world is laid on the planet: the eye's longitude and latitude (`view/frame/pose.rs` `anchor`), which the sky and the Earth read. Nothing else reads a controller's words; what is generic reads the eye, the look and the focus.

The orbit's three controls — `dist`, `polar`, `azimuth` — are written only through one funnel (`control/camera/orbit.rs` `ask`), and terrain may ask one question in it: would this state put the eye inside the ground (`control/camera/orbit/terrain_gate.rs` `allowed_view`). `Settings > Camera` picks the answer: `through` ignores terrain, `stop` takes a control as far as it can, `climb` raises the drawn pitch instead where the eye gets there through the air. A pan moves the focus by the lowest way there (`control/navigation/support.rs` `pan_to`). It goes along the way at its height, and over what stands across it, tried again from the top of what it struck (`query/support.rs` `open_from`) to a height the focus's own column must reach before its ceiling does (`ceiling_over`). Then it stands on the floor under that height: under the open sky over anything, under a roof, a deck or a cave's rock on its layer.

The **pose** (`view/frame/pose.rs`) is the eye and the turn from the world's axes to the camera's, and the matrices made from it. The world stays in its own axes, in doubles whose X runs on past the seam, a canonical point entering them on the lap nearest the eye (`view/frame/coord.rs` `to_world`); what turns is the observer. Every draw uploads its point less the eye, subtracted in doubles before it is narrowed (`pxl-render frame/draw.rs` `translation`), so nothing is kept against an origin of its own. What the pointer picks and edits — the tool held, what is under it, what a tap does — is `control/interaction.rs`'s, apart from the camera.

## The eye's cells

What the eye sees is quantized into **cells**, each the least the observer promises to tell apart: a solid angle on the sphere about the eye, the same wherever on the sphere it lies, so no coordinates on the sphere enter it. A screen only calibrates it — a cell spans about twelve logical pixels at the optical axis (`pxl-spec space/camera/grid.rs` `SPLIT`, `target_cell_px`), never device pixels — and a perspective screen draws the same solid angle wider toward its edges. Inside a cell nothing is promised: drawn finer, exact, averaged or some other shape altogether, as long as the approximation does not leak into a neighbouring cell by more than the area measure allows; a cube seen along a face's diagonal spans 2^¼ of a cell on one axis.

That is the one rule of detail: a piece of the world is refined where it contributes to some cell and its error shows the eye more than a cell's area, the silhouette of its error box seen from its nearest distance (`view/sight.rs` `Lens::error`). Height is not special: storage may be anisotropic, but the rule measures the area that reaches the eye, every face of the error box alike. A far pole thinner than a cell is folded into what is around it; what is hidden contributes to no cell and is never refined. The rule is not yet closed by area. The error box is a cube judged from a piece's nearest point, and a flat top seen grazing shows the eye far less than the cube's silhouette, so ground there is drawn at a quarter of its cell's area or less across the far half of a wide view; how the bound is closed is BACKLOG.md's P1.

How far away a cell of a zoom begins is the **camera grid**: one factor, τ, on the distance each zoom splits at (`grid.rs` `Tau`, `Tau::cell_at`), and the ground's tile zoom a distance wants follows from it (`space/layer/lattice.rs` `z_at`). τ = 1 is `SPLIT`, the finest cell everything is sized for, and a screen that asks finer is held there. A screen holds at most `GRID_CELLS_MAX` cells (`param/view.rs`), so a large window grows its cells rather than what it draws.

## The query, the only seam

The query is the only place a layer's shape is known. Every layer answers one question of a **node** — a tile or a cube, by zoom and index — at a cell zoom the asker states (`layer/query.rs` `Query`): its **bound** (`bounds`), nothing known, empty, or inside a conservative box with the finest zoom held under it (`Bound`'s `finest`); and its **records**, a view over what holds the node's cells, never a copy. A descent carries a **cursor** from a node to its children, so each step is answered from its parent and never asked again from the top. Each answer says what it lacked: exact, coarser with the tile it would have needed, or nothing and that tile (`layer/answer.rs` `Answer`). The query fetches nothing, holds nothing and keeps no copy.

The **world** composes the layers into one answer (`query/world.rs` `World`). Its record for a chunk column is the ground the column is read from and what is placed in it (`query/block.rs` `Column`), and what is in each block is composed from that once, for drawing and for control alike (`pxl-spec block/stand.rs` `compose`): the ground, then its bridges and tunnels, then what is placed, air included, each block filling its cell or not (`Block::fills`). A chunk draws the column it composes; a point or a ray asked of the world (`query/support.rs`, `query/ray.rs` `first_hit`) is answered from the same column, composed at the zoom what is placed there is held at, so where a chunk draws a cell as held, what control meets there is what is drawn. Control never asks a folded cell, which is a summary. The query composes only the cell it is asked of, and keeps nothing: a tile's ways are found through the box tree it carries (`pxl-spec bake/separation.rs` `crossing_grid`), so composing costs what is asked, not what the tile holds. What the ground grows stands where its footing is: a void block over ground it grows on (`World::grows`). A ray walks the ground region by region: where it stays above a region's index bound it passes, decided by the index and never by the surface a coarse tile draws, which averages away the peaks under it; elsewhere it walks cell columns. Objects are found first, so ground is walked only up to them. The pointer's ray also strikes decor, as the walk passes it: each leaf column strikes the decor standing near it against its model, and only where the detail rule at the τ the chunks draw with holds decor as it is (`query/decor.rs`), so what is struck is what is drawn there; it follows that rule, not the chunk the view chose. The pointer, the camera's clearance, the focus's footing and the view's own observations all ask the world; none reads a layer.

## Chunks

A **chunk** is what is built and drawn as one: a cube of the camera grid's cells, 32 a side, so its zoom is its cells' less five (`grid.rs` `CHUNK_STEPS`), addressed by zoom and x, y and z (`chunk.rs`). A cube that holds nothing is no chunk. It is never seen: one cell of it is one thing drawn, and what lies inside a cell is its layer's to fold.

**Materialization** makes what is drawn. Its goal is all around the eye, refined only where the screen's cells observe. The screen is cut into **patches** coarser than its cells (`param/view.rs` `OBSERVED_PATCH`), each casting its corner rays, and the stretch of world between their hits is what the patch's cells touch. A chunk splits into its eight children while their error crosses a cell and something under it is held finer, as the query's bound says (`chunk/domain.rs`): the ground stops at a leaf's cells, the object layer at its finest held. A chunk hands off where one of its cells would project to about twelve pixels, so the cells drawn step `1/2/4/8/16/32/64` units and on. What is drawn is a **frontier** moved toward that goal in whole moves (`chunk/frontier.rs` `plan`): a piece is replaced by the pieces under or over it only once every one of them is built, and a move starts only when every arena slot it takes is free. Waiting for a tile, a build or a slot is always waiting with a coarser piece on the screen, never a hole. When the arena cannot hold the goal at all, the **quality** is chosen coarser — the finest step of τ that fits, coarser at once when the view needs it and finer a step at a time once there is room — and read by demand and the pick alike, handed down by the turn (`chunk/quality.rs`).

**The table** (`chunk/table.rs`) is one slot per mounted chunk: its role in the frontier, flags, distances, bounding sphere, pending-since, retained bytes. A tick refreshes each against the frustum, rechecks whether what it was built from still answers, takes the frontier's step (`materialize`) and commits a layout whose mesh is in hand. What to build next is one priority (`priority`): the chunk whose build is in flight first, then what the view sees nearest the eye, then what it does not.

**A build** is where the layers meet, once. A chunk asks the query for its column's records and composes them (`query/block.rs` `Column::compose`). The ground's are sampled a column a cell: its look and height, and its bridges and tunnels standing off the column. Past the leaves each column is the leaf column it lies in, so nothing finer than the ground is invented (`pxl-spec block/sample.rs` `ChunkSample::refine`). The object layer's are folded to the same columns by majority, with a cell of apron around, as the edits' previews show them (`Cells::fold`, `Cells::carve`). What is laid later wins a cell: the ground, then its separation, then what is placed, air included. Each column becomes its **runs**, one span per stretch that holds a single material, in a heap block exactly the size the writer counted (`pxl-render mesh/run.rs`); the chunks of one column bake their parts of one set, and then it is let go. A chunk is remade only where what answers its node changed: the tile that answered, the objects written where it draws, or the edits' previews (`chunk/source.rs`).

```text
terrain tile ──▶ sample, a column a cell ──────────────┐
                                                        ├─▶ column runs ─▶ mesh
object store ──▶ folded to the chunk's cells ──────────┘
```

**A mesh is its boxes, and lives on the device.** It is held in slots of the fixed buffers the renderer keeps (`pxl-render gpu/arena.rs`); any chunk takes any free slot of its pool (`chunk/mesh_slot.rs`), all it needs at once or none, and gives them back the moment it is not drawn. A mesh is never cut, since a box left out is a hole, and never baked coarser to fit, since which chunks are drawn is the quality's to choose. A box is its corner, size, color, texture layers and face mask, unfolded into a cube by the vertex shader (`frame/shader/chunk.wgsl`). A run that is not its column's top and whose four neighbors are solid across its exact `z0..z1` span emits nothing, a chunk's edge columns exempt; adjacent columns holding the same run merge into one box, so a plateau is one box.

A chunk draws its decor beside its mesh: the instances the ground grows in it and those placed, as one block of lanes a group of kind and variant (`chunk/decor/build.rs`). A kind is drawn as a model down to its own size on screen and as a sprite below it (`param/chunk.rs` `DECOR_MODEL_PX`, `DECOR_SPRITE_PX`). Culling uses the exact union of the mesh's bounds; until a chunk is measured, its bounds are what the query said of its cube.

## Demand and residency

**Demand** is what an observation requires — the ground at the resolution its distance chooses — before it is any tile; the layer's storage maps the requirement to its tiles (`view/demand.rs` `observe`). The exact requirement depends on what the world holds, so it is estimated from what is known: the ground the patches reach, at each zoom whose band of distance some of it lies in, and what their rays read coarser than asked (`view/observation.rs` `ByFootprints`). Where the rays met coarse ground they name what they lacked, and once it comes the patches over it are cast again, so the requirement grows to what the exact records make it. A tile of the requirement that is not resident is a miss, asked for at the zoom it wants. Every layer of the pyramid is asked the same way, so the place layer has no residency, schedule or key of its own.

**Residency** holds a tile hard for the floor, for the demand while it is demanded, or as cache; only the hard set is correctness. Every reader of the ground takes the floor's answer where the exact tile is not resident. The **floor** is a law — every observation has an answer — kept by holding the coarsest zoom the source supplies whole, all around the eye (`layer/residency/floor.rs`); the source declares that zoom, the published version in its metadata and the dev tile server at `/tile/source`. Capacity is two asserts on its frames (`Residency::FRAMES`): one frame for everything held hard and one to land in, and everything demand can name at once besides. A tile that arrives takes a free frame or one nothing holds hard and the view does not read, the farthest first. A region is unresolved until the floor lands over it, then covered coarse, then as finely as it is observed, and it never goes back.

A version is never changed once written, so a tile never changes under its key (how a version is made and named is Making the data's); what changes is which tile answers a node, and that is the one change the chunks are told.

**Nothing is prefetched.** A view converges to exact on its misses alone — a miss, a coarser answer meanwhile, a fetch, a refinement — so a prefetch would be a policy and part of no meaning, bought only as latency where it is measured to be needed.

## Inside a cell

A cell drawn coarser than a leaf's (`lod_step > 1`) is the `lod_step²` columns under it summarized, one zoom finer (`pxl-spec space/layer/lattice.rs` `Lattice::fold`), and the ground and what stands on it are summarized apart (`pxl-render mesh/chunk.rs`). The ground is a height field, so only its footprint coarsens: one run from the lowest floor to the columns' mean top, its color the mean of their top surfaces. What objects add above it is a volume: its `lod_step³` voxels to one box whose fill is coverage over the cube, shrunk about the cell center by the cube root of the fill so its volume is the fill's. Heights are units at every zoom, so no ground is ever a cell part full.

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

Flat ground at a distance therefore reads as separated cubes rather than a sheet — the cost of a rule that treats every axis alike, taken over rules where orientation would decide whether a thing exists. What is folded is a summary, not what is held: nothing in a folded cell is struck, hidden or cut by a preview (`chunk/build.rs` `draws_as_held`, `control/interaction.rs` `folded_at`), and nothing in one is pointed at.

## Each concern's bound

Taken together, the view is six concerns between the world and the screen, each with its own bound and its own contract:

| Concern         | Asks                                          | Bounded by                           | Where                                              |
| --------------- | --------------------------------------------- | ------------------------------------ | -------------------------------------------------- |
| Storage         | a layer's records, in the layer's own shape   | nothing of the view                  | a layer; the bucket                                |
| Query           | what a node holds, a point, or first on a ray | —; it holds nothing                  | `query.rs`                                         |
| Materialization | what is drawn from what is held               | the arena                            | `chunk/domain.rs`, `chunk/frontier.rs`             |
| Demand          | which records an observation requires         | 3 × 3 tiles a zoom (`REACH_TILES`)   | `pxl-spec space/layer/demand.rs`, `view/demand.rs` |
| Residency       | which records are held, and why               | its frames, asserted at compile time | `layer/residency.rs`                               |
| Scheduling      | which waiting work runs now                   | the turn's quota and the credits     | `io/request.rs`, `io/policy.rs`                    |

Every number a person would tune — τ's target, the patch, the arena, the quality's step and wait, the per-frame caps — is named once in `pxl-kernel param.rs`, grouped by the policy it tunes, with its reason.

## Marks

A mark stands at a cell's center (`Cell::center`, `pxl-spec space/layer/tile.rs`), at the height of the surface drawn there. A place is the place layer's mark (`chunk/place.rs`): each drawn chunk reads the tiles whose cells are its grid's cells, an icon wide, each from the tile that says it exactly, and shows what Carto shows at the zoom those cells are drawn at — a place from the zoom it is shown from, as a dot or its icon by its kind's zooms, named from its kind's label zoom once it was shown a zoom before (`badge.rs` `Label::at`, `layer/place.rs` `carto_zoom`). A place stands on what a ray down through its cell's center meets first at the zoom the view draws there (`first_hit`), and where nothing is drawn there is none. A tap on one is its before the ground's (`chunk/place.rs` `tap`). How it is drawn is Drawing's.

---

# Below the view

Everything the view asks for has to arrive, be paid for in time and memory, and fail in a way that leaves what is drawn alone. That is what the chain stands on: the host's end and the reads it carries, the turn that gives each step its share of a frame, the memory every range lives in. None of it names anything above it. A layer asks io for a tile; io never asks what a tile is for.

## The kernel, its host and its shell

The kernel is the process: it holds the world and decides everything about it, and it owns no device. The TypeScript around it is two things to it. Its **host** is what an operating system is to a process — xpute's host — and its **shell** is the UI, a program that talks to the kernel as one process talks to another.

The host is most of `apps/web/src/kernel/`. It opens the kernel: in a tab it reserves the memory and instantiates the module over it (`mem.ts`, `module.ts`); on a device it asks the app's native module for the linked kernel's memory (`platform/guest.ts`, `apps/native/modules/platform/`). It rings the doorbell once per animation frame when something asked for one, which runs the kernel's `interrupt` (`pxl_interrupt` on a device), whose first turn boots it. It forwards pointer, wheel, sensor and visibility events as commands (`control/input.ts`), does the I/O the kernel asks for into blocks the kernel chose, and holds the GPU: in a tab it runs the renderer's recorded calls against WebGPU after each turn, on a device it binds a view's Metal layer or `SurfaceView` to a surface and the kernel draws into it directly.

The shell (`apps/web/src/app/`, `feature/`) is React over the snapshot the kernel writes, and every control is a command the kernel applies. Its end of the channel is `shell.ts`, `snapshot.ts` and `render/surface.ts` in `apps/web/src/kernel/`. It hands the kernel the surfaces it draws into, each named by a handle — a canvas transferred once to an `OffscreenCanvas` in a tab, a view on a device. Nothing in it computes a coordinate, a tile or a draw; what it needs of the model — catalogs, ABI enums, routes — is generated into `apps/web/src/**/*.spec.ts` from the same JSON the Rust is.

A **bracket** is what gives the kernel a place to run: `pxl-wasm` in a tab, `pxl-native` on a device (with `android.rs` for its JNI). It hands in the memory, the clock, the allocator's reading and where to say what stopped the kernel (`door.rs` `Bracket`), and the kernel asks everything else of the host. So the kernel also runs under `cargo test`, with no bracket at all.

## The host's end

Everything the kernel needs from outside it asks of its host, as calls it records during a turn and the host runs after the turn returns, so each is done before the next turn begins (`io/host.rs`). Only the clock is called directly, since a turn measures its own budget while it runs. An answer comes back on the next turn as a number the host wrote, and it is read once, into a count or an errno: `answered` turns the raw result into `Ok(u32)` or one of xpute's `Errno` members, and a code no member names is the host breaking the protocol, `EPROTO`. No reader compares a raw number's sign again.

What the host tells of itself sits beside the calls: the wall clock, whether the device is handheld, whether the build is a development one, the allocator's reading, and the bracket's way of saying why the kernel stopped. The door, at the top, only hands these in at boot; everything below reads them here.

## Reads, and the one scheduler

A read is a file, in xpute's sense: the kernel names an object and opens it, and the host alone knows the address and the protocol (`io/file.rs`). An open answers one of three things, and the type says which, so no caller tells them apart by a number:

| `Opened`             | What it means                        | The descriptor |
| -------------------- | ------------------------------------ | -------------- |
| `Bytes(size)`        | the file is there, `size` bytes long | the owner's    |
| `Absent`             | no such file                         | closed         |
| `Failed { errno … }` | anything else                        | closed         |

Each descriptor has an owner, so an answer reaches the one that opened it and an answer for a closed descriptor reaches no one.

The world's reads are fetches (`io/fetch.rs`): a tile named by its bake and its name in it (`DemandName`), opened as a file. An absent object is an answer with no body, whose meaning its owner decides — no ground tile is ground its parent already says — while an empty read is no object at all (`EBADMSG`). A body is a heap block, handed to its owner in an `Answered` and released by the command that decodes it unless a store keeps it. Answers queue until their owner takes them, each owner taking only its own categories (`next_answered`), and an answer settles its fetch once the owner has taken it in or refused it (`answer_settled`).

How many reads are out at once is bounded by **credits**, which the host grants each turn, and one place spends them: the request scheduler (`io/request.rs`). What it holds to is that the demand of a camera that has just moved is representable however much old work is still draining. That follows from its shape. A producer says only what it wants — the subjects it wants read, each with a rank, replacing whatever it said last turn — and never that an attempt should start, stop or be retried; what is wanted and what is running are two tables of fixed size. So an attempt ending answers nothing about the demand that outlived it: an answer ends its want, a failure ends only its attempt and waits rather than asking straight back. A running attempt is never preempted, and stops only when nobody wants it any more.

The scheduler is the mechanism and orders nothing itself. A serve policy (`io/policy.rs`) ranks each want: by layer in the order a view builds up — terrain, then the assets decor is drawn with, then the place layer — and within a layer seen before unseen, nearer first, and of tiles as near the finer first, so the leaf under the eye is not asked after the coarse tiles over it. The policy changes how soon the view is right, never what it is. A model or a texture is a read like any other, spending the same credits as a tile rather than bidding against it from a second scheduler (`io/load.rs`): fetched and parsed at most once, retried if the read did not arrive, final if the bytes do not parse, since asking again returns the same bytes. Images are decoded by the host under a handle the kernel issued and never cross into the kernel.

Two files are kept on the device, the settings and the placed objects (`Kept`). Reading one back answers `KeptFile`: `None` when nothing was kept, the `Bytes`, or `Unreadable` — there, but larger than any heap a device is given, or failing with an error other than its absence. An unreadable file must not be written over, so what keeps it writes nothing until it has loaded. A read the heap refuses waits for the heap's next release and opens again (`take_kept`).

Text is asked of the platform a line at a time (`io/text.rs`): the kernel asks for a text's lines and widths and then for each line's pixels, through the same recorded calls, and keeps only the answers the frame before read. How it is drawn is Drawing's.

## When something goes wrong

Two kinds of trouble, and they never mix.

A run that does not succeed **ended**, and how is one value: `Ended { Ok, Aborted, Failed(Failure) }` (`io/failure.rs`), so a run that ended well carries no failure and a failed one always does. A `Failure` is an errno, a detail and, where the kernel made it, the source location, since `EBADMSG` alone does not say which rule a body broke. A failed fetch waits and is asked again; a failed parse is final. Neither touches what is drawn.

A broken invariant is a **bug**, and it ends the module. `bug!` reports the errno and the place to the host (`host_bug`), and where the bracket's process dies with the kernel — a device — the kernel's last words say why (`Stop::Bug`, `Stop::Panic`), as numbers, since formatting is the bracket's and a module that reports nothing links none of it. A page's host outlives the kernel and reads the report itself.

Memory pressure is neither. What memory being full means decides how each piece is sized, and AGENTS.md holds the rule: must-fit state sized from a bound the structure proves, a bounded flow full by its owner's policy, a workspace that asks first and waits when refused. Where each lives is the memory map's, below.

## The turn

The turn is xpute's, and so is the quota: the host rings once, the guest spends what it was given, and the falling edge says when to ring again ([xpute/ARCHITECTURE.md](./xpute/ARCHITECTURE.md) is the mechanism). What pixelet decides is what fills it.

```text
                         the rising edge
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│ the frame task                                                 │
│                                                                │
│   effects  each compares the world with what it last saw:      │
│            navigation, the view, the fetchers, the object      │
│            store, the hover, the puck                          │
│   tick     interaction → visible → content → cosmetic → report │
│   pump     start the reads this turn's credits bought          │
│   stage    the frame's uniforms and its draw list              │
│                                                                │
│ five tasks beside it, polled on what the tick left             │
│                                                                │
│   fetch · glTF · build · decor assets · the object store       │
└────────────────────────────────────────────────────────────────┘
                                │
                                ▼
                        the falling edge  ──▶  wake_ms
```

The host submits one `WORLD_FRAME` before it rings — the viewport, the gap since the last turn, and how many reads this turn's credits allow to be out — and reads the completion ring after.

**The quota's policy** is the kernel's (`sched/tick.rs`, `QUANTUM`), and reaches the host in the kernel's directory, which the first turn writes for the host to read once. The frame is the 25th percentile of recent gaps between frames, ignoring any gap over 50 ms. A turn is interactive when input arrived within `settle_ms`, or when the kernel asked for it on its last falling edge; an interactive turn gets that frame less a margin, every other turn `batch_frames` frames.

**The phases** are xpute's, in the order that is their priority:

| Phase         | What pixelet runs in it                                           |
| ------------- | ----------------------------------------------------------------- |
| `interaction` | the controller, input, the camera, the environment, the companion |
| `visible`     | the sight, the goal, demand, the chunks' pass                     |
| `content`     | nothing yet; it is guaranteed `content_share` of the budget       |
| `cosmetic`    | the world gizmo                                                   |
| `report`      | the profiler, last, so it sees everything                         |

The tasks run on `embassy-executor`'s raw executor, which the module installs. The frame is a task rather than a function so that the order above is its own, and no task depends on the order the executor polls them in.

**What moved** is said once a turn, after arrivals are admitted and before the layers observe or the chunks compare their sources (`turn.rs` `changes_step`): the tiles whose answer moved, as identities, and the regions the object store wrote (`layer/object/store.rs` `take_written`). A tile's identity fixes its content, so a list of identities is all a reader needs; the store calls nothing above it, and its regions move on a write and on nothing else.

## The channel and the snapshot

The host and the kernel share the two rings xpute describes, of four-word messages with page-sized payload slots: 2048 for what the host submits — its own commands and the shell's, which go through it — and 4096 for what the kernel completes (`mem.json`). The completion ring holds everything one turn leaves, since the host reads it dry before it rings again; a turn short of room for what one more command might raise leaves the rest of the submissions for the next turn, in order, and asks for it.

The commands are `spec/client/kernel/abi/cmd.json`'s, numbered from `0x10` past xpute's own (`xpute/spec/abi/cmd.json`, the fetches among them): each one's number and its packet's fields, from which both sides' readers and writers are generated, so no field is found by its position anywhere else. A packet is an XTP tree, xpute's relocatable binary tree, and anything larger than a slot crosses in a heap block the kernel hands out. The kernel also posts messages nobody asked for — a fetch to start or cancel, an image to decode, a sensor to watch, a new snapshot — and those are its signals.

The shell reads the kernel through one **snapshot**, written after a turn that changed it and replaced whole, posted only when its bytes differ from the last (`door.rs` `post_shell`). It carries what the kernel owns and nothing chosen for a presentation: the focus (`FocusState`: whether the view shows any ground yet, `GroundStatus`; whether the focus has wandered off the device; where it is; whether the source holds nothing under it), the device's location and heading, the camera's azimuth and motion, the settings, the meters a logical pixel spans at the focus, and the place it is in. A length rounded for a scale bar, how deep a place is named, when a toast speaks are the shell's, derived where read.

## The memory map

The client's state is xpute's [core memory](./xpute/ARCHITECTURE.md#the-core-memory-placed-once): in a tab a `WebAssembly.Memory`, 1 GiB on the desktop and 512 MiB on a phone, `initial == maximum`; on a device one region of 512 MiB the library takes from the system on the first call (`pxl-native memory.rs`). Since it never moves, a ring keeps its words and a table is read where it lies for the guest's whole life. A phone's tab gets half because JavaScriptCore counts the whole reservation as the tab's JavaScript memory, committed or not.

Every address the kernel names is an offset from the memory's first byte (xpute's `mem/base.rs`), so the map holds on both targets and the shell reads the same numbers either way. On a device the stack and the library's data are where the system put them, outside the region, and the two ranges the map keeps for them stand empty.

```text
address      range               bytes                  placed by
──────────────────────────────────────────────────────────────────────
0            .stack              1 MiB                  rustc
1 MiB        .ring.desc          36 KiB + 68 KiB        mem.json
2 MiB        .ring.slot          8 MiB + 16 MiB         mem.json
32 MiB−4KiB  the directory       4 KiB                  mem.json
32 MiB       .rodata .data .bss  ~0.7 MiB               wasm-ld
D            .fixed              each owner's range     door/layout.rs
D + .fixed   .heap               the rest               the allocator
end          512 MiB or 1 GiB
──────────────────────────────────────────────────────────────────────
             D: __data_end rounded up to a page, about 32.7 MiB
```

The map is `pxl-kernel mem/section.rs`'s, over xpute's sections. `.stack` is first so that an overflow wraps below zero and traps — there is no MMU, so the wraparound is the only guard — and the bracket asserts at boot that the linker's `__stack_high` is where `mem.json` says. The rings and the directory sit at fixed addresses from `spec/client/kernel/mem/mem.json`, which `build.rs` also turns into `--global-base`, so the linker cannot place `.data` over them. The directory is xpute's (`ipc/directory.rs`): the first turn writes it, and the host reads from it once where the rings are and what the quota's policy says. The same file holds the heap a phone is guaranteed (`MIN_HEAP_BYTES`), past which a kept file is refused as larger than any device's heap.

`.fixed` is what the kernel places once, sizes at compile time and never frees: the resident terrain's frames (`.fixed.terrain`, `layer/terrain/fixed.rs`, one frame a tile, as many as residency's capacity), the one open mesh bake (`.fixed.bake`), the chunk table, the renderer's live blocks, the GPU call stream, the host stream, the profiler's rings. Each owner says its size; the roof sums them and lays the ranges in order (`door/layout.rs` `Fixed`, `FIXED_BYTES`), handing each owner its range at boot, which the owner holds (`mem/fixed.rs` `Held`), so the memory's mechanism names no tenant. The live blocks' bound is each owner's count summed, every one a count the structure fixes, so the renderer's table cannot fill. Every range is read through its `Section`, which checks what is asked against the range's end. A thing belongs here when it is in the kernel's own format and its size is settled at compile time.

`.heap` is everything else, under xpute's slab over buddy: what is asked for, refused and given back, and what another format sizes — a glTF, a PNG — whose bound nothing here can predict. A refused request is a wait, not a failure: a chunk's build, its decor and a model keep their place and ask again after the heap's next release (`chunk/build.rs` `refuse`).

What the renderer reads each frame is laid out as **column lanes**, structure-of-arrays: terrain planes, column runs, a mesh per chunk, the chunk table, decor instances, every registry. A table whose columns move together is one struct of lanes with one length, and a lane set has one owner that a consumer borrows for one call. The JavaScript heap holds only glue: React, the WebGPU objects behind the kernel's handles in a tab, and a file's bytes until the kernel reads them.

## Settings and the profiler

Every setting is one entry of one schema (`spec/client/kernel/setting/schema.json`): default, kind, section. It is a leaf: anything reads it, it reads nothing. The store keeps only the user's overrides, by the setting's name, so moving a setting between sections keeps what the user chose. An entry marked `dev` is offered by a development build only; a published one leaves it out of the shell's table and holds it at its default, which `apps/web/script/check-bundle.ts` holds the published bundle to.

The **profiler** (`profiler/`) sits beside the axis rather than on it: it reads anything, and production reaches it only where it records. It keeps what each frame and each task cost — pacing, per-phase time, the GPU's, heap gauges, LOD pressure, a census of draws, per-read waits — in rings laid out by `spec/client/kernel/profiler/profiler.json`, which the shell reads in place (`PROFILER_READ`, `PROFILER_TAKE`) and prints as a report or exports as a Chrome trace. Nothing schedules by what it records.

## The device app

The device app (`apps/native`) runs the same TypeScript as the page and adds only what a device does. What differs is behind one import prefix, `@platform/`, which the page resolves to `apps/web/src/platform/web/` and Metro to `apps/native/src/platform/`; a check fails the gate if a device's module lacks a name the page's has (`apps/native/src/platform/conform.ts`). The kernel is not a module there but a library linked into the app (`pxl-native`), reached through one native module; it runs every call on a thread of its own while the caller waits, and draws through wgpu into a view's Metal layer or `SurfaceView` directly ([a device's kernel on a thread of its own](./decision/20260926-a-device-s-kernel-on-a-thread-of-its-own.md)). On an iPhone, any control a touch lands on is UIKit's, since a SwiftUI switch inside React Native's views missed quick taps; which technology draws each piece is [apps/native/src/platform/README.md](./apps/native/src/platform/README.md).

---

# Drawing what the view chose

The kernel decides and lists what to draw; the renderer executes the list and decides nothing. Everything below is the second half: how one list becomes one frame, and what is drawn that no chunk holds — the sky, the planet past the pyramid, the badges.

## The draw list and the renderer

Each turn ends with a **draw list** (`pxl-render frame/draw.rs`): entries tagged by the pipeline that draws them, each with the block it reads and its uniform. In it: the chunk meshes, fine and coarse; decor, instanced, one model at many placements, and as an impostor sprite where a kind is drawn smaller than its own model is worth; the actors, skinned; the location puck; the ghosts of an edit about to land; the badges; the debug lines. An entry's presence is its meaning: what is drawn is what is listed, and nothing beside the list says it again.

The kernel writes the frame's uniforms into the stage lane (`frame/stage.rs`) — the matrices, the viewport, the camera grid's cell angle, the light's settings — and calls `render_frame` (`gpu/renderer.rs`) with a `FrameInput`: the list, the environment, the models and their skins, the badge sheet, the Earth's tiles, and the surfaces. The renderer draws into every open surface — the world's and the character picker's display case — and reports what the frame cost and whether it wants another turn.

A frame runs in this order. What outlives the frame — the atmosphere's tables, redrawn only when the light or the camera's altitude moves — is drawn and submitted ahead of it. Then the sun's shadow map, the clouds marched at half size, and the screen-space occlusion's prepass where it is on. Then one world pass: the opaque chunks, decor and impostors; the sky at the far plane, so only empty pixels run it; the stars and the clouds over it; the puck, the ghosts and the debug lines last. The badges follow in a pass of their own, over the world, and the display cases after.

Its calls are WebGPU's, and how they are made is the target's (`gpu/api.rs`). A module cannot reach WebGPU itself, so on wasm32 they are records the page makes once the turn has returned (`api/record.rs` ↔ `apps/web/src/platform/web/webgpu.ts`); on a device the same calls go through wgpu as they are asked for (`api/device.rs`). A call carries numbers and the address and length of what it names, the same words to both, and every GPU object is one of xpute's handles.

Shaders and pipelines are the kernel's (`frame/shader/`). A pipeline is a record of words — shaders, vertex lanes, bind group layouts, primitive, depth and blend state — that the page turns into a WebGPU descriptor and a device into wgpu's. `cargo test` compiles every shader with naga and checks each record against its shader. What a shader shares with the host has no room inside it — 4-byte scalars and `vec4`s, and arrays and structs of them with no gap — so the host's words and every driver's layout agree, since a phone's Vulkan read a padded field and a matrix array in storage its own way ([a matrix the driver reads its own way](./decision/20260927-a-matrix-the-driver-reads-its-own-way.md)). A uniform block is bound as `array<vec4f, N>` and each field read from its word by a function of its own, never by one that builds the struct back, with which Safari's WebGPU lost its GPU process. A fact the GPU needs has one home there: the cell angle is the frame uniform's `cell`, which every pass that needs it reads.

Two choices outlive a frame. Depth is reversed float, near at 1, so precision falls where perspective needs it and far coplanar faces do not fight. And on device loss the blocks are still in memory, so everything is re-uploaded from the addresses it already had.

## Past the pyramid: the sky and the sphere

The pyramid ends where PVF3 does: it is planar, follows 3857, and leaves the poles out. The planet is larger than that plane, and something has to be drawn past every tile — the sky, the sun and moon, and the ground beyond what the terrain reaches. What decides whether a thing of that kind is a layer is not where it is but whether there is anything to hold.

- **Stored data** has extent and is held at resolutions, so it is a layer of the pyramid, refined by the eye's rule like any other. The Earth's picture is one.
- **An evaluated model** is computed per pixel from a few parameters, and has nothing to hold at a resolution: the sun, the moon and the stars by the wall clock and the anchor, the scattering by its tables, the clouds by their field. None of them needs a pyramid.

The **earth** layer is the planet's picture, reflectance with its cities' lights in alpha (`pxl-spec bake/tile/earth.rs`). Only its top zoom is baked, a ground cell a texel, and only the floor's tiles about the eye are held (`layer/earth.rs`), so it brings no demand, zoom or schedule of its own. Where no tile is held, a global picture answers, eight times coarser, read at the point's latitude and longitude. The two are not copies of one fact but two domains, the plane and the sphere around it, and the one rule for which answers is the window the kernel hands the GPU: a tile's layer where one is held, zero where the global picture answers (`earth_window`).

The **atmosphere pass** draws the air as the frame shows it (`frame/shader/atmos_view.wgsl`): the sky, the ground past the terrain, and the aerial perspective over both, in one pass. Ground past the terrain is where a view ray meets the analytic sphere (`meets_land`), not a mesh, so the horizon and the curvature are exact. It is shaded from the earth layer, a held tile taking over from the global picture over the octave where a pixel goes from two ground cells to one, and lit by the terrain's own light so the drawn edge is no seam; the ground's bounce into the sky takes the mean albedo, not the tiles. The terrain, nearer, is the voxels' alone, and the two meet over the octave at the earth layer's reach.

The **clouds** are three layers over the planet — a cumulus slab marched, altocumulus and cirrus sheets read once — drawn at half the screen's size and laid over the opaque world. They write no depth: from below they fill only the sky, from above they cover everything, so a peak above a layer is wrong either way, a cost taken knowingly.

## Badges and the sheet

A badge is a billboard: it stands at a cell's center in the world and is drawn flat to the camera. Where its records come from is the view's (Marks); how it is drawn is this.

Badges are one instanced quad (`billboard/field.rs`) drawn from one sheet (`pxl-render gpu/sheet/shelf.rs`, one texture the badges are packed into shelf by shelf): anchor, size and sheet rectangle per instance, the fit to screen pixels and the anchor's pinning in the vertex shader. Occlusion is all or nothing per badge: the pass (`frame/shader/badge.wgsl`) binds the world's depth as a texture, reprojects several points up each badge through it, and discards the badge when most are behind something. The same pass writes the fragment's depth (`@builtin(frag_depth)`) from an upright twin of the quad, so badges sort against the world along world-up. Chat bubbles are drawn by the same pass.

An entry of the sheet is drawn on the GPU once, when what it shows changes (`gpu/sheet.rs`, `frame/shader/sheet.wgsl`): its shapes — rounded rectangles, circles, rings, a bubble's tail — as signed distances, and its text as lines the platform drew, into a texture of the entry's size copied into its rectangle. Text is asked of the platform a line at a time (Below the view), so kerning, joining, bidirectional runs, fallback and emoji are the platform engine's on every target: a tab's canvas, an iPhone's CoreText, Android's `Canvas` ([the sheet on the GPU, and text by the line](./decision/20260926-the-sheet-on-the-gpu-and-text-by-the-line.md)).

A marker someone puts up is drawn by the same pass, beside the places: the object layer's marker kind (`spec/spec/block/id.json`), read through their query in each drawn chunk and shown whether or not the place layer is on. Its words are one for every marker until each keeps its own ([BACKLOG.md](./BACKLOG.md)).

---

# Making the data

Everything the view reads of the world arrives as a published version of baked tiles, and every tile in a version is a function of two things: the data the database held when it was baked and the code that baked it. This part follows the data down one chain, each stage reading only the one before it: a provider's release into the geo database, the database into leaves, the leaves into the pyramid, the pyramid into a version, the version into the bucket the client fetches from.

```text
provider release ──ingest──▶ db-geo (+ catalog) ──leaf bake──▶ z15 leaves
                                                                   │
                                                          fold, no DB
                                                                   │
client residency ◀── bucket (Worker) ◀── publish ◀── version (z15 … top)
```

## A shard is the unit

The world is ingested, baked and grown a **shard** at a time: a z8 tile of the pyramid (`SHARD_ZOOM`), named by its Morton key. The coverage is the set of shards the bakes publish (`infra/geo/coverage.json`, read by `pxl-geo-bake coverage.rs`). A shard holds everything a leaf inside it reads, so a shard's leaves bake from it alone: what a leaf reads past its own edge — a structure's neighbours, the widest drawn line, the elevation's pad, and what it reads through a neighbour's own derived data — stays within the 2-ring, and every bake reads no farther past its leaf than one **reach** (`BAKE_REACH`, a z13 tile); that reach and a ground cell for rounding are the shard's **apron** (`SHARD_APRON`). How the databases hold shards is [infra/geo/README.md](./infra/geo/README.md); what matters here is that every stage below names its work by shards and never by a hand-drawn box.

## The database

Three sources go in by three ingests, none compiling the model: the Overture types, a table each, the elevation, one raster, and the coast, whole (`infra/geo/overture/ingest.rs`, `infra/geo/elevation/ingest.sh`, `infra/geo/coastline/ingest.sh`). An Overture feature is one row, keyed by its `gers_id`, however many shards it meets, and the catalog alone says which shards are in and at which release. A run adds the shards it is given and leaves what a table holds already; a whole run (`--whole`) is filled beside the table and swapped in, never emptied first ([infra/README.md](./infra/README.md) §2). Which to run, and when, is the operator's to say: the catalog records each shard's release and when it was ingested, and decides nothing. The elevation is warped a shard at a time onto one grid of blocks, its apron taken out to whole blocks, so a block two aprons share is the same samples and is stored once.

What the database holds at a moment is its **holdings**: per table the catalog's release and its counts, and the elevation (`coverage.rs` `holdings`); a set's manifest says it once. A bake refuses to start unless the catalog holds every shard it needs, none past the coverage.

## A leaf

A leaf is baked from its shard's rows and nothing else (`terrain/leaf.rs` `bake_leaf`). What is read is what the paint policy consumes: the policy's own tables name the classes a read asks for (`overture/policy.rs` `LAND`, `INFRASTRUCTURE`), and the deck solve asks only for the waterways it clears, so a large island polygon the policy drops is never fetched. Rows become features as Carto maps the OpenStreetMap tag each class comes from, and are painted in Carto's order — its layers, landcover, water, highway areas, roads, aeroways, but buildings over the roads; within a layer its own sort, a larger area first or a road's `z`; then the source's id, so no row order decides a cell (`terrain/pipeline/feature.rs` `Rank`). A cell is painted by what covers its center, and only what is seen from above: a tunnel, an indoor passage, a way not built or gone and a ferry's route are left off it. Why a cell is a Carto look and not channels of what the land is used for is [the ground is a Carto palette](./decision/20261005-the-ground-is-a-carto-palette.md). The coastline is OSM's land polygons, not a provider's water, and it alone says where the sea is: the sea is painted where they do not reach, landcover never paints it, and a leaf is baked only where they, a building or a way reach it (`terrain/repository.rs`, `terrain/leaf.rs` `has_ground`).

Bridges and tunnels are solved as Arnis solves them, structure by structure, from a tile and as much of the eight around it as its structures reach (`pxl-geo-bake separation/arnis.rs`, [as Arnis solves them](./decision/20261006-bridges-and-tunnels-as-arnis-solves-them.md)); why a deck is a line in the terrain tile and not a lane or an object is [ways off the ground](./decision/20261001-ways-off-the-ground.md).

Rasterizing is the bulk of a leaf: polygons are filled even-odd with holes, and lines are drawn with a thickness kernel. Elevation is stored as signed 16-bit cell units under one Mercator Z scale per tile, which is what makes the bake, the terrain and the object layer agree on what a unit of height is. A tile is written as XTP, xpute's tree, in the form the client reads where it lands, so nothing is translated on arrival.

The elevation lane is sampled from the raster at the leaf's resolution, its pad included: the apron reaches past every pad, so a pad is always the source and `0` is the source's own sea, never a shard nobody ingested. The interpolant the client draws, its bound and the separation's footing are one function the bake calls too, so a leaf's index lane bounds exactly what will be drawn ([the pad at a shard's edge](./decision/20261007-the-pad-at-a-shard-s-edge.md)).

## The pyramid

Every tile above the leaves is folded from the zoom below with no database in the loop, by the layer's one-zoom fold (`pxl-spec bake/fold.rs`): the ground by majority, the elevation by an average whose pad reads the neighbours' inner samples, the index by joining the children's lowest and highest, the place layer by keeping what shows from the parent's zoom on. A tile beside the four being folded, whose inner samples a pad reads, lends nothing until it is baked; the pad repeats the edge until then.

Not every tile is an object. A leaf with nothing drawn in it is never baked, and a tile the bucket lacks is drawn from the one over it, so open ocean costs nothing. A tile with no object folds as what it stands for (`pxl-geo-bake terrain/absent.rs`): sea inside the coverage, and past it unknown, sea or land, which gives no surface, so the planet's picture is seen under it as it is past the pyramid.

A bake folds again only what its shards change: at each zoom the parents whose sources read a changed tile, each staged only where its content differs, until a zoom changes nothing (`pyramid.rs` `fold`).

## A version

A bake keeps one **set** a kind, every zoom of the pyramid flat under the tiles' names and never pruned, with a manifest of what it was made from, and changes it a shard at a time, as the ingest does (`pxl-geo-bake tile/shard.rs`): the named shards' leaves are staged beside it, each marked whole with the leaves it no longer makes; only what they change is folded again, a neighbour's edge band included, up to a zoom where nothing changed; and the staged tiles are moved in only once folded, each step able to stop and run again. A change of source, code or coverage is a purge by command. A **version** is the set uploaded under a name of its format and the day (`spec/spec/bake/published.json`), immutable once written, less where a layer is pruned the tiles no reader asks for. Which shards to bake, and when, is the operator's to say.

## The dev tile server

In development the same bake runs on demand (`pxl-geo-bake demand/`), every pyramid layer the same way (`demand/pyramid.rs`): a leaf baked when it is first asked for, a tile above folded as the offline bake folds, no more than `--folds` zooms over the leaves, the one guard every layer shares. Its cache is kept across restarts, a kind a directory; a re-ingest or a bake change is a purge by command. With `--shelf` it serves each kind's set as it is, so a bake is looked at before it is published. The development tasks are named for where the tiles come from, as a bakery's bread is: `dev:oven` bakes them on demand, `dev:shelf` takes the last bakes, and `dev` has them delivered from the published deployment.

## Publishing

A version reaches the client through one private bucket read only by the Worker (`.agents/skills/publish-bucket/SKILL.md`, [deployment hosting](./decision/20260816-deployment-hosting.md)). The client asks for the published version's tiles by name.

## Catalogs and assets

Everything a build ships is decided at build time, as JSON under `spec/` grouped the way `crates/` is. `pxl-tool` writes a Rust module from each file and, where the shell reads it, a TypeScript one; `deno task spec` writes them and `check:spec` fails when the tree disagrees with the JSON. That covers the commands and their packets, the cell catalogs, the memory map, the GPU API's enums, the profiler's layout, the settings schema, the decor and character catalogs, the routes and the published versions.

A catalog is never sent. Both sides compile it, so a message carrying it would be a program telling itself what it already knows. Where bytes do cross — a published tile, a saved blob — the layout is the model's and the version is in the envelope: the path, or the storage key.

- **Cubes and decors** are numbers in `spec/spec/block/` (`cube.json`, `color.json`, `decor.json`), and the number is what the object store writes.
- **Models** are named by path under `asset/` in the catalog that uses them — each decor kind's variants, a model and a skin each (`spec/spec/block/decor.json`), each character group's `keys`, `models`, `skin` and `size` (`spec/client/kernel/scene/character/variant.json`). A path resolves to a bucket key and a measured extent through the asset table `pxl-tool`'s ingest writes (`pxl-spec asset/ingested.spec.rs`, `apps/web/src/catalog/asset/ingested.spec.ts`), naming every model, skin and texture by its own path with a hash of its bytes in the stem — `asset/actor/kenney_cube_pets/beaver-20f91809e69c.glb`, the way a bundler names a chunk. Nothing imports a model or an image; the client fetches a key, the same key from `./out/asset` in development and from R2 in production.
- **The texture pack** is one table (`pxl-spec asset.rs`: which image a cube wears on which face) and one WebP image in the bucket. A look's color is `color.json`'s; a textured look is drawn from afar as its tile's mean, so the ingest draws each top toward its look's color as it composes the pack (`pxl-tool asset/pack.rs` `TOWARD_CARTO`), and the map reads as Carto's either way.

# Prior art

What the tree took from work done before it, or found it had arrived at: each line names the precedent, where the tree follows it, and what it takes. A link is given where the tree records the source; the rest are named as the code names them.

| Precedent                                                                                                                              | Where                                                                                                         | What is taken                                                                                        |
| -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Google Earth and the web map's tile pyramid                                                                                            | the pyramid (`pxl-spec space/`)                                                                               | Structure, storage, tiling and invalidation across zooms                                             |
| Cesium 3D Tiles, `skipLevelOfDetail` and `preloadAncestors`                                                                            | `pxl-kernel io/demand.rs`                                                                                     | A tile tree whose nodes arrive out of order; a held ancestor shown at once                           |
| Morton order                                                                                                                           | tile and shard names (`pxl-spec space/hierarchy/morton.rs`)                                                   | One base-4 digit a zoom, coarsest first                                                              |
| Web Mercator (EPSG:3857), WGS 84                                                                                                       | `pxl-spec space/coord/`                                                                                       | The plane PVF3 follows; the sphere's radius                                                          |
| Mapbox's globe                                                                                                                         | `pxl-spec space/coord/globe.rs`                                                                               | Mercator laid on the sphere in the center's frame                                                    |
| Cozzi and Ring's horizon cull                                                                                                          | `pxl-kernel chunk/table.rs`                                                                                   | A point is over the horizon while E·P ≥ r²                                                           |
| Earth Engine's `MODE` pyramiding                                                                                                       | `pxl-spec bake/fold.rs`                                                                                       | A 2 × 2 majority fold of the zoom under it                                                           |
| Distant Horizons                                                                                                                       | `pxl-render mesh/paint/palette.rs`                                                                            | A distant cell drawn as the mean of what it folds                                                    |
| Minecraft's `cube_all`, `cube_bottom_top`                                                                                              | `pxl-render gpu/layers.rs`                                                                                    | Which face image a cube wears                                                                        |
| Quake III's `PM_StepSlideMove` ([bg_slidemove.c](https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_slidemove.c)) | `pxl-kernel query/support.rs` `step_slide`                                                                    | Blocked, step up while open overhead, settle, slide along the wall; no step height but the ceiling's |
| Amanatides and Woo's voxel traversal                                                                                                   | `pxl-kernel query/ray.rs`, `query/support.rs`                                                                 | The next cell along a ray, by the share of the way to each axis's edge                               |
| OpenStreetMap Carto                                                                                                                    | `pxl-geo-bake terrain/pipeline/overture/policy.rs`, `spec/spec/block/`                                        | The ground's palette and paint order, way widths, place icons, colors and zooms                      |
| Mapnik's `text_line.cpp`, CSS 2.1 `line-height: normal`                                                                                | `pxl-render gpu/pass/badges.rs`                                                                               | A label's line pitch                                                                                 |
| OpenMapTiles' schema                                                                                                                   | `spec/spec/block/marker.json`                                                                                 | A marker shows from z14, as its `poi` layer does                                                     |
| [Arnis](https://github.com/louis-e/arnis)                                                                                              | `pxl-geo-bake separation/arnis.rs`                                                                            | Bridges and tunnels solved structure by structure: clearances, ramps, decks, pillars                 |
| osm2streets, osm2lanes                                                                                                                 | the bake's way rules ([decision/20261001-ways-off-the-ground.md](./decision/20261001-ways-off-the-ground.md)) | Lane and buffer widths                                                                               |
| Overture's schema                                                                                                                      | `pxl-spec division.rs`, `pxl-geo-bake terrain/pipeline/overture/between.rs`                                   | Places and divisions read as the schema defines them                                                 |
| Stable shadow maps, PCF                                                                                                                | `pxl-render gpu/shadow.rs`, `frame/shader/shadow.wgsl`                                                        | Texels snapped to a grid fixed in the world; a 3 × 3 filtered edge                                   |
| Bruneton; Hillaire (EGSR 2020)                                                                                                         | `pxl-render look/atmosphere.rs`                                                                               | The atmosphere's tables: transmittance and irradiance; multiple scattering and the sky view          |
| Unreal's SkyAtmosphere, OPAC, Cornette–Shanks                                                                                          | `pxl-render look/atmosphere.rs`, `frame/shader/atmos_medium.wgsl`                                             | The Earth's scattering numbers, the haze, the Mie phase                                              |
| ACES as Stephen Hill fits it                                                                                                           | `pxl-render frame/shader/finish.wgsl`                                                                         | One tone curve for every pass                                                                        |
| N8AO                                                                                                                                   | `pxl-render gpu/ao.rs`                                                                                        | Screen-space ambient occlusion, ported to WGSL                                                       |
| Roberts' R2 sequence                                                                                                                   | `pxl-render frame/shader/clouds.wgsl`                                                                         | The cloud march's dither                                                                             |
| Kasten and Young; Ballesteros                                                                                                          | `pxl-render frame/shader/stars.wgsl`, `gpu/stars.rs`                                                          | Airmass at the horizon; a star's color from B−V                                                      |
| Duffett-Smith                                                                                                                          | `pxl-kernel scene/environment/celestial_position.rs`                                                          | The sun's, the moon's and the stars' positions                                                       |
| Stöckli et al. (Blue Marble)                                                                                                           | `pxl-geo-bake bin/bake_earth.rs`                                                                              | The imagery's bytes read back as reflectance                                                         |
| three.js                                                                                                                               | `pxl-kernel scene/actor.rs`, `pxl-render frame/shader/atmos_view.wgsl`                                        | A clip's cross-fade; Lambert as its standard material                                                |
| Howard Hinnant's date algorithm                                                                                                        | `pxl-geo-bake util.rs`                                                                                        | A civil date from a day count                                                                        |
| Figma                                                                                                                                  | [decision/20260912-why-the-destination-is-wasm.md](./decision/20260912-why-the-destination-is-wasm.md)        | A compiled engine in the page over one memory                                                        |
| iOS's interface guidelines, Material's motion, SF Symbols                                                                              | `apps/web/src/shared/ui/theme.ts`, `shared/ui/motion/transition.ts`, `platform/web/icon.tsx`                  | Touch targets and type scale, easing, the icons' names                                               |
