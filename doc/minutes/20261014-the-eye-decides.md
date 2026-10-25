# The eye decides: why a ray a pixel, and not Nanite

**2026-10-14** — a discussion with the user. It decides nothing. It records
why the drawing came to cast a ray a pixel, why the usual alternatives were
left, and where the structure points next. The decision it explains is
[the GPU casts a ray a pixel](./20261014-the-gpu-casts-a-ray-a-pixel.md).

## Two orders of drawing

There are two ways to find what a screen shows.

- **Object order** walks the data. Each thing is projected onto the screen,
  and a depth buffer keeps the nearest at each pixel. This is rasterizing:
  Minecraft, Nanite and almost every game draw this way.
- **Image order** walks the screen. Each pixel's ray asks the world what it
  meets first. This is ray casting: sparse voxel octree renderers,
  GigaVoxels, Teardown, and pixelet.

For what the eye sees first, both give the same answer. They differ in what
the cost follows. Object order pays for every thing it walks, seen or
hidden, unless something else culls it first, and that culling is where
special cases pile up. Image order pays per pixel, for the steps of a
descent, whatever is hidden behind the first surface.

The first rule of the main axis AGENTS.md then held, "the eye decides", is
image order; ARCHITECTURE.md now opens with it. The rest of this record is
how the tree arrived there, one counterexample at a time.

## How the tree arrived

**August** (`checkpoint/20260810-longhorn-reset`). The ground was a
heightmap, one height and one look a cell, drawn as columns. Each chunk
chose its level of detail from its own distance, against a hand-tuned table
of thresholds, with hysteresis and a dithered blend between levels.
`66d741e91` (8/15) put the ground and placed objects into one voxel chunk.

**28 September to 12 October** (`checkpoint/20261012-eye-axis`). The
distance table went for screen-space error. A chunk was refined where a
block would show the eye more than one cell of the camera grid, a cell
being a fixed angle about the eye. What was drawn was a frontier
(`3cccd2cf7`): an antichain of the chunk tree that covered the view with no
gap, changed only by whole moves, with the coarser piece drawn until its
children were ready. Each chosen chunk was meshed whole, from column runs:
the heightmap's lineage, a column from the chunk's floor to its top, then
what stands on it. In the same weeks coordinates became relative to the eye
(`8096e81c6`), every layer came to answer one query of a node
(`7615b4c1c`), and no dependency was left running up the axis
(`f35d428cc`).

A chunk meshed whole draws what no eye can see. A heightfield chunk `n`
cells a side has a surface of about `n²` faces. Caves are hash-drawn
pockets through the ground (`rust/spec/core/src/caves.rs`), and their walls
grow with the volume, toward `n³`. A camera on the surface sees almost none
of them, and the chunk drew them all.

**13 October** (`47ec6b35b`, `9df215fe4`, `b08d01376`). One ray a cell of
the camera grid, down the pyramid. A chunk drew only the blocks rays stopped
on. The frontier, the runs and the column composition went. The ground came
in stripes, rows of rays with sky between them, and `480e6b8f9` proved why.
Rays sample points. Between two neighbouring rays on ground seen at a
grazing angle lie blocks that no ray stopped on, so they are never built,
and their place shows sky. Two conditions cannot both hold:

1. only what a ray reaches is built;
2. what is built is turned back into geometry and rasterized at more pixels
   than there are rays.

Building every block in a ray's footprint drops the first and brings back a
whole-chunk build. Casting a ray a pixel drops the second.

**14 October** (`5bdc893dc` and the decision above). A ray a pixel on the
GPU, with nothing turned back into geometry. The descent is one source,
`rust/spec/core/src/cast.rs`, compiled for the CPU and through rust-gpu to
WGSL. The camera grid still chooses the zoom a ray asks at each distance,
so the resolution rule did not change; only the number of rays did. A
pixel is not the final form of the rule, only its finest setting: one ray a
cell, with the cell shrunk to a pixel.

## The middle roads, and why each comes back to the same place

**Draw whole chunks, but let rays choose which.** Mesh every chunk a ray
reaches, and rasterize it. This fixes the stripes inside a chunk, and the
cost moves from every frame to each new chunk. It brings back three
things. A chunk takes turns to build, so something must be shown meanwhile:
three conditions — built over several turns, never a hole, no state beyond
the present view's chunks — cannot all hold, and keeping the old piece
until the new one is ready is the frontier again. That is a second layer
hiding latency, beside the one the tiles already have, where a coarser tile
stands in until its own arrives. Neighbouring chunks at different zooms meet
in seams. And everything inside a reached chunk is built, caves included.

**Draw only a chunk's cross-sections.** Mesh, for each of the six axis
directions, the first face met along each line through the chunk. That is
at most `6n²` faces whatever the caves hold. It leaves holes wherever the
ground is concave, even with no caves at all:

```
 z
10 |           B
 5 |  A        B
 0 |  A  .  .  B        x →
      0  1  2  3
```

A's wall facing +x is hidden along every x-line by B, and every z-line
meets the street's floor, so no axis line sees it first. An eye above and
beyond B looks down into the street and sees it. In a city of buildings
this is ordinary.

**Drop the air no outside air reaches.** Flood-fill the air from the
sky and the chunk's sides, and draw only faces on air that was reached.
That leaves no hole and removes sealed pockets, but a cave open at one
mouth is drawn whole. A pocket cut by the chunk's side is kept too, unless
the chunks share a graph of what connects to what, which is Minecraft's cave
culling. It also states a second time, without the eye, what is visible.

Together these give one fact. The visible surface of a convex thing can be
known from the thing alone. Where the ground is concave — a street, a
cave, a cliff — what can be seen depends on where the eye stands, and the
only exact way to put the eye in is a ray.

## Minecraft

Notch's first renderer (2009–2011) meshed 16×16×16 sections whole into
OpenGL display lists, dropped a face only where its neighbour was opaque,
and culled by frustum alone. Cave walls were drawn. It ran well because the
view was cut: about 256 blocks at its farthest, with fog over the edge. The
infinite world of February 2010 made generation infinite, not drawing.

What came later is the culling stack. Cave culling (1.8, 2014) gives each
section a table of which of its faces connect through air, and walks
outward from the camera's section through those connections. Sodium groups
each section's faces by direction and skips the groups facing away. Every
step answers a cost the whole-section mesh created. Each would grow again
here, where the view reaches the horizon, the pyramid gives chunks of every
size, and the caves are pockets.

## Why not Nanite

Nanite is object order done well. It selects clusters from a hierarchy by
screen-space error, culls them against a hierarchical depth buffer, and
rasterizes the smallest triangles in software. Its cost, like a ray's,
follows the screen rather than the scene. In principle a ray a pixel solves
the same problem with the same result, at pixels times the log of the scene;
for triangles the obstacle was building an acceleration structure over each
frame's choice of clusters.

It does not fit here, for two reasons.

- **Voxels are their own hierarchy.** A grid needs no structure built over
  it. Turning voxels into triangles multiplies the faces, and Nanite then
  builds a second hierarchy to undo that multiplication. Its simplification
  merges triangles, which blurs a block's edges at a distance, where pixelet
  folds a zoom's blocks into larger blocks instead.
- **It assumes geometry built in advance.** An edit means rebuilding the
  clusters and their simplification around it.

Building a voxel world on Nanite is what a team inside Unreal does, because
lighting, shadows and tools there all expect triangles. An engine written
for voxels can cast into the voxel hierarchy directly.

## The hierarchy, named from outside

A ray here does what a ray does in a bounding volume hierarchy (BVH): it
asks a node whether anything under it is filled, passes it whole if not,
and descends into the children it meets, nearest first
(`rust/spec/core/src/cast.rs`). Three things make up that hierarchy, and none
is built while drawing.

- **The tree** is the zooms: eight children a node, an octree whose nodes
  are addresses and are never stored.
- **The bound** says whether a node can be passed. The ground's is the
  lowest and highest it draws over a column, read from a square's levels,
  which the GPU folds over the square's columns as it lands, each coarser
  level the join of the one under it (`bound_cs`;
  `rust/spec/core/src/window.rs` `bound_over`, `rust/spec/core/src/ground.rs`
  `relief`). Outside this is called a
  min-max (or maximum) mipmap, the usual structure for casting rays at a
  heightmap. The kernel's query carries a box a node
  (`rust/client/kernel/src/domain/layer/query.rs` `Query::bounds`), as a BVH does.
- **The content** is the pyramid's tiles, folded at each zoom, which answer
  what a block is where a ray stops.

Object order's hierarchical depth buffer is the same shape turned around:

|                        | plane              | each cell holds                 | a test discards           |
| ---------------------- | ------------------ | ------------------------------- | ------------------------- |
| depth pyramid (raster) | the screen         | the farthest depth under it     | a thing farther than that |
| levels (ray)           | the world's ground | the lowest and highest under it | a node outside that range |

Raster keeps its pyramid in screen space and rebuilds it every frame from
the frame before. The ray keeps its pyramid in world space, and the GPU
folds it once, as a square lands (`bound_cs`).

What the rays need was then streamed the way GigaVoxels streams it: the
tiles a descent lacked were part of the observation, and demand followed
them (`view/sight.rs`, since removed). That no longer holds: demand is a
function of the eye alone, never of what the rays met
(`rust/client/kernel/src/domain/view/observation.rs` `ByEye`).

## Where it points

_Since carried out in part: the grid, the bound and the overwrites are the
GPU's (the decision's Status), the beam is
[a cell casts a beam](./20261022-a-cell-casts-a-beam.md), and the chunk no
longer draws (below)._

**Cost.** The decision records the price: at full resolution a few frames a
second, at half about thirteen. The descent itself is cheap. What costs is
each step's query, which reads a tile's samples and computes a height again
for every pixel at every step. The decision's order of work — the ground's
cell grid first, then the bound from it, the wall, the step's size, the
storage, the beam, and last what is laid over the ground — is aimed at
that, each step measured alone and reverted if it does not pay. If the whole does not pay,
the retreat is the first middle road, chunks chosen by rays, from
`checkpoint/20261012-eye-axis` with the runs gone.

> Since a square's columns are filled on the GPU as it lands (pxl-shader
> `heights_cs`), a step reads its cell's column, and no longer reads a
> tile's samples or runs the bicubic again.

**Two reductions in view.** Neither adds a mechanism; each hands a role
the rays already play to the rays alone.

- _Which tiles matter._ Fetching then followed the rays' lacks (now the
  eye alone, above), but evicting ranks tiles by whether a box around each
  is in the frustum and by distance (`rust/client/kernel/src/sys/io/demand.rs`
  `Order`, through `rust/spec/src/space/camera/eye.rs` `measure`). A tile
  behind a mountain is in the frustum and no ray reads it. GigaVoxels marks
  what the rays read and evicts what they did not; here the descent already
  counts the nodes it asks about (`c44070e06`). The same order also ranks
  every layer's work, which this did not look into.
- _The chunk._ What a chunk then did was draw the blocks the CPU's rays
  stopped on (`chunk/domain.rs`, since removed). The decision's last
  step moves what is laid over the ground — the separation, what is placed,
  decor — to the GPU's query, and then retires the CPU's drawing cast.
  After that a chunk has nothing left to do, and the axis loses a rung:
  scene, view, query, layer. Sparse voxel renderers have no chunk either,
  only pages of data and rays. Done but for the rung: a chunk draws
  nothing, and is now only the address by which what stands in the world,
  decor and places, is gathered about the eye
  (`rust/client/kernel/src/domain/chunk.rs`, `domain/view/observation.rs` `cubes`).

**The floor.** Below these, five things remain, and removing any one brings
its problem back: the tree, or the cost is linear in the world; the bound,
or no empty space is skipped; the fold, or there is no level of detail;
residency, or the planet must be held whole; and the ray's rule — the zoom
its distance asks, and a stop at the first block that fills — or level of
detail and visibility must be solved apart.

**Meshes among the voxels.** A model can stand in a cell — decor today —
and stay inside this structure on three conditions. It must stay inside the
cells it is registered in, or a ray passing the part outside never learns
it is there; a decor model can be larger than its box today
(`rust/spec/src/block.rs` `Decor::reach`). A ray must not stop at its cell
as if the cell were full, or the ground behind shows a cell-shaped hole;
decor already leaves its cell empty (`rust/spec/core/src/answer.rs`
`fills`). And the fold must say what it is
at a zoom whose block is larger than it. Within those, a mesh is a block of
an intricate shape. Decor today is not inside the rays: the GPU grows it and
draws it as a model or a sprite beside them
(`rust/client/render/src/gpu/grown.rs`).
