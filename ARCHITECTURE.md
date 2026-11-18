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
pixelet takes that literally, by two rules.

**What is seen is sampled.** The eye looks through a grid of cells of one angle,
so every place is looked at in cubes just large enough to fill one cell, and a
change too small to tell apart is no change: what a frame costs is bound to
the screen and to what changed on it, not to the size of the world.

**The world answers at every size, and falls back on the coarser answer and the
older picture until the finer and the newer are ready.** Whatever looks asks the
same question, what fills this cube of space at this size, and one function
answers it, for the pixel's ray, the body's feet, the pointer and the hand that
builds alike, with the id of what fills the cube, whoever put it there. What
that function can compute is never stored: the ground is baked small and folded
for every size, trees and caves grow from a seed wherever they are asked about,
and only what people build is stored. What it does not know yet it never makes
up, so the picture only ever gets sharper.

The chapters below follow that idea in the order the system is built: the
question and its parts, the drawing that remembers the answers, the core and
the host underneath, and the data made ahead of time.

## The question

Everything that looks at the world asks it one question, _what fills this cube
of space at this size_, and the two rules are what make it one a phone can
answer. This chapter first gives the cube and the size their measure, and then
takes each rule's parts in the sections the figure names.

```text
             what fills this cube of space, at this size?
                                 │
         ┌───────────────────────┴────────────────────────┐
 what is seen is sampled                 the world answers at every size,
         │                               and falls back on the coarser and
         │                               the older until the finer and the
         │                               newer are ready
         │                                        │
 in space, by the camera grid:           one answer, whoever asks
   the size a place is asked at            The query, the only seam
   The camera grid                         What fills a cell
 in change: a difference that            an answer at every size
   cannot be told apart is none            Baked and built
   What is seen holds until it           the fallback, and nothing made up
   can be told apart                       Honest answers, bounded holding
 so an answer is keyed by its              What is seen holds until it
   cube and its size                       can be told apart
   The query, the only seam
```

Each part is one rule that holds for every case alike: the same in every
direction from the eye, for what the base made and what someone placed, on the
CPU and on the GPU. What is understood of one direction, one source or one
side then holds of the rest, and a part can be reasoned about without reading
all of it. The design prefers that to a rule tuned for one case, and pays what
the one rule costs there.

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

### The camera grid

One **camera** looks: an **eye**, the direction it faces, and the **pivot** it
turns about, which is a point ahead of the eye for the orbit camera and, in
first person, the point under the eye where a body stands. The player's
character walks to the pivot.

**The eye alone sets the size a place is asked at, by the place's distance from
it in three dimensions, the same in every direction, and what was asked at a
size is the world's, valid wherever the eye moves.** No axis of the screen, no
facing, no platform and no light sets a size. Two forms read the world, and
there is no third: a ray of the eye's, which crosses many sizes and asks each
node at its own, and a read at one size, a body's cell or a tile of the sun's
light, which asks at the one size the eye sets for it. An answer tied to the
eye's exact place could not be cached, so no reader is given one. The size is a
zoom, one function of that distance (`pxl-spec space/camera/grid.rs`
`cell_zoom_at`), the same bits on the CPU and the GPU, and it is read off the
**camera grid**, a set of angular cells about the eye, each the least the eye
promises to tell apart. A cell is the same angle on every screen, so what is
observed at a distance is the same on every device; a display only draws it. A
node is drawn at the zoom at which it spans one to two cells at its distance, so
each zoom is drawn over a band of distance, and the bands double going out:

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

Each band holds about the same number of tiles, and a ray crosses one more zoom
for each doubling of distance, so the tiles held and the depth of a ray's
descent grow with the logarithm of how far the view reaches, and what a frame
draws is bound to the screen's cells.

The world itself stays in its own axes; what turns is the eye. A draw holds
its position as a `Q3` and a small remainder in floating point, and the shader
takes the wrapping integer difference from the eye, so precision is always
spent where the eye is.

### The query, the only seam

**One function answers the question, and the answer is the world's alone.** The
eye's one part in it is the size: a place is asked at the size its distance from
the eye sets, the same in every direction. So an answer is a function of its
cube and its size, and whoever caches one keys it by those and asks again only
when the world changes in what it read or the eye sets another size there.

A ray descends the octree by the same rule on the CPU and the GPU
(`pxl-engine cast.rs`). A node coarser than the zoom its distance asks for is
not what is drawn there, so it is asked only whether anything under it might
fill. If nothing can, the ray passes it whole; what a zoom folds away is not
there to miss.

```text
a node coarser than the zoom its distance asks for
   ├── nothing under it fills ─────────▶ passed whole
   └── something may, or it is not held ▶ split; the children are asked in
                                          the order the ray meets them
a cell at the zoom its distance asks for
   ├── it fills ───────────────────────▶ met: its pixels show it
   └── it is not known ────────────────▶ the ray stops there
```

Control asks the same world, one ray at a time
(`pxl-core domain/query/ray.rs`), so there is no collision mesh beside a
drawing mesh to drift apart: what you see is what you stand on and what you
click.

A body asks its cell at the size the eye sets where it stands, no finer than
what the world holds over the cell, and stands on what fills it. A ray across
what the eye sees, the pointer's or the orbit camera's way, crosses many sizes
and asks each node at its own, as a pixel's ray does, and the pointer judges
what it strikes by the zoom the eye's rays draw there
(`domain/control/edit/pick.rs`).

The **query** is the seam: the only place that knows how the answers are
stored (`pxl-core domain/query.rs`). The world is stored in layers, which the
next section describes; in the module tree the query is a child of the layers
(`domain/layer.rs`), what the terrain and the object layer store their answers
in is visible within the layers alone, and the compiler refuses any other
reader.

### Baked and built

**The world has an answer at every size.** What is stored is folded, the base's
tiles and the object tree below, and what is generated, a tree or a cave, is
generated in whatever node is asked ("What fills a cell").

The world is stored as **layers**, and they split on who changes them.

The **base** is baked from the map sources and never changes within a version.
Its main layer is **terrain**: a ground cell is one byte, the id of its asset,
a ground cube from OpenStreetMap Carto's palette or pixelet's own. Elevation
comes as samples from which the client works out each cell's, so a cell's
elevation is never sent, and bridges and tunnels come as lines with a top and a
width. Beside terrain are the **places** Carto would label, the administrative
**divisions**, and the **imagery** of the Earth seen from orbit. Every base
layer but the imagery is a **pyramid**: a tile at one zoom is the four under it folded
into one, by the layer's own rule. A faraway region is drawn from a single
coarse tile that already summarizes everything under it, the way a texture is
drawn from its mipmap, so a view from orbit reads a handful of tiles, not the
planet.

**Every kind of tile goes through the same demand and tile cache code**
(`pxl-core domain/view/demand.rs`, `domain/layer/tile_cache.rs`): the same code
asks for its tiles, receives them, caches them and answers a missing one by a
coarser tile. A kind differs in what its tile holds, in how four fold into one,
and in what it asks for: its ask scope, which of demand's four reasons, told
below, the layer asks by, and the tiles its readers pin. It differs in nothing
else. The imagery is the one layer that is no pyramid: it is baked at its top
zoom alone, and one global image answers wherever no tile is held.

The **object layer** is what people build. It is one sparse octree that keeps
only the leaves people filled and, in each branch, its children folded into one
word. A write splits only the path down to its leaf and folds again up it, and
eight equal leaves are joined back into one, so the same contents make exactly
one tree. Placing air erases what the ground would show.

```text
       the core                             the object source
  ┌──────────────┐   the top, first      ┌────────────────────────┐
  │ one tree     │ ◀──────────────────── │ a file for each shard  │
  │              │   a shard, when the   │ that holds objects     │
  │              │ ◀── view reaches it ─ │                        │
  │              │   edits, in order     │ applies them, rebuilds │
  │              │ ────────────────────▶ │ the top                │
  └──────────────┘                       └────────────────────────┘
```

The object layer comes from the **object source**, which keeps a file for each
shard that holds any, and a **top** that says what each of those shards looks
like folded. The top is read first, so a view from afar draws people's
buildings without reading one shard, and a shard the top does not list is known
to be empty without asking. Edits go up in the order they were made, and the
source applies them and rebuilds the top. Whether the source is a database on
the device or a server, the core cannot tell.

The **world** composes the layers into the one answer, for drawing and for
control alike (`pxl-engine world.rs`), each written over the one below:


```text
  what is placed, air included            ┐ each is written
  the ways off the ground: decks, bores   │ over the one
  the air of the caves, under the ground  │ below it
  the grid of the ground, cubes at height ┘
```

### Honest answers, bounded holding

An answer always says what it lacked: it is exact, or coarser along with the
tile it needed, or a lack, which gives no value, along with that tile. Where the
tile is missing, a coarser one answers in its place; where none is held, the ray
stops and says nothing past it. Nothing is invented.

What the view fetches is **demand**, and a tile is asked for by one of four
reasons: it lies within its zoom's band about the eye; it is an ancestor of a
missing tile, up to the coarsest zoom; it is **pinned**, the leaf over the pivot
or over a place the camera is sent to, with its ancestors, or a tile a reader
that takes no coarser answer pins; or it is of the floor, below. Demand follows
where those stand, never what the rays met, and each layer holds what is
demanded in its own **tile cache**, which keeps four promises. Every observation
has an answer, because the coarsest zoom is always held all round the eye, as
the **floor**. No view asks for more tiles than the cache holds, because what
the floor and demand can hold at once is counted at compile time against its
slots. A region only ever goes from unresolved to coarse to as fine as it is
observed, never back. And nothing is fetched on a guess: a view converges on its
misses alone, and a camera sent somewhere waits for the ground there.

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

The bricks are filled on the CPU once their tiles land, by a task of its own
between frames, and uploaded whole, so the CPU's question and the GPU's ray
meet the same ground, bridges included. Where a level holds no brick yet, the coarser
level answers, and is drawn, in its place. The layers' README tells how each
layer holds its tiles and how the clipmap is filled.

Every bound in the view has an owner: the camera grid bounds what is drawn,
demand bounds what is fetched, the slots bound what is held, and an entry's
end bounds the work done at once. The numbers a person would tune are none
of these. Set to their least, the view still converges, only coarser or later.

### What fills a cell

**Whatever fills a cell is an asset, kept as its id**: its kind over its index
in one 16-bit number (`spec/spec/asset/id.json5`), of which a ground cube's
fits the byte a ground cell stores. The kind is the asset's
shape, a cube, which is the cell itself, a **decor**, which is a model such as
a tree, or a marker that faces the camera, and there is no other. The id says
nothing of who put the asset there, the base or a person, so a tree the ground
grew and a tree someone planted are the same thing to everything that draws,
walks or points.

Trees and caves are never stored or sent. Each is a function of the node it
stands in and the world's one seed, evaluated wherever someone asks
(`pxl-engine seed.rs` `node_seed`). A count of trees at a coarse node is handed
down to its children by weights a hash draws, exactly (`pxl-content halve.rs`
`halve`), so asking at any depth grows the same forest, and the CPU and the GPU
grow the same tree in the same cell.

Decor is part of the answer on the CPU alone, since the GPU draws it as models
and its rays never ask for it: a leaf cell the layers leave void is the decor
whose box claims it, grown or placed (`pxl-core domain/query/decor.rs`). What
the eye sees and the pointer strikes include it; a body reads the layers alone.
A body shows, its character drawn and its walk walked, only where the eye
would draw a decor of its box; past that it goes where it is sent
(`domain/query/support.rs` `body_shown`).

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

### What is seen holds until it can be told apart

**The screen holds its last picture, and changes it only when a new one,
different enough to tell apart, is ready.** Its two conditions are the two
rules. Different enough to tell apart is the first, carried from space into
change: what is seen is sampled, the world by the camera grid, a motion by its
steps in time and the picture by the CSS pixel, which is a visual angle, and a
difference that cannot be told apart is no change. That decides what is drawn,
how often something that moves on its own steps, and when the screen is drawn
again. Ready is the second: the older picture is the fallback until the newer is
ready. A frame drawn for a difference that cannot be told apart, or a thing gone
before what replaces it is ready, breaks it.

- The camera moves in reals: its pose is control's, eased and turned by any
  amount. The **view** is that pose as the world is seen from it, taken in one
  place (`pxl-core domain/view/frame/pose.rs`): the eye at the lattice step the
  rays start from, and a turn once it moves the screen's corner by half a CSS
  pixel, half the picture's sample. Everything that looks reads the view, the
  screen's rays and the pointer's pick alike, never the camera.
- Something that moves on its own, a character, the clouds, the location's
  beacon, steps a clock by what its motion shows at its distance and wakes the
  frame at its next step (`pxl-core domain/scene/animation.rs`); that frame
  draws again only the screen's tiles the motion marked, whatever the view did.
- A marker stays until the ground over it is read again, as a coarser tile
  answers until the finer one is ready.

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
**completes**: a pixel that no fresh voxel answers is a **hole** (what the
samples' spacing missed, or a voxel whose data changed), and its ray walks on
to the first thing it meets. The last step has to be a ray, because a hole has no
candidate to draw: a raster would need the cells that might lie there listed
first, and a list cannot stop at the first thing it meets in a world that is
solid below its surface.

### What was found is remembered

What the rays asked of each node is kept between frames in the **node cache**,
and forgotten where the data that node read changed, or given up for room once
it has gone undrawn. The voxels in it are the **record**, and each frame the
raster draws every one in view that is still **fresh**, still the node the eye's
ray stops on, whatever that frame's rays reached. So a voxel found once is drawn
until its data changes or the eye's distance asks another zoom there. A frame
whose view moved walks the eye's whole lattice of rays again, and a walk asks
the world only where a node's answer is not held, so what such a frame pays for
is what it newly asks. A still camera over a still world costs almost nothing,
and a frame where nothing it reads moved submits no work at all. The renderer's
README tells how the cache forgets and gives up room.

When the GPU runs out of time anyway, two knobs give it room: the **scale**
draws fewer pixels, and the **rate** spaces the frames further apart. The `auto`
setting steers both against the frame's GPU time, as congestion control steers a
connection.

### The sun casts at the sizes the eye sets

Shadows come from a **shadow map**, the depth of the world along the sun's
rays in a square box about the eye. The sun's rays are walked through the same
passes as the eye's, at sizes the eye sets. The eye's rays ask each node at its
own size; a tile of the box asks its whole bundle of rays at one zoom, its
**band**, the finest the eye sets anywhere along the bundle. The box looks down
the sun's rays with the eye at its middle, so a tile's place in the box is its
bundle's distance from the eye, and a band depends only on where the tile lies
in the box. When the eye moves, the box slides a tile at a
time and every tile keeps what it cast; only a tile that slid on, or into
another band, is cast again. And the sun casts only what something seen will
read: what is seen within the reach of the effects marks the tiles it stands
on, and a tile is cast only where it lies sunward of one of those. The
renderer's README tells how the bands and the marks are worked out.

The sun keeps its rays for the same reason the eye's holes do: the ground is
solid as deep as the box reaches, and a ray stops at the first thing it meets,
so only what is seen costs.

### The GPU gives the CPU's answer

What the CPU and the GPU both decide, a ray's walk and the decor the ground
grows, is written twice: in Rust (pxl-engine, pxl-content) and again by hand in
WGSL. The two have to decide alike, or the world would be drawn where it cannot
be walked, so tests hold the compute passes over the world to the engine's
Rust, word for word, over a real world's buffers
(`rust/client/render/tests/`). Positions are integers on both sides, and any
value a rule of the world's state would divide by or take the root of is
rounded once on the CPU and handed to the GPU. A drawing's floats may still
part, and nothing reads them back.

### What is not a layer

The sky, the sun, the moon, the stars, the scattering of the air and the clouds
are not stored either: each is evaluated from a few parameters, and drawn again
only where a change would show on the screen. The sky is drawn only in pixels no
ray met. Markers, the labels and pins that face the camera, are drawn once into
an atlas and kept there until what they show changes. Their text is measured
and drawn by the platform, so each platform shapes its own scripts and emoji,
and the core breaks its lines.

## The core and the host

The world lives in one Rust **core**, `pxl-core`, which draws through
`pxl-render`. It is a pure guest of xpute: linked with `xpute-guest` into one
image, it owns no device and names no host, and what the app composes around it
implements what it imports: a GPU, its files, the clocks and the platform's
text. On the web the image is a WebAssembly module and its runtime a worker;
natively it is linked into the app with `pxl-runtime`, the native counterpart
of that worker, and xpute's native runtime runs it on a thread of its own. The
core cannot tell which, so the same world runs everywhere. Every interface
between the core and what implements its imports is declared in WIT, pixelet's
own under `spec/client/core/wit/`; xpute's own documents describe the link it
crosses, and this one is about what pixelet does over it. The GPU is pixelet's
plugin of xpute (`rust/gpu/README.md`).

The core runs only when the link enters it, and every entry has an end by one
rule: the margin before the next frame's start where the core knows that start,
and a slice from the entry's own start where it does not. A frame is asked for
only when something changed, and drawn at the display's next vsync as one task
in one order: the effects of what the UI sent, the tick, then the draw, and
none of it while the GPU cannot take a frame yet. The entry that draws a frame
does nothing else; other work, the ground's fill among it, is a task of its own
that works a part at a time in the entries between and yields at an entry's
end. The core's memory is laid out once, before its first entry, and never
grown.

Reads are asked for, never taken. Every frame each stream of reads declares
everything it wants now, and one pump starts the best of them, ranked by what
the view needs first, so a read nobody wants any more is stopped in the same
frame and a zoom the camera only passed through holds nothing. When the network
or the server fails, the core holds its reads and sends one probe at a time
rather than asking again for everything. Meanwhile the view draws what it has,
coarser, as it always does. `sys/`'s README tells how the memory is laid out,
how an entry is spent and how reads are scheduled.

The UI sits outside the image. It never computes a coordinate or a tile: it
calls the core's interfaces through `ts/client/src/service/core/`, over the
UI's end of the runtime and bindings generated from the same WIT, so the two
cannot disagree about a field, and it shows what the core tells it.

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
- Only the query reads the terrain and the object layer. Everything else asks
  the world, and the compiler refuses their reads anywhere else.
- Whatever fills a cell is an asset's id, and nothing that draws, walks or
  points can tell what the base made from what someone placed.
- No kind of tile is cached by code of its own: every kind's goes through the
  same demand and tile cache code.
- The core is a pure guest: it names no host.
- The UI computes no coordinate and no tile, and reaches the core only through
  `service/core/` and the bindings generated from WIT.
- Nothing under `xpute/` knows pixelet: no tile, no world, no pixelet name.
- The CPU and the GPU decide the world's state alike, held by tests word for
  word.
- A tunable number is never a correctness bound.
- Nothing is fetched on a guess, and nothing goes from fine back to coarse.
- No decision reads the profiler, which sits beside the core and may read
  anything.

## Prior art

What is pixelet's own is that the eye sets the detail and one query answers
whatever asks. The machinery under it is earlier work:

| Precedent | What is taken |
| --- | --- |
| Google Earth and the web map's tile pyramid | The pyramid: a tile at one zoom folded from the four under it, so a view from afar reads a handful of tiles |
| Morton order | Tile and shard names, each a prefix of its children's |
| Earth Engine's `MODE` pyramiding | The ground's fold: a parent's asset id is the majority of the four under it |
| Cesium's `preloadAncestors` | Demand: the ancestors of a missing tile are asked for, so a coarser tile answers wherever the view looks |
| Web Mercator (EPSG:3857), WGS 84 | The plane a position follows, one lap of X its whole width, and the Earth's radius |
| Quake III's `PM_StepSlideMove` ([bg_slidemove.c](https://github.com/id-Software/Quake-III-Arena/blob/master/code/game/bg_slidemove.c)) | The body: blocked, it steps up and slides along the wall as `PM_ClipVelocity` clips the velocity, with no step height but the ceiling's, and settles on the floor |
| Amanatides and Woo's voxel traversal | The ray's walk: the next cell along a ray, by the share of the way to each axis's edge |
| Laine and Karras, _Efficient Sparse Voxel Octrees_ (I3D 2010) | The ray's descent: push, advance and pop down the octree, and a split passed whole where the ray misses its contour, here a box |
| Laine and Karras's beams; Reshetov et al. | The beam rays: rays at a cell's corners walk the descent its pixels share, and the samples walk on from where the nearest stopped |
| Tevs, Ihrke and Seidel's maximum mipmaps | The clipmap's min-max pyramid: the lowest and highest under a node, which a ray above it passes whole |
| The clipmap (Tanner, Migdal and Jones 1998) | The clipmap, and the shadow map's box: a square about a center, each part held at its place modulo the side, so it follows the center and nothing held moves |
| Burns and Hunt's visibility buffer (JCGT 2013) | The raster: each pixel keeps the number of the nearest box drawn over it and reads the rest off that |
| Overture's schema | The divisions: a division is one of its subtypes |
| OpenStreetMap Carto | The ground's palette and paint order, way widths, and the places' icons and zooms |
| [Arnis](https://github.com/louis-e/arnis) | Bridges and tunnels: the bake's solve for their clearances, ramps, decks and piers, ported |
| Bruneton; Hillaire (EGSR 2020) | The air: the atmosphere's tables of transmittance and irradiance, multiple scattering, the sky view and the aerial-perspective volume |
| Unreal's SkyAtmosphere, OPAC, Cornette–Shanks | The Earth's scattering numbers, the haze and the Mie phase |

## Where things are

The core is one order of dependency: an import may skip a level but never points
up, and the folders are that order. The check-rules tool holds it.

```text
  link · frame · ui · pointer    the top: wires everything
  ────────────────────────────────────────────────────────────────────
  domain/                the world and the eye

    scene/       what this frame draws          ┐ two heads; neither
    control/     moves the eye, asks the query  ┘ names the other
    setting      the settings both heads read
    preview      an edit not yet made, which both may name
    view/        the lens, demand               ┐
    query/       the only seam                  ├ the trunk
    eye          where the world is seen from   │
    layer/       the base and the object layer  ┘
  ────────────────────────────────────────────────────────────────────
  sys/                   what all of it stands on
    io/          reads and their scheduler
    sched/       the frame's signal, an entry's budget, the tick
    sensor       the device's heading and location, as the UI tells them
    heap · range the memory, laid out once
    load         the files it loads

  beside: perf/, the profiler, which reads anything and decides nothing
```

The rest of the tree is grouped by language, and within a language by where a
thing runs:

```text
  rust/spec/            pxl-spec, the model every crate shares
  rust/gpu/             the GPU: pxl-gpu-link, -guest and -host
  rust/client/engine/   pxl-engine, what the CPU and the GPU both compute
  rust/client/content/  pxl-content, the content: decor, caves, clouds
  rust/client/global/   pxl-global, the global the core's state lives in
  rust/client/core/     pxl-core, the core
  rust/client/runtime/  pxl-runtime, the core as xpute's runtime starts it
                        natively
  rust/client/render/   pxl-render, the renderer and its WGSL
  rust/server/          the servers and the bakes, one crate a folder
  rust/tool/            pxl-tool, the generator and the checks
  ts/client/            the app: the UI, its services and its runtime
  ts/gpu/               @pxl/gpu, the GPU's link, web host and native module
  ts/server/edge/       the Cloudflare Worker in front of everything
  spec/                 the specs and WIT both languages' code is
                        generated from
  xpute/                the system under the core, a project of its own
```

How a part works in detail is written beside its code:
`rust/spec/src/space/README.md` for sizes and positions, `rust/gpu/README.md`
for the GPU, and a README in each of the renderer, the core's layers, `sys/`,
the bake and the client.
