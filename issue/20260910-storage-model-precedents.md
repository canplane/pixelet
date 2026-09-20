# Precedents for the storage model, and the shape they point at

**2026-09-10** — the reading pass that
[20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md)
said had to happen before anything was built. That document named three
families of solution "at the level of 'this is the family of solution to
look at'" and stated plainly that adopting them from its own description
would be reinventing them badly. This goes and reads them.

Part 1 is the reading, in the same shape as
[20260906-voxel-engine-industry-comparison.md](./20260906-voxel-engine-industry-comparison.md):
what the precedent actually does, what pixelet already matches, and where it
is genuinely different. Part 2 is what falls out of it once the reading is
held against this project's own constraint — ten thousand actors, ten
thousand POI and ten thousand chat bubbles in one place.

Nothing here is implemented. It is the baseline the next work gets measured
against.

The columns are the ones that document already separated — **store**,
**index**, **batch** — plus two it named without a column: **declutter**,
which turned out to be the same question as the index, and **residency**,
which is what the index becomes once there is a server.

---

# Part 1 — What seven systems already decided

## Store — Cesium 3D Tiles 1.1, implicit tiling

The published spec for streaming massive 3D geospatial content, and the
closest thing to a formal standard for what pixelet's terrain layer is.

**Implicit tiling.** A quadtree or octree is represented by _coordinates
alone_ — no per-node JSON, no tree of objects to walk. A tile is addressed
by its level and its x/y/z, which "allows for random access of tiles based
on their tile coordinates."

**Availability as three bitstreams.** Each subtree carries tile
availability, content availability, and child-subtree availability as
separate 1D bit arrays. "What exists here" is a bitmap, not a structure.

**Morton Z-order within a level.** The index for a tile is computed by
interleaving bits, so a parent or child index is one bit removed or added —
constant time. Consecutive indices are spatially near each other, which
gives both locality of reference and much better compression of the
availability bitstreams.

**What pixelet already matches.** The L0–L5 hierarchy is exactly implicit
tiling: `L3_IDX(i)` derives a page address from a coordinate with no lookup,
and a tile is fetched by that address. The choice was made independently and
the spec validates it.

**Where it differs.** Two things.

The index is packed but not Morton, so parent/child is arithmetic on
separate lanes rather than a bit shift, and consecutive page indices are not
guaranteed spatially adjacent. Whether that matters depends on whether
anything ever wants range-scans or run-length compression over page
availability — nothing does today.

And availability is implicit in a different sense: a page either is in
`tile_map` or it is not, which conflates "does not exist" with "not fetched
yet". `_ocean` is the one explicit "exists and is empty" marker. 3D Tiles
splits tile availability from content availability precisely because those
are different questions.

---

## Index — Teschner et al. 2003, optimized spatial hashing

The canonical paper behind "use a spatial hash for sparse moving 3D points".
Its value is that it measured the parameters rather than asserting them.

**Cell size ≈ the average extent of the thing being indexed.** Figures 5 and
6 sweep cell size against average primitive edge length and both bottom out
at a ratio of exactly 1.0, rising steeply below it and gradually above.
Larger cells put more primitives per bucket; smaller cells make one
primitive span many buckets.

**Cell size matters more than table size.** Stated outright: "the grid cell
size has a more significant impact on the performance than the hash table
size." The table-size curves plateau — 1200 vertices are fine past about
1000 buckets, 5898 vertices past about 2000 — while the cell-size curves
have a sharp minimum.

**Table size well above the primitive count, and prime.** They used 4999 for
1200 vertices, 9973 for 5898.

**Never clear the table.** Each simulation step gets a timestamp; a bucket
carries the timestamp of its last write, and a stale bucket is treated as
empty and overwritten rather than being cleared in a reset pass. This is
what makes a per-frame rebuild affordable.

**Linear, and independent of object count.** Two passes: insert every vertex
`O(n)`, then test every primitive `O(n·p·q)` where `p` is average vertices
per cell and `q` average cells per primitive. Choosing the cell size as
above makes both constants. "The performance is independent from the number
of objects. It only depends on the number of object primitives."

**The measured envelope**, for calibration: 20k tetrahedra and 6k vertices at
15 Hz on a Pentium 4 1.8 GHz in 2003; 1000 tetrahedra at 4.3 ms, 50,000 at
172.5 ms.

**What this means for pixelet.** The cell size should track the _entity's own
extent_, not a level of the existing hierarchy. An L2 chunk is 121 m and an
L1 cell is 1.89 m; a POI badge or an actor is neither, so quantising a
dynamic-point index against a terrain level would be choosing the cell size
for the wrong reason. The L4 ceiling from the density document bites here
too: above L4 the lattice has no Z, so a hash key derived from lattice
indices cannot be 3D above that — but a hash does not need the lattice at
all, which is the point.

### The 2024 update: same family, different construction

Zhyhallo, _Better Spatial Hashing with Linear Memory Usage and Parallelism_
(ICAART 2024), revisits the same algorithm two decades later and changes
what the build should look like.

**The bottleneck is RAM I/O, not CPU.** Stated outright: "we identified RAM
I/O operations rather than CPU power as the primary bottleneck", and
"increasing the number of CPU cores does not guarantee a decrease in
simulation time, even with the algorithm's lock-free design and utilization
of relaxed memory access." Memory layout outranks the algorithm.

**A table sized to the scene is unusable for an unbounded world.** The
classic construction allocates `T x T` for the whole scene, most of which is
empty when the scene is large and sparse. The fix is chunked allocation —
partition the space into uniform chunks and allocate only the chunks that
contain objects — after which "memory usage is no longer dependent on the
simulation space but rather linearly depends on the number of objects."

**The build is a counting sort, not a map of buckets.** Count per pivot,
prefix-sum the starts, then place each object into a flat data table with a
fetch-add. Two implementation notes come with it: the chunk size should be a
power of two so the chunk index is a shift rather than a division, and the
pivot tables should be padded and aligned to the cache line.

**At ten thousand objects the cost is flat as the world grows** — the
chunked curve is level across scene sizes where the scene-sized table climbs.
That is exactly the property an unbounded world needs.

---

## Declutter — Mapbox GL

The production answer to "which of the overlapping labels do you show", from
a renderer that has had to solve it at map scale for a decade.

**A uniform grid in viewport space, not world space.** `GridIndex` divides
the screen into cells — 30 px for a 600×600 viewport, so 400 cells — and
each placed geometry records which cells it covers. Queries are near
constant time; "doubling the dimensions of the index is not expected to have
a significant impact on the cost of a query."

**Two geometry kinds.** Rectangles for point labels, circles for labels that
follow a line. Padding is applied at runtime in screen coordinates rather
than baked into the precomputed shape.

**Greedy, in importance order.** For each label in priority order: compute
its screen geometry, query the grid, insert if the space is free, otherwise
mark it collided. There is no global optimisation — first fit wins.

**Cross-tile identity carries the fade state.** `CrossTileSymbolIndex` gives
the same real-world symbol appearing in tiles at different zooms one shared
`crossTileID`, and opacity is stored against that id rather than against any
individual symbol; at render time only the front-most symbol receives it.
This is what stops a label flickering as tiles load and unload beneath it.

**And the line that matters most here:** placement runs synchronously but is
explicitly _not_ required every frame — "a latency of a couple hundred
milliseconds in actually detecting a new collision is perceptually
acceptable."

**Against what pixelet does.** `frustum_budget.ts` is not a declutterer. It
is a distance cut: frustum-test every candidate, sort by squared distance,
take the first N. Nothing tests whether two badges overlap on screen, which
is the actual problem this solves. It also runs on every camera tick over
every loaded POI — 6621 in the measured session — where Mapbox's own
guidance is that this pass tolerates hundreds of milliseconds of staleness.

The cross-tile identity problem exists here too and is unsolved: POI records
are baked per L3 page, so the same place near a page boundary has no
identity independent of the page it arrived in.

---

## Cluster — Mapbox supercluster

The other half of the label problem, and the one that answers "what happens
at ten thousand" rather than "which of these two wins".

**Hierarchical greedy clustering.** Start from any point, take everything
within a radius, make that a cluster, repeat. Not sophisticated, and
deliberately so: it is fast enough to run over millions of points in a
browser, which the more careful algorithms are not.

**The hierarchy is built bottom-up and is therefore nearly free.** Cluster
at the most-zoomed-in level first, then cluster _those clusters_ — carrying
weighted centroids — into the level below, and continue down. The count
falls exponentially at each step, so building every level costs barely more
than building the first.

**Each level is a KD-tree** (KDBush), with `nodeSize` 64 leaves.

**The radius is in pixels, not world units.** Default 40, with `minPoints`
2 and `maxZoom` 16. Six million points is the demonstrated scale.

**And a cluster is a circle carrying a count.** That is the output shape,
not an embellishment on it.

**What this settles.** Clustering and decluttering are not alternatives. A
40 px cluster radius already guarantees the circles do not overlap, so for
_symbols_ the clustering is the decluttering. Labels are wider than 40 px,
which is why Mapbox still runs collision over the text afterwards. Both
passes, in that order.

---

## Batch — UE5 Mass Entity, and deck.gl

Two different scales of the same column.

### Mass: ten thousand agents, and what each tier costs

The tier table, with per-tier population caps as a first-class control:

| tier | distance | representation                       | rule                                   |
| :--- | :------- | :----------------------------------- | :------------------------------------- |
| 1    | 0–25 m   | skeletal mesh, full animation        | ~50x tier 3; **10–30 visible at once** |
| 2    | 25–60 m  | vertex-animation-texture static mesh | no AI                                  |
| 3    | 60–150 m | instanced static mesh, one draw call | no avoidance, overlap accepted         |
| 4    | 150 m+   | billboard impostor or culled         | ~0                                     |

Measured: 10,000 agents at **4.6 ms / 7.8 ms / 9.1 ms** on RTX 4080 / RTX
4060 / PS5. Animation instancing is **0.001–0.005 ms per agent** against
0.05–0.15 ms for skeletal. And the crossover worth remembering: below ~5000
agents the cost is dominated by rendering, above it by simulation.

The structural point is not the tiers, which are obvious, but that **each
tier carries its own maximum count**, enforced regardless of total
population. pixelet has one budget (`MAX_RENDERED`) and one representation.

### deck.gl: what makes an update cheap

Rules for keeping a large instanced layer from rebuilding itself:

- **Reference equality on the data array decides everything.** A shallow
  comparison between renders determines whether GPU buffers regenerate;
  handing back a new array with identical contents regenerates all of them.
- `updateTriggers` invalidates _one_ attribute rather than the whole set:
  "changing `data` will update all buffers, so if, for example, object
  positions have not changed, it will be a waste of time to recalculate
  them."
- **A constant prop uploads a constant.** `getFillColor: [255,0,0,128]`
  uploads four numbers; `getFillColor: d => [255,0,0,128]` allocates a typed
  array of `4 × data.length`.
- Avoid creating per-item objects in accessors at all.

Envelope: ~1M items at 60 fps on a 2015 MacBook Pro, 10–20 fps approaching
10M, browser limits between 10M and 100M.

**Against what pixelet does.** Decor already sits at the Mass tier-3
equivalent — instances baked into one merged buffer per chunk, no
per-instance scene objects — and that was arrived at independently. POI has
no tiering at all: every selected badge is one React component, one portal,
one sprite. And the deck.gl accessor rule names exactly what
`frustum_budget.ts` does wrong: it pushes one `{item, id, d2}` object per
in-frustum candidate and returns freshly `.map()`ed arrays, every tick.

---

## Residency — Photon Fusion, area of interest

What the index becomes when a server has to decide what to send.

Fusion keeps a **3D grid of interest cells**: default cell size 32, default
grid 1024³, default player radius 32 (capped at 300 in shared mode).
Interest is expressed as shapes — sphere, box, cone — which are then
_converted to cells_, and the documentation is explicit about the
consequence: "the actual area replicated to client is always bigger than
area defined by shapes." Objects outside a player's cells stop being
replicated at all.

The transferable parts are the shape, not the numbers: interest is declared
as a volume, resolved to cells, and deliberately over-approximated by up to
one cell rather than tested exactly. And it is the same grid as the client
index — which is the argument for choosing one and using it on both sides,
rather than the client inventing an index that the server later cannot
reuse.

This column is downstream of everything else in the density document's
order, and is listed here only so the client-side choice is made knowing
what will be asked of it later.

---

## The whole-system model — Figma

Named as the mental architecture model for this project, so it is worth
recording what it actually is rather than what it is assumed to be.

**A browser inside a browser.** Their own description: "we have our own DOM,
our own compositor, our own text layout engine." The C++ engine holds the
scene graph, the renderer, and the multiplayer client, compiled to
WebAssembly.

**A tile-based renderer on WebGL, everything on the GPU, fully
anti-aliased.** The reason given for not using canvas is precise and worth
keeping: "the 2D canvas API is an immediate mode API instead of a retained
mode API so all geometry has to be re-uploaded to the graphics card every
frame."

**Objects in pre-allocated typed arrays, to avoid GC pauses.** This is the
direct precedent for
[20260910-where-the-allocation-goes.md](./20260910-where-the-allocation-goes.md)'s
finding — a 4.9 s stall that was 4898 ms of CppGC pre-finalizers and atomic
sweep. Figma's stated reason for the layout is consistent 60 fps, and 32-bit
floats and bytes instead of JavaScript's 64-bit doubles is the same
decision seen from the memory side.

**Handles to out-of-heap data for large resources.** An `IndirectBuffer`
API holds images and geometry buffers outside the main heap, to avoid
fragmentation over a long session and to get around the 31-bit typed-array
size limit. Long-session fragmentation is a problem this project has not met
yet and will.

**Within a factor of 2x native, or better.**

**Multiplayer is deltas with last-writer-wins per property**, over a
per-document server process on a WebSocket. Not OT, and not a full CRDT —
"a custom system inspired by CRDTs but simpler", where each delta is "set
property P on object O to value V". That is the same shape
[20260908-command-channel-for-state-mutation.md](./20260908-command-channel-for-state-mutation.md)
arrives at from the other direction, and it is worth noting that the
industry reference point for this did _not_ need a CRDT to get there.

---

## Where pixelet actually differs

The stance in the density document was to follow the platforms and keep the
project's own invention for where it is genuinely unlike them. Four places
where it is:

**The world is unbounded and streamed; a Figma document is bounded and
loaded.** Every residency question — eviction, working set, what a jump
costs — exists here and not there. Figma never tears down three thousand
objects because the user scrolled.

**Terrain is derived, not authored.** A Figma node is user-created and can
never be regenerated; a pixelet chunk can be rebuilt from a tile in about
8 ms and a decor placement from a seed for free. That makes eviction a real
option here in a way it is not there — bounded by the regeneration cost,
which the ledger measures.

**The scene is 2.5D terrain plus a sparse object layer, not a general scene
graph.** Neither 3D Tiles' octree nor Mass's actor model is quite the shape;
terrain is a dense heightfield with channels and objects are a sparse tree,
and the existing `WorldLayer` contract already composes them into one volume.
That composition has no precedent among these six.

**Labels are anchored in 3D, not on a plane.** Mapbox's GridIndex is a
screen-space grid over a 2D map. Here a badge's anchor is a world point that
may be occluded by terrain, and the density document already records that
sprite occlusion is all-or-nothing against one anchor point — which is a
question Mapbox never has.

---

## What this changes about the open question

Nothing is decided by reading. What the reading does is narrow it:

- **Store**: the existing implicit addressing is right; the open choices are
  Morton ordering and splitting existence from availability, neither urgent.
- **Index**: a spatial hash with cell size set from the entity's extent, a
  timestamp instead of a clear, and a prime table above the entity count.
  The parameters are known and measured, not guessed.
- **Declutter**: pixelet does not have one. What it has is a distance cut.
  The precedent is a screen-space grid, greedy in priority order, with a
  stable cross-tile identity carrying the fade — and it does not run every
  frame.
- **Batch**: the missing piece is per-tier caps and more than one
  representation. Decor already demonstrates the merged-buffer tier.
- **Residency**: choose the client index knowing the server will want the
  same grid.
- **Everything**: the memory layout that avoids collection is the precedent
  that most directly addresses what was measured, and it is a policy about
  where objects live rather than which language they are written in.

---

# Part 2 — The shape this points at

Not a plan and not a schedule. What the reading, held against ten thousand
of each, leaves standing.

## The split that makes it tractable: volumes and points

Every mechanism in the engine today is volumetric — cell, voxel, run, box.
Terrain and placed objects fit it because they occupy space.

POI, chat bubbles and actors do not. They are **points with a screen
presence**: no volume, position that matters, and overlap as the governing
problem. That is why
[20260910-asset-identity-and-storage-model.md](./20260910-asset-identity-and-storage-model.md)
found billboards to be the one shape with no spatial structure of its own,
and why forcing them into the object octree kept feeling wrong. It was.

So the first move is a sibling to `WorldLayer`, for points rather than
voxels. None of the seven precedents puts both in one structure either.

## Store: partition by what does not move

3D Tiles' lesson is to address by coordinate and build no node objects. The
point equivalent is a flat array per page — which is what POI already is,
so nothing changes there.

Moving points are the different case. An actor crosses pages, so
partitioning it by page means re-bucketing on every step, and that is the
index's job rather than the store's.

> **The store partitions along the axis that does not move. The index
> partitions by where things are now.**

Static points (POI) keep their per-page arrays. Dynamic points (actors, chat
bubbles) get one structure-of-arrays with a stable id and no spatial
partition at all.

## Index: two of them, and merging them is the trap

This is the sharpest thing the reading produced. Mapbox has only a
screen-space grid because it has no depth. Photon has only a world grid
because it does not render. **pixelet needs both, and they answer different
questions.**

**A world-space hash** — "what is near this point in 3D", for residency,
interaction and eventual AOI. Teschner's rule sets the cell to the _query
radius_, since that is the effective extent of the thing being asked about.
Timestamped rather than cleared, prime table well above the entity count.

It must not quantise against the L hierarchy. An L2 chunk is 121 m and an L1
cell is 1.89 m; an actor is neither, so picking a level would be choosing
the cell size for the wrong reason. And above L4 the lattice has no Z at
all. A hash does not need the lattice, which is the point.

**A screen-space grid** — "what overlaps on screen", for placement. Mapbox's,
directly: uniform cells, greedy in priority order, and explicitly not run
every frame.

pixelet has neither today. `frustum_budget.ts` is a distance cut wearing the
name of the second one.

## Batch: the cap is the design

Mass's real lesson is not the tier ladder, which is obvious, but that **each
tier carries its own population cap** enforced independently of the total.
deck.gl and Figma say the same thing from the memory side.

```
fix a maximum per tier
allocate that buffer once, at startup
rewrite it in place every build
never reallocate, never dispose
```

Then the count of browser-owned objects is **constant rather than
proportional to what is on screen** — which is precisely the quantity
[20260910-where-the-allocation-goes.md](./20260910-where-the-allocation-goes.md)
measured at 4.9 s when it was allowed to vary.

## Ten thousand of each is three different problems

The instinct to collapse overlapping things into a circle with a count is
exactly supercluster's output, and it is right — for one of the three.

|                       | why                                                                           | strategy                                                                |
| :-------------------- | :---------------------------------------------------------------------------- | :---------------------------------------------------------------------- |
| **POI**, 10k          | individual identity matters less than presence; "47 here" is real information | **cluster**, circle plus count                                          |
| **chat bubbles**, 10k | the content _is_ the object; merging fifty into "50" destroys all of it       | **priority cut** — the nearest or most recent N, the rest simply absent |
| **actors**, 10k       | an individual whose position means something; two cannot become "2"           | **representation LOD**, never merged                                    |

Chat is the one worth stating explicitly, because it looks like the other
two and is not. A bubble is a message, not a symbol. MMO chat and floating
combat text both arrive at a cut rather than an aggregate, for this reason.

The three differ only in that first step. The index beneath them and the
batch above them are shared.

## Perspective breaks supercluster's assumption, and the fix already exists

supercluster's levels are **map zoom levels**, and zoom is uniform across a
viewport. A perspective camera is not: near and far are in the same frame,
so choosing one cluster level per frame is wrong.

This is already solved here. Chunks resolve their own `LodStep` from
distance — the 1/2/4/8/16/32/64 ladder `DECOR_DENSITY_BY_LOD_STEP` uses. So
a cluster hierarchy keyed on **page × LodStep** lets each page use the tier
it has already chosen. No new quantisation, and the tiers cannot drift out
of step with terrain and decor, because it is the same number.

Mapbox handles pitch the same way, and for the same reason: clustering is
per tile, and tiles at different distances are already at different zooms.

## The pipeline, then

```
1. cluster     10k POI  ->  the clusters visible at this page's tier
2. declutter   of those, which may carry a text label
3. batch       per-tier cap, preallocated buffer, rewritten in place
```

Actors skip step 1 and enter at step 2 with a representation tier. Chat
bubbles replace step 1 with a recency cut. Steps 2 and 3 are one
implementation.

## Verification, which the density document asked for first

This design is expressible as invariants rather than as claims, and the
ledger already counts all three:

```
live GPU buffers    constant
live textures       constant
new objects/frame   zero
```

`geometries` and `textures` are already in `__PXL_RENDER_STATS__`. A
regression here is caught by the instrument that measured the problem, not
by a test that would have to imagine it.

## What is genuinely undecided

**Where the clustering happens.** supercluster is a runtime library, but POI
is already baked per page, and baking the page × tier clusters would leave
the client reading rather than building a KD-tree — the same model terrain
tiles already use.

The seam has a known general answer and it does not fully cover this case.
Vector tiles clip features to a _buffer_ around each tile, and anything
inside the buffer is understood to carry over, with the duplicate resolved
by identity at render time. For points that works completely, and pixelet
already does it. For clusters it does not follow: a cluster computed from
page A's points plus a margin of B's is not necessarily the cluster B
computes over the same region, and a cluster has no natural stable id to
dedupe on. The cause is in the algorithm — hierarchical greedy starts "with
any point" and is therefore order-dependent, and two pages present their
inputs in different orders.

So baked clustering is possible under two conditions: the margin must be at
least the cluster radius at that tier, and the clustering must be made
order-independent — seeded from a deterministic key such as sorted
`gers_id`, so both pages produce the same clusters over the shared band and
a cluster id can be derived from its leaves. Runtime clustering needs
neither. The fork is still open; it now has a price on each side.

**Atlas repacking.** If badge labels are drawn into an atlas, a changing
visible set means repacking, which brings texture churn back. Mapbox does
not have this problem because its atlas is of _glyphs_, fixed, with labels
assembled from them. Following that removes repacking entirely, and looks
like the answer.

**Cross-tile identity is already solved, and better than the precedent.**
Checked: `poi_layer.tsx` already dedupes by `POI.id` across pages, and that
id is Overture's `gers_id`, stable globally. Mapbox cannot do this — vector
tile feature ids are not reliable across zooms, so `CrossTileSymbolIndex`
identifies a symbol by _(label text, anchor position quantised to a ~4 px
grid)_ and searches a KDBush within one grid unit, sorting matches to keep
the assignment deterministic. That machinery exists to reconstruct an
identity we are simply given.

**`rank` already exists and is deliberately unused — for a reason that
expires.** `POI` carries a bake-time importance rank, and `poi_layer.tsx`
declines to use it: "rank is 'how notable is this in general', not 'is this
visible right now'". That is correct for a distance cut and wrong for a
collision pass, where the order decides which of two conflicting labels
survives and the more notable one should. Mapbox's greedy placement runs in
exactly that order. The priority function for decluttering is already
computed and already shipped; it has had nothing to be a priority _for_.

**A 3D anchor's occlusion.** Mapbox's grid is a plane and never asks this.
[20260909-lod-look-and-seams.md](./20260909-lod-look-and-seams.md) already
records that a sprite cannot be partly occluded, so it is all-or-nothing
against one anchor point; a billboard free in Z reopens which point that
should be. None of the seven precedents answers it.

---

# Part 3 — The implementation shape

Where Part 2 says what the pieces are, this says what they look like as
code and in what order they arrive. Still not a schedule.

## Three structures

**`PointIndex` — a spatial hash built by counting sort.**

```ts
class PointIndex {
  private readonly cell_shift: u32; // power of two: shift, never divide
  private readonly counts: Uint32Array; // per bucket
  private readonly starts: Uint32Array; // prefix sums
  private readonly items: Uint32Array; // entity indices, flat
  private readonly stamps: Uint32Array; // per bucket generation
  private readonly touched: Uint32Array; // buckets written this generation
  private generation: u32;
}
```

Three passes, and no clearing pass:

```
count    hash the cell; if stamps[h] !== gen, reset counts[h] and record h
         in touched, then increment          (Teschner's timestamp)
prefix   accumulate starts over `touched` only, never the whole table
         (Zhyhallo's chunking, in miniature)
assign   items[starts[h] + offset[h]++] = i  (the fetch-add, single threaded)
```

Every array is preallocated at capacity. A rebuild allocates nothing — no
`Map`, no array literal, no per-entity object. That is the same rule Figma
states as typed arrays and deck.gl states as accessors, arriving from the
third direction.

**`ScreenGrid` — the declutter pass.** Mapbox's, directly: viewport divided
into cells, a fixed-capacity slot list per cell, greedy in `rank` order,
insert if the space is free and mark collided if not. Run on a camera-delta
threshold rather than per frame.

**Per-tier instance buffers.** One `Float32Array` per tier, sized at that
tier's cap at startup, rewritten in place with `addUpdateRange` and
`setDrawRange(0, n)`. Never grown, never disposed.

## Two indices, because lifetimes differ

```
static    POI             rebuilt when pages load or unload
dynamic   actors, bubbles rebuilt every frame
```

Same class, different cadence. Mixing them would rebuild ten thousand static
points every frame to keep a few hundred moving ones current.

## Order, and what verifies each step

**1. `PointIndex` alone.** No behaviour changes. Pure logic, so `deno test`
covers it with no browser — which is the half
[20260910-point-density-and-the-order-to-multiplayer.md](./20260910-point-density-and-the-order-to-multiplayer.md)
calls already tractable. The invariants are stateable:

```
every entity within the query radius is returned, none missed
nothing outside it is returned, including across cell boundaries
results are identical after a thousand rebuilds (does the stamp leak?)
a rebuild allocates nothing
```

**2. `ScreenGrid` and the declutter.** Replaces `frustum_budget.ts`'s
distance cut with the collision pass it was standing in for, and gives
`rank` the job it was computed for. The baseline already exists — B against
B1 in
[20260910-where-the-allocation-goes.md](./20260910-where-the-allocation-goes.md):

```
busy JS      2861 ms   -> ?    (1825 with POI off entirely)
MinorGC        x64     -> ?    (x26)
allocation  238.6 MB/s -> ?    (72.4 MB/s)
```

**3. Per-tier buffers.** This is where the collection cost goes. Verified by
`geometries` and `textures` in `__PXL_RENDER_STATS__` staying constant while
the scene changes, which the ledger already reports.

**4. Clustering.** Largest, reaches into the bake pipeline, and the fork
above is still open.

## The one decision step 1 must not make

What `cell_shift` should be. Teschner sets it from the extent of the indexed
thing; since the query is "what is within this radius", the extent here is
the _query radius_.

But the radius differs per consumer — actor interaction, badge selection and
eventual AOI are not the same distance. Either each consumer gets its own
index with its own cell size, or one index takes the most common radius and
the others sweep more or fewer cells than they need. Teschner's finding that
cell size matters more than table size argues for the first; the cost is one
rebuild per index.

That cannot be settled before the real query radii are counted. Step 1 takes
`cell_shift` as a constructor argument and defers it to step 2.

## Sources

- [3D Tiles Implicit Tiling specification](https://github.com/CesiumGS/3d-tiles/blob/main/specification/ImplicitTiling/README.adoc)
- Zhyhallo, _Better Spatial Hashing with Linear Memory Usage and Parallelism_,
  ICAART 2024 — [PDF](https://www.scitepress.org/Papers/2024/124157/124157.pdf)
- [mapbox-gl-js `cross_tile_symbol_index.ts`](https://github.com/mapbox/mapbox-gl-js/blob/main/src/symbol/cross_tile_symbol_index.ts)
- Teschner, Heidelberger, Müller, Pomeranets, Gross, _Optimized Spatial
  Hashing for Collision Detection of Deformable Objects_, VMV 2003 —
  [PDF](https://matthias-research.github.io/pages/publications/tetraederCollision.pdf)
- [Clustering millions of points on a map with Supercluster](https://blog.mapbox.com/clustering-millions-of-points-on-a-map-with-supercluster-272046ec5c97),
  [mapbox/supercluster README](https://github.com/mapbox/supercluster/blob/main/README.md)
- [Mapbox GL — Collision Detection](https://github.com/mapbox/mapbox-gl-native/wiki/Collision-Detection),
  [Optimize map label placement](https://docs.mapbox.com/help/dive-deeper/optimize-map-label-placement/),
  [Viewport collision detection (PR #5150)](https://github.com/mapbox/mapbox-gl-js/pull/5150)
- [Overview of Mass Gameplay in Unreal Engine](https://dev.epicgames.com/documentation/unreal-engine/overview-of-mass-gameplay-in-unreal-engine),
  [Building Crowds and Traffic in UE5: Mass AI and 10,000 NPCs at 60fps](https://www.strayspark.studio/blog/crowd-traffic-simulation-ue5-mass-ai)
- [deck.gl — Performance Optimization](https://deck.gl/docs/developer-guide/performance),
  [Attribute Management](https://deck.gl/docs/developer-guide/custom-layers/attribute-management)
- [Photon Fusion — Interest Management architecture](https://doc.photonengine.com/fusion/current/addons/interest-management/architecture),
  [Area of Interest](https://doc.photonengine.com/fusion/current/manual/advanced/interest-management)
- [Figma — Building a professional design tool on the web](https://www.figma.com/blog/building-a-professional-design-tool-on-the-web/),
  [Figma is powered by WebAssembly](https://www.figma.com/blog/webassembly-cut-figmas-load-time-by-3x/),
  [How Figma's multiplayer technology works](https://www.figma.com/blog/how-figmas-multiplayer-technology-works/)

## Status 2026-09-13

Steps 1–3 are built: `poi_index.ts` is the Teschner hash over lanes,
rebuilt a range per unit; the declutter is the greedy screen-grid
placement in `poi_layer.tsx`, in distance order with absorbed badges
becoming counts; the per-tier buffers are one instanced quad over one
slot atlas uploaded by rectangle
([20260912-linear-memory-what-is-left.md](./20260912-linear-memory-what-is-left.md),
[20260913 POI under the tick](../ARCHITECTURE.pixelet.md#billboards-and-poi)).
Step 4, baked clustering, was not taken: the runtime absorb-into-count is
enough at current densities, and the order-independence condition above
is the price still on record. The `cell_shift` question resolved itself
to one index with one radius, since the only consumer is the badge query.
