# Coarse tiers: what the field does, and why the block was the wrong cut

Written after a day spent building a "block" — a unit of 4×4 chunks scanned
and baked as one — and finding on measurement that it reduced draw calls and
nothing else. This records what three established systems do for the same
problem, what they have in common, and where the block departed from it. It
is the check that should have come before the code.

## The question

Zoomed out, the renderer shows ~1,100 chunks, most at steps 8 and 16. Each
one emits a handful of boxes and costs a full 3 ms scan, a node, a draw call
and a pool block — the same as a chunk at the finest step that emits
thousands of faces. The zoom will grow: a district today, a city or a
province later. What is the general rule for the coarse end?

## What three systems do

### CDLOD (Strugar 2010) — heightmap terrain, the closest match

Structure: a quadtree over the heightmap of constant depth; a node stores the
min and max height of the area it covers. Every level's node covers four
times the area of the level below and is drawn with **the same grid mesh**,
so each successive level has four times fewer nodes and the on-screen
triangle density stays roughly even. LOD ranges double per level; a node is
selected when it falls in its level's range and none of its children are in
theirs, and a morph over the last 15–30% of each range blends a node into its
parent's resolution before the swap.

The part that matters here, from the paper: "The actual rendering is
performed by rendering areas covered by selected nodes using only one unique
grid mesh, reading the heightmap in the vertex shader, and displacing the
mesh vertices accordingly." A coarse node **samples the source heightmap at
a coarser stride**. It is not built from a finer mesh. Nothing is folded.

### 3D Tiles (OGC, Cesium) — the streaming standard

Structure: a spatial hierarchy (quadtree, octree, k-d) where every tile has
its own content at its own detail. Selection is by geometric error projected
to screen-space error against a pixel threshold, not by zoom level. Tiles at
every level are **precomputed offline** by conversion tools; the client never
derives a parent from its children at runtime. Refinement is either
replacement (the parent's content is swapped for the children's) or additive
(children add to the parent).

### Slippy-map tile pyramids — the web map standard for twenty years

Every zoom level is a full, independent set of same-sized tiles; level z+1
has four times the tiles of z. Each level is **resampled from the source**
and cut into tiles. A zoomed-out view reads coarse tiles; it never assembles
them from fine ones. This is how every web map shows a district, a city and
a country with the same code path.

## What they have in common

1. **Coarse levels come from the source at a coarser sampling, not from
   folding the fine level.** All three. It is the whole reason the coarse
   end is cheap: a level-4 node costs the same as a level-0 node, because
   both are one grid read from the source at their own stride.
2. **The unit grows with the level, and the cost per unit is flat.** Four
   times the area per level, the same mesh, the same work.
3. **One selection rule across all levels, distance- or error-based**, with
   a blend at the boundary. Every system selects nodes top-down from the
   coarsest level and only descends where the range demands it.
4. **The finest level is the only one that has to be exact.** Coarse levels
   are approximations by design; their job is a stable silhouette and the
   right color at the right size on screen.

## Where this codebase stands against that

The terrain page is already the source these systems read from: a page is
`{ ground, ele: Int16Array }` — one elevation and one ground type per cell,
a heightmap with a material map. The finest tier does the right thing with
it: the terrain layer fills one column per cell into a 64³ volume, the scan
reads the columns back as runs, and the mesher bakes faces with neighbor
culling and greedy merging. That is a voxel pipeline, and at step 1 it has to
be one — placed cubes, sub-cell objects and carved terrain live in the same
volume and the mesh must be exact.

The coarse tiers do the same thing and then fold it. Step 8 fills 4,096
columns from the heightmap, scans them all into runs, and folds every 64 of
them into one box. The heightmap is inflated into a volume and reduced back
to a heightmap sample, at 3 ms per chunk, to produce what a direct read of
`ele` at stride 8 would have produced in microseconds. The coarse end pays
the fine end's whole cost to throw most of it away.

**The block did not change that.** A 4×4 block scans 16 chunks' worth of
columns into one run set and folds them once. Same 65,536 column reads, same
3 ms per chunk of them. What it changed was the number of meshes and draw
calls — 986 to ~100 — and draw calls were not the bottleneck (`gpu p95`
~10 ms before and after). It added a second kind of node with shared,
member-driven work on top of a scheduler whose every rule assumed one chunk
is one unit of work, and three slot deadlocks and a broken draw order
followed from exactly that assumption breaking. The measurement after all
of it: scans/s and commit-per-bake unchanged, total scans unchanged, pool
higher (members and blocks both holding run sets), drain time longer.

So the diagnosis in
[20260912-coarse-tiers-need-a-larger-unit.md](./20260912-coarse-tiers-need-a-larger-unit.md)
was right and the prescription was wrong. The cost is not that the unit is
small. The cost is that the coarse tiers are built by the fine pipeline.

## The rule, then

Coarse tiers should be built the way every system above builds them: **from
the page's heightmap directly, at the tier's own stride**, without a volume,
a scan, or a fold. At step 8, a unit is a 64×64 grid of 8-cell samples read
straight out of `ele` and `ground` — `ele[cy*8*W + cx*8]` with the four
ground layers' majority or the same nearest sample — baked as boxes with the
same coarse mesher that exists today. The cost is a few thousand array reads
per unit: three orders of magnitude under the scan.

What this preserves:

- The finest tiers, untouched. Step 1, 2 and 4 stay on the voxel pipeline,
  which is where exactness is needed and where placed objects live. Whether
  step 4 moves to the direct path is a measurement, not a principle.
- The scheduler, untouched. A chunk stays the unit of work at every tier;
  a coarse chunk is simply a cheap chunk. Every rule from
  [20260912-what-to-render-when.md](./20260912-what-to-render-when.md)
  — priority classes, aging, slot holders, request validity, release order,
  memory budget — keeps its premise.
- The mesher's coarse path, reused. It already takes a `res × res` grid of
  cells; the only change is that the cells come from the heightmap rather
  than from folded runs.
- Placed objects at coarse tiers: the object layer's `paint` still runs per
  chunk into the direct grid at the tier's stride, so a placed cube that
  covers a coarse cell still shows. What is lost at coarse tiers is
  sub-cell detail, which is lost there today too.

What it gives up: at coarse tiers, a cell's color is a nearest or majority
sample rather than the exact four-material average the fold produces. The
field accepts this everywhere — coarse levels are approximations by design —
and at the distances where step 8 applies a cell is a few pixels wide.

And the general case the question asked about: a city or a province is the
same rule at a larger stride. A step-64 unit reads one sample per chunk. The
units per level fall by four each level, the cost per unit is flat, and the
range grows without the work growing with it. That is what the tile pyramid
has always been.

## Done, 2026-09-12

The direct path is in. `WorldLayer.runs` lets a layer write its content at a
key as column runs straight into the scan's typed scratch, and the terrain
layer does so from the page heightmap: one run per column, floor to
elevation, in the column's material. A chunk takes it when every wanted tier
is step 8 or coarser and every layer either offers runs or is empty there;
the object layer answers that with one map lookup, so a column holding a
placed cube keeps the scan and its exactness. The mesher, the bake, the
cache and every scheduler rule are untouched — a coarse chunk is the same
unit of work as before, only cheap.

Pinned by test: the same heightmap bakes to the identical coarse mesh
through both paths, and the direct path costs 0.15 ms per chunk against
1.63 ms for the scan — 10.6×, the floor the test enforces. The first draft
built run objects and measured 1.4×; the ratio comes from writing typed
scratch, not from skipping the volume alone.

This closes the rendering work. What remains on the coarse end — draw
calls, a wider unit — is not a bottleneck today and is not to be taken up
without a measurement that says it is.

## What to do with the block

Revert the node wiring (step 3). Keep the span-parametric scan and the block
module: they change no behavior, the tests pin them, and a wider unit may
still be wanted later for draw calls once the scan cost is gone — at which
point it would be built on the direct path, not on the fold, and would not
share work across scheduler entries. Keep the boot-time bake-slot fix found
along the way; it is a regression from the request-validity rule and has
nothing to do with the block.

Sources:

- [CDLOD paper (Strugar)](https://aggrobird.com/files/cdlod_latest.pdf)
- [Introducing 3D Tiles (Cesium)](https://cesium.com/blog/2015/08/10/introducing-3d-tiles/)
- [3D Tiles Q&A (CesiumGS)](https://github.com/CesiumGS/3d-tiles/blob/main/Q-and-A.md)
- [Slippy map (OpenStreetMap wiki)](https://wiki.openstreetmap.org/wiki/Slippy_map)
- [Distant Horizons (GitLab)](https://gitlab.com/distant-horizons-team/distant-horizons)
