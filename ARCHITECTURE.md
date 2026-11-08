# Architecture — pixelet

pixelet draws the real Earth as cubes you can build on, in a browser tab and as
an app on iOS and Android. This document is for someone who wants to understand
how that can work, or who is about to change the code. It follows one idea
through the system, and ends with a map of where everything is.

## The problem, and the idea

Cut the Earth into cubes 2.4 meters wide and there are 2^24 of them around the
equator, and as many again north to south, before a single one of height. No
disk holds that, no network sends it in time, and no GPU draws it. Yet a phone
has to show any part of it, from a street corner to the whole planet, and let
someone stand on it and build.

The way out is that a world only has to exist as far as someone is looking, and
pixelet takes that literally. The eye sets the detail: every place is looked at
in cubes just large enough to fill one cell of the screen, so what a frame costs
is bound to the screen and not to the size of the world. Whatever looks asks the
same question, what fills this cube of space at this size, and one function
answers it, for the pixel's ray, the walker's feet, the pointer and the hand
that builds alike. What that function can compute is never stored: the ground is
baked small, trees and caves grow from a seed wherever they are asked about, and
only what people build is kept. What it does not know yet it never makes up; a
coarser answer stands in until the finer one arrives, so the picture only ever
gets sharper. And what the GPU has found once it remembers, so a frame's work
follows what changed on the screen.

The chapters below follow that idea in the order the system is built: the world
the question is asked of, the eye that asks it, the drawing that remembers the
answers, the core and the runtime underneath, and the data made ahead of time.

## The world

### Every size is a zoom, every position an integer

pixelet measures every size with one number, its **zoom**. X goes once round the
world in 2^24 units, a unit being one ground cube, and each zoom finer halves
the side: zoom 24 is a ground cube, zoom 15 a tile of 512 of them, zoom 0 the
whole world. Because every size is a power of two, a coarser grouping is a shift
of a finer one, and the groupings at every zoom line up exactly.

A position is a **`Q3`**: three 32-bit integers, each counting 256ths of a unit.
X fills its integer exactly, so walking east past the date line is the integer
wrapping round, and nothing treats it as a special case. Since a position is an
integer, it is the same bits on the CPU, on the GPU, on a server and in a bake,
and a block placed in one is in the same place in all of them. Only an adapter
at the edge knows what a unit is worth on the Earth;
`rust/spec/src/space/README.md` describes the zooms, the adapters and the keys.

The same zooms cut the plane into **tiles**, the squares that are fetched and
stored, and cut space into an octree of **nodes**, eight under each, which is
what a ray descends. The world is kept one **shard** at a time, a tile as wide
as the world is tall, and to a shard the octree is still the plane's quadtree:
its height has no digit until the zoom below.

### Baked and built

The world is stored as **layers**, and they split on who changes them.

The **base** is baked from the map sources and never changes within a version.
Its main layer is **terrain**: a ground cell is one byte, an index into
OpenStreetMap Carto's palette, beside an elevation from which the client works
out the heights, so heights are never sent. Bridges and tunnels come along as
lines with a top and a width. Beside terrain are the **places** Carto would
label, the administrative **divisions**, and a picture of the **earth** seen
from orbit. Every base layer but the earth is a **pyramid**: a tile at one zoom
is the four under it folded into one, by the layer's own rule. A faraway region
is drawn from a single coarse tile that already summarizes everything under it,
the way a texture is drawn from its mipmap, so a view from orbit reads a handful
of tiles, not the planet.

The **objects** are what people build. They are one sparse octree that keeps
only the leaves people filled; a branch holds no data, and when someone asks
what a branch looks like from afar its leaves are folded on the spot. A write
splits only the path down to its leaf, and eight equal leaves are joined back
into one, so one picture has exactly one tree. Placing air erases what the
ground would show.

```text
       the core                             the objects' source
  ┌──────────────┐   the top, first      ┌────────────────────────┐
  │ one tree     │ ◀──────────────────── │ a file for each shard  │
  │              │   a shard, when the   │ that holds objects     │
  │              │ ◀── view reaches it ─ │                        │
  │              │   edits, in order     │ applies them, rebuilds │
  │              │ ────────────────────▶ │ the top                │
  └──────────────┘                       └────────────────────────┘
```

The objects come from the **objects' source**, which keeps a file for each shard
that holds any, and a **top** that says what each of those shards looks like
folded. The top is read first, so a view from afar draws people's buildings
without reading one shard, and a shard the top does not list is known to be
empty without asking. Edits go up in the order they were made, and the source
applies them and rebuilds the top. Whether the source is a database on the
device or a server, the core cannot tell.

### What fills a cell

Whatever fills a cell is an **asset**, kept as its id. An asset has a kind,
which is how it is drawn (a cube, a model such as a tree, or a marker that faces
the viewer), and a source: the base made it, or someone placed it. The two are
independent, so a tree the ground grew and a tree someone planted are the same
thing to everything that draws, walks or points.

Trees and caves are never stored. Each is a function of the node it stands in
and the world's one seed, evaluated wherever someone asks (`pxl-engine seed.rs`
`node_seed`). A count of trees at a coarse node is handed down to its children
by weights a hash draws, exactly (`pxl-content share.rs` `halve`), so asking at
any depth grows the same forest, and the CPU and the GPU grow the same tree in
the same cell. A forest costs nothing to store and nothing to send.

A cell coarser than a ground cube is a summary of what it folds. It remembers
how much of its volume is filled, in eighths, and is drawn as a box shrunk to
that volume:

```text
   a cross-section of one coarse cell:      the box drawn in its place:
   eight cells, one filled (four shown)     half the side, one child cell

        ┌───┬───┐                              ┌───────┐
        │   │   │                              │ ┌───┐ │
        ├───┼───┤   fill = 1/8                 │ │▓▓▓│ │
        │▓▓▓│   │   ∛(1/8) = 1/2  ──────▶      │ └───┘ │
        └───┴───┘                              └───────┘
```

Every ray stops where it enters that box, whether it is the eye's, the sun's or
the pointer's, so a summary looks, casts a shadow and is pointed at alike. It is
still only a summary: nothing in it can be edited.

## The eye

### The camera grid

One **camera** looks: an **eye**, the direction it faces, and the **pivot** it
turns about, which is a point ahead of the eye for the orbit camera and the eye
itself in first person. The character stands at the pivot.

How finely each part of the view is drawn is set by the **camera grid**, a set
of angular cells about the eye, each the least the observer promises to tell
apart (`pxl-spec space/camera/grid.rs`). A cell is the same angle on every
screen, so what is observed at a distance is the same on every device; a display
only draws it. A node is drawn at the zoom at which it spans one to two cells at
its distance, so each zoom is drawn over a band of distance, and the bands
double going out:

```text
  bands of distance from the eye; s is the side of a node at zoom z, and D
  the distance at which such a node spans one angular cell

eye ●───────────┬───────────────────┬─────────────────────────────────────┬──▶
               D/2                  D                                    2·D
                │◀──── zoom z ─────▶│◀─────────── zoom z − 1 ────────────▶│ …
  an angular   s/2                  s                                    2·s
  cell spans
  a node spans    2 cells … 1 cell    2 cells … 1 cell
```

This is the rule the whole system leans on. Each band holds about the same
number of tiles, and a ray crosses one more zoom for each doubling of distance,
so the tiles held and the depth of a ray's descent grow with the logarithm of
how far the view reaches, and what a frame draws is bound to the screen's cells.

The world itself stays in its own axes; what turns is the observer. A draw holds
its position as a `Q3` and a small remainder in floating point, and the shader
takes the wrapping integer difference from the eye (`frame.wgsl` `from_eye`), so
precision is always spent where the eye is.

### The query, the only seam

Everything that looks at the world asks it the same question, and the **query**
is the only place that knows how a layer stores its answer
(`pxl-core domain/layer/query.rs` `Query`). The **world** composes the layers
into one answer for drawing and for control alike (`pxl-engine world.rs`
`node`), each written over the one below:

```text
  what is placed, air included            ┐ each is written
  the ways off the ground: decks, bores   │ over the one
  the air of the caves, under the face    │ below it
  the grid of the ground, cubes at height ┘
```

A ray descends the octree by one function that the CPU and the GPU compile alike
(`pxl-engine cast.rs` `first`). A node coarser than the zoom its distance asks
for is not what is drawn there, so it is asked only whether anything under it
might fill. If nothing can, the ray passes it whole; what a zoom folds away is
not there to miss.

```text
a node coarser than the zoom its distance asks for
   ├── nothing under it fills ─────────▶ passed whole
   └── something may, or it is not held ▶ split; the children are asked in
                                          the order the ray meets them
a cell at the zoom its distance asks for
   ├── it fills ───────────────────────▶ met: its pixels show it
   └── it is not known ────────────────▶ the ray stops there
```

Control asks the same world, one ray at a time (`pxl-core domain/query/ray.rs`
`first_hit`). So there is no collision mesh beside a drawing mesh to drift
apart: what you see is what you stand on and what you click. What a body stands
on and what an edit takes are the world's at its finest held zoom, not the
eye's: the walker stands on the first cell that fills under it, the orbit camera
keeps clear of what is there, and the pointer asks at the finest zoom and then
judges what it struck by the zoom the eye's rays draw there
(`domain/control/edit/pick.rs` `Pick`, `by_dist`), since an edit takes only
whole cells and a folded cell is a summary.

### Honest answers, bounded holding

An answer always says what it lacked: it is exact, or coarser along with the
tile it needed, or nothing along with that tile (`domain/layer/answer.rs`
`Answer`). Where the tile is missing, a coarser one answers in its place; where
none is held, the ray stops and says nothing past it. Nothing is invented.

What the view fetches is **demand**: for each zoom, every tile within that
zoom's band about the eye. It depends on the eye alone, never on what the rays
met, and each layer holds what is demanded in its own **tile cache**
(`domain/layer/tile_cache.rs`), which keeps four promises. Every observation has
an answer, because the coarsest zoom is always held all round the eye, as the
**floor**. The tiles held for the floor and for demand are counted at compile
time and fit the cache's slots with one to spare (`TileCache::SLOTS`), so memory
has a bound no view can break. A region only ever goes from unresolved to coarse
to as fine as it is observed, never back. And nothing is prefetched: a view
converges on its misses alone.

The ground's cells are held in a **clipmap** (Tanner, Migdal and Jones 1998):
one level per zoom about a center, each a few **bricks** wide and addressed
modulo its size, so the level follows its center a brick at a time and no held
cell moves:

```text
  one axis of a clipmap level, drawn as 8 bricks; brick x is held in entry
  x mod 8

  brick x     12   13   14   15   16   17   18   19
            ┌────┬────┬────┬────┬────┬────┬────┬────┐
  entry     │ 4  │ 5  │ 6  │ 7  │ 0  │ 1  │ 2  │ 3  │
            └────┴────┴────┴────┴────┴────┴────┴────┘
  the center moves a brick east: brick 20 enters and takes entry 4, which
  brick 12 leaves, and 13 to 19 stay where they are
```

A tile is cut into its bricks as it lands, and the bricks are filled on the CPU
over the following turns, a part at a time within the turn's share, with a
min-max pyramid over their heights and their bridges and tunnels as a tree. All
of it is uploaded, so the CPU's question and the GPU's ray meet the same bridge.
Where a level holds no brick, the coarser level's ground is read, and drawn, in
its place.

Every bound in the view has an owner: the camera grid bounds what is drawn,
demand bounds what is fetched, the slots bound what is held, and a turn's time
bounds the work done at once. The numbers a person would tune are none of these.
Set to their least, the view still converges, only coarser or later.

## Drawing

### Rays find, a raster draws, a walk completes

Each drawn pixel is, in principle, one ray. Neighboring pixels share most of
their descent, though, so the GPU walks the shared part once for each angular
cell and lets a raster finish the job:

```text
  one angular cell of the camera grid
  ●─────────────●   beam rays at the corners walk the descent that the
  │ ·  ·  ·  ·  │   pixels of the cell share
  │ ·  ·  ·  ·  │
  │ ·  ·  ·  ·  │   samples walk on from there, and each stops on the
  ●─────────────●   first voxel it meets

  each pixel ──▶ the nearest box its own ray enters (raster.wgsl)
                   ├── a fresh voxel ──────────────▶ takes it
                   ├── none, and no sample met
                   │   anything ───────────────────▶ the sky
                   └── else, a hole ───────────────▶ walked (queue.wgsl)
```

The rays **find**: a node a ray stops on is a **voxel**, the world's own answer.
The raster **decides** what each covered pixel sees. Every box it draws is a
voxel itself, and a face keeps a pixel only where the pixel's own ray enters the
box, so the depth test keeps exactly the nearest voxel that ray meets. A walk
**completes**: a pixel that no fresh voxel answers is a **hole** (a strip
narrower than the samples' spacing, at the edge of whatever hides it, or a voxel
whose data changed, or one the GPU does not hold yet), and its ray walks on to
the first thing it meets. The last step has to be a ray, because a hole has no
candidate to draw: a raster would need the cells that might lie there listed
first, and a list cannot stop at the first thing it meets in a world that is
solid below its surface.

### What was found is remembered

What the rays asked of each node is kept between frames in the **node cache**
(`pxl-engine node_cache.rs`), and forgotten where the data a node read changed,
or given up for room once it has not been drawn for two frames. The voxels in it
are the **record**, and the raster draws every one in view each frame, whatever
that frame's rays reached (`pxl-engine record.rs`). So a voxel found once is
drawn until its data changes, and rays walk only where something is newly seen.
A still camera over a still world costs almost nothing, and a frame where
nothing it reads moved submits no work at all.

When the GPU runs out of time anyway, two knobs give it room: the **scale**
draws fewer pixels, and the **rate** spaces the turns further apart. The `auto`
setting steers both against the frame's GPU time, as congestion control steers a
connection.

### The sun is a second eye

Shadows come from a **shadow map**, the depth of the world as the sun sees it in
a square box about the eye (`gpu/shadow.rs`). The sun is an observer like the
eye, by the same rule and the same passes. Each tile of the box asks the world
at one zoom, its **band**: the zoom the eye's rays would ask at the point of the
tile's column nearest the eye, measured from the box's middle
(`pxl-engine light.rs` `Light::band`). A band depends only on where the tile
lies in the box, so when the eye moves, the box slides a tile at a time and
every tile keeps what it cast; only a tile that slid on, or into another band,
is cast again. And the sun casts only what something seen will read:

```text
  each voxel the eye sees, and each pixel it walked  ┐
  decor, where it is placed                          ├─▶ the tiles it stands
  a character, where it is drawn                     ┘   on, within the reach
                                                         of the effects
                                                                 │
            a tile is read if it lies sunward of one stood on,   │
            within the height of the tallest decor  ◀────────────┘
                                 │
                                 ▼
            the tiles read anew are cast; the others stand
```

The sun keeps its rays for the same reason the eye's holes do. A raster would
need every cell under a tile listed, and the ground is solid as deep as the box
reaches; a ray stops at the first thing it meets, so only what is seen costs.

### The GPU gives the CPU's answer

The GPU runs the same rules as the CPU, rewritten by hand in WGSL, and both have
to give the same answer, or the world would be drawn where it cannot be walked.
A test holds every compute pass over the world to the engine's Rust, word for
word, over a real world's buffers (`rust/client/render/tests/`). Positions are
integers on both sides, and any value a rule would divide by or take the root of
is rounded once on the CPU and handed to the GPU, so the two never round
differently. The build checks every shader with naga, and a test compiles each
with Tint, the compiler a browser runs.

### What is not a layer

The sky, the sun, the moon, the stars, the scattering of the air and the clouds
are not stored either: each is evaluated from a few parameters, and drawn again
only where a change would show on the screen. The sky is drawn only in pixels no
ray met. Markers, the labels and pins that face the viewer, are drawn once into
an atlas and kept there until what they show changes, and their text is laid out
by the platform, so each platform shapes its own scripts and emoji.

## The core and the runtime

The world lives in one Rust **core**, `pxl-core`, which draws through
`pxl-render`. It owns no device. It is linked with `xpute-core`, a small library
OS, into one image, and a **runtime** around it plays the virtual machine: it
gives the core one memory, a GPU, its files and its time, and nothing else
reaches in. On a page the image is a WebAssembly module and the runtime is
TypeScript in a worker; on a device it is a library linked into the app and the
runtime is native. The core cannot tell which, and does not need to, so the same
world runs everywhere. xpute's own documents describe that machine; this one is
about what pixelet does on it.

The core is given its time in **turns**. Each turn is paced to the display and
arrives as a grant that carries its budget and how many reads it may start
(`turn.rs` `TurnGrant`). A turn runs one task in one order: the effects of the
last one, the tick's phases under the budget, then the frame; a frame the GPU
cannot take yet is not begun. The core's memory is laid out once at boot, in
sections that nothing grows past (`sys/mem/section.rs`), so a long session has
the same footprint as a short one.

Reads are asked for, never taken. Each turn every stream of reads declares
everything it wants now, and one pump starts the best of them on the credits the
grant gave, ranked by what the view needs first (`sys/io/request.rs` `declare`,
`pump`; `policy.rs` `rank`). A read nobody wants any more is stopped in the same
turn, so a zoom the camera only passed through holds no credit. When a read
fails it keeps its want, and when the network or the server fails, every start
waits while one probe goes out, after a growing, jittered delay (`policy.rs`
`backoff`). Meanwhile the view draws what it has, coarser, as it always does.

The UI sits outside the image. It never computes a coordinate or a tile: it
sends the core commands and draws the snapshot the core sends back, and its one
way in is `ts/client/src/service/core/`, through stubs generated from one
specification, so the two cannot disagree about a field.

## Making the data

The base is made ahead of time, so almost nothing is computed when someone asks.
A bake reads the geographic database one shard at a time, writes the finest
tiles, the **leaves**, and then folds each coarser tile from the four under it,
by the layer's rule. It bakes only what the client cannot compute: trees and
caves are not baked, because they grow from the seed wherever they are asked
about.

A bake goes through its steps (leaf, fold, commit and publish) in a staging
directory of its own, and any step can stop and run again without spoiling the
set; a commit refuses to run before the fold. Publishing uploads a new version
and only then points the bucket's declaration at it, which every client reads at
boot. A published version never changes, and an asset is named by a hash of its
bytes, so a name never comes to mean something else, everything can be cached
for good, and going back is serving the older version again.

## Boundaries and invariants

Some of what holds the system together is an absence, and easy to break without
noticing:

- The ground is stored as 2.5D, but nothing behaves as 2.5D: every reader asks
  the 3D query, so caves, bores and placed objects are seen alike.
- Only the query reads a layer. Everything else asks the world, and the
  check-rules tool refuses a layer read anywhere else
  (`rust/tool/src/bin/rule.rs` `LAYER_READERS`).
- The UI computes no coordinate and no tile, and reaches the core only through
  the generated commands and the snapshot.
- Nothing under `xpute/` knows pixelet: no tile, no world, no pixelet name.
- The CPU and the GPU give the same answer to the same question, held by tests
  word for word.
- A tunable number is never a correctness bound: at its least the view still
  converges, only coarser or later.
- Nothing is prefetched, and nothing goes from fine back to coarse.
- No decision reads the profiler, which sits beside the core and may read
  anything.

## Where things are

The core is one order of dependency: an import may skip a level but never points
up, and the folders are that order. The check-rules tool holds it.

```text
  turn · door · ui · pointer     the top: wires everything
  ────────────────────────────────────────────────────────────────────
  domain/                the world and the eye

    scene/       what this frame draws          ┐ two heads; neither
    control/     moves the eye, asks the query  ┘ names the other
    setting/     the settings both heads read, with the shared
                 smoothing (smooth.rs) and parameters (param.rs)
    view/        the pose, the lens, demand     ┐
    query/       the only seam                  ├ the trunk
    layer/       the base and the objects       ┘
  ────────────────────────────────────────────────────────────────────
  sys/                   what all of it stands on
    io/          reads and their scheduler
    sched/       a turn's edge and its phases
    mem/         the memory's sections
    boot · load  bringing it up and what it loads

  beside: perf/, the profiler, which reads anything and decides nothing
```

The rest of the tree is grouped by language, and within a language by where a
thing runs:

```text
  rust/spec/            pxl-spec, the model every crate shares
  rust/spec/engine/     pxl-engine, what the CPU and the GPU both compute
  rust/spec/content/    pxl-content, the content: decor, caves, clouds
  rust/client/core/     pxl-core, the core
  rust/client/render/   pxl-render, the renderer and its WGSL
  rust/server/          the servers and the bakes
  ts/client/            the app: the UI, its services and the runtime
  ts/server/edge/       the Cloudflare Worker in front of everything
  spec/                 JSON both languages' code is generated from
  xpute/                the system under the core, its own repository
```

How a part works in detail is written beside its code:
`rust/spec/src/space/README.md` for sizes and positions, and a README in each of
the renderer, the core's layers, `sys/`, the bake and the client.
