# What a coarse LOD cell should look like, and which seams are real

**2026-09-09** — a design argument that ran through several reversals and
ended by restoring a model an earlier commit had replaced. Recorded because
the reasoning matters more than the diff: the same argument will come back
the first time someone looks at distant terrain and thinks it should be
smoother.

## The question

Merging a `lod_step`-cubed block of voxels into one box loses occupancy. If
seven of eight sub-cells are VOID and one is a red block, what gets drawn?

Alpha is the wrong answer — a voxel renderer's whole cost model assumes
opaque, unsorted geometry, and partial transparency would drag in ordering
problems for something that is not actually translucent. So the choices are
geometric or photometric: draw a smaller box, or draw a full box in a
different color, or refuse to merge the cell at all (the sparse-voxel-octree
answer, which costs geometry exactly where the merge was supposed to save
it).

## What was there, what replaced it, what came back

The original coarse tier shrank the box isotropically by the cube root of its
fill fraction, centered in its own cell. Over the day that was replaced twice
and then restored:

|          | model                                               | why it was tried                               | why it was dropped                            |
| -------- | --------------------------------------------------- | ---------------------------------------------- | --------------------------------------------- |
| original | `cbrt(fill)` shrink, centered                       | density is size; needs no alpha and no sort    | thought incompatible with a dither cross-fade |
| B        | full-size box, color dimmed by fill, cull below 25% | a dissolve needs both tiers in the same volume | dilates a lone block 8x or erases it          |
| B′       | as B, but Z kept at fine resolution                 | distant terrain read flat                      | made XY the privileged axis                   |
| C        | box fitted per axis to the voxels present           | fixes both, and is symmetric _within_ a cell   | still decimates XY only                       |
| final    | back to `cbrt(fill)`, centered                      | no axis is special                             | —                                             |

The measurable defect that killed B′ is worth keeping: its fill number mixed
resolutions, a coarse XY footprint over a fine Z height. At `lod_step` 8,
thirty-two voxels laid flat came out fill 0.5 and were drawn; the same
thirty-two standing up came out 0.06 and vanished. Orientation decided
whether geometry existed, not mass.

## Why the anisotropic versions were tempting, and why they lost

There is a real argument for XY-only decimation. Terrain is a surface, and a
surface's volumetric density in a coarse cell goes to zero as the cell grows
— area scales as L², volume as L³. So an isotropic occupancy rule _must_
erode surfaces at every coarsening. This is the same reason mipmapping an
occupancy grid thins geometry, and why voxel cone tracing stores directional
opacity rather than one scalar. Decimating XY while keeping height exact is a
heightfield mipmap: filter the coordinate, not the value.

That argument is correct about the mechanism and wrong about the decision.
It buys smoother distant ground by assuming the camera stays near-top-down
and the data stays surface-shaped — assumptions about today, not about
voxels. And it does not survive its own generalization: the pipeline's
`ColumnRunSet` is already XY-indexed with Z run-length encoded, so making the
LOD anisotropic too compounds an existing asymmetry rather than paying for
it once.

The final position: **no axis is a first-class citizen.** One cubic lattice,
one scalar occupancy over `lod_step³`, one cube root, centered. Mesh a chunk
on its side and you get the same chunk on its side. The cost is real and
accepted — flat ground shrinks too, and distant terrain reads as separated
cubes rather than a continuous sheet. That look is the price of a rule with
no exceptions, and it is cheaper than a rule whose exception is the camera
angle.

Two things were kept from the failed attempts, because they were independent
of the axis question:

- **Color is a surface average, not a volumetric one.** Only the topmost run
  of each sub-column contributes. A cell of grass over twenty of rock used to
  come out almost entirely rock, though grass is all you can see.
- **Full cells stay ordinary runs.** A cell at fill 1 shrinks by nothing, so
  it emits as a plain full-cell run and keeps greedy merging and face
  culling. Only shrunken cells take the inset path. The original emitted one
  box per bucket unconditionally and got neither.

The sub-cell encoding was made symmetric along the way: offset and size are
both fractions of the extent that axis would otherwise have had — one cell of
the current tier for X and Y, the run for Z. It used to be one isotropic size
with X/Y silently ignoring the tier's scale.

## The cross-fade, and why zooming out popped

Two ways to spend an LOD transition band. Per chunk: a stable hash picks one
tier for the whole chunk, so neighbors cross at different moments — cheap,
nothing drawn twice, and it reads as chunks flipping one at a time. Per
fragment: both tiers draw, punched with dither patterns — a real dissolve, at
the cost of drawing a blending chunk twice.

The per-fragment path is behind a debug toggle (**Settings → Debug → LOD
cross-fade (per pixel)**), default **off**. Edge shading is on by default at
strength 0.3.

Two bugs surfaced once the isotropic look came back.

**Complementary patterns assume equal volume.** A shrunken coarse box does
not fill what the fine tier fills, so exclusive patterns dissolved to
background rather than to the other tier. Fixed by overlapping the ramps
instead of splitting them: the coarser tier reaches full over the first half
of the band, the finer holds and then dissolves over the second. Mid-band
both draw. The only holes left are the coarse tier's own, which is the model's
look at that distance.

**The blend pair swapped roles at the band's midpoint.** Refining commits its
tier change at the band's inner edge, where the fade is over anyway;
coarsening commits at the threshold itself — the middle of the band. At that
point `guide.lod_step` crossed from the finer step to the coarser one, which
swapped which of the two the layout called primary. Same two meshes, opposite
labels, and an ordered comparison read it as a layout change: the node went
pending, abandoned the blend, showed one tier whole, and rebuilt a layout it
already had. Zooming in was smooth; zooming out snapped. Fixed by pinning the
finer tier as primary for the whole band.

Blending only happens at the three nearest thresholds (192/384/768) and
within ±5% of each. Wider bands put over half of all loaded chunks in some
band at once on a pulled-back view, which reads as uniform speckle — worse
than the pop it was hiding.

## TAA, and why not yet

The dither pattern is screen-space (interleaved gradient noise over
`gl_FragCoord`), which is what makes a cross-fade read as a fade rather than
as a texture stuck to the surface. The price is that the pattern is pinned to
the screen while geometry slides under it, so a moving camera makes the holes
crawl.

Temporal AA is the standard fix: jitter the projection by a sub-pixel offset
each frame, keep the previous frame, reproject it through the camera's motion,
and average. Successive frames then punch different pixels and 50% discarded
reads as 50% translucent. It also gives near-free supersampling as a side
effect.

Not being built now. It needs a history target, motion vectors, a reworked
post chain, and — the part that is actually most of the work — rejection
heuristics for pixels the reprojection gets wrong (disocclusion, fast
movement), or it ghosts. The renderer has SMAA, which is spatial and already
handles the aliasing TAA would incidentally fix. The only thing TAA would buy
here is making one debug toggle prettier. Revisit alongside WebGPU.

Judged in-session as not bad enough to act on.

## Seams: which joins are real

Edge shading draws a line where a cube edge is a silhouette. Getting that
right turned out to be three separate mistakes, all the same shape — a bit
that answered a _culling_ question being read as a _shading_ one.

1. **Chunk boundaries.** A face at a chunk edge is emitted without ever
   looking at the neighbor, so its face bit says "exists" while the world
   says "flat ground continues". Fixed earlier by separating `CREASE_*` bits
   from face bits.

2. **Placed user cubes.** An object is exempt from face culling — it must
   never drop a face and open a hole under or beside itself (a bug from
   2026-08-16). Shading read that exemption as "nothing is there", so a user
   cube set flush into flat ground drew a bevel down both sides of the join.
   Terrain against terrain was seamless; terrain against a user cube was not.

3. **Walls capped by a user cube.** The shader read `FACE_PZ` as "is this
   wall capped" — a proxy that only holds for terrain, since an object keeps
   its neighbor's face alive.

The general rule that came out of it: **culling gets to be conservative,
shading has to be accurate.** They now ask separately. `HIDES_SIDE` and the
vertical face tests still ignore objects, so nothing is dropped that an
object is standing in for; `ABUTS_SIDE`, `CAP_PZ`/`CAP_NZ`, and the diagonal
concave test count everything solid. An edge is a silhouette when nothing is
against it, whoever put that something there.

## Billboards are in the world, so the world can hide them

Same instinct, one tier up. Tier 3 billboards (chat bubbles, POI badges —
see [20260907-world-rendering-tiers.md](../20260907-world-rendering-tiers.md))
were `depthTest={false}` on the reasoning that the tier reads as UI rather
than as occludable geometry. But they sit at real world positions, so a POI
badge on the far side of a ridge stayed legible through the ridge, while tier
4's hover cursor — which has always been depth-tested — hid correctly.

`depthTest` was turned on, and turned back off the same day. The failure is
worth recording, because the reasoning that led to it is correct right up to
the point where it isn't.

**A `THREE.Sprite` is depth-tested as a point, not as a surface.** Its vertex
shader offsets the quad's corners in view-space XY and leaves view Z at the
sprite center's, so every fragment carries the anchor's depth. The depth
_test_ is still per fragment. So a badge is not compared against the world as
a thing occupying space — each of its pixels is asked "is the terrain behind
_you_ nearer than my anchor?", and different pixels get different answers.

What that looks like in practice is not a badge disappearing behind a hill.
It is a horizontal band of grass punched through the middle of the badge,
right through the text, wherever the terrain behind the upper half happens to
be nearer than the anchor. An arbitrary slice, changing as the camera turns.

`polygonOffset` was tried first — the coplanar bias `hover_cursor.tsx` uses —
on the theory that the badge was z-fighting the ground it sits 0.3 cells
above. It isn't fighting; the terrain crossing it is genuinely in front of
the anchor by a whole cell or more, and no constant bias should or could beat
that.

The real conclusion: **occluding a billboard has to be all-or-nothing.**
"Partially occluded" is not a coherent picture for a thing with one depth,
whatever the depth buffer says pixel by pixel. The question is binary and it
is about the anchor, so it has to be asked once, at the anchor.

The obvious way to ask it is a CPU raycast per badge, and that is the one
option ruled out. It is what the previous POI implementation did and what
this tier was rebuilt to stop doing — `MAX_RENDERED`'s comment in
`poi/poi_layer.tsx` records the DOM `<Html>` + occlusion-raycast version the
flat sprite replaced, cheaply enough to raise the cap from 20 badges to 80.
Worse, `ray_hit_world` goes through `with_raycast_composite`, whose cache is
four chunks sized for a single hover ray; every miss composites a whole
`ChunkVolume`, which is the same work the mesher does per chunk. Running two
to four of those a frame would undo the metric this whole document's other
half was spent improving.

**What was built instead: ask the depth buffer, once, at the anchor.**

`render/scene_depth.ts` is a `Pass` that declares `needsDepthTexture` and
copies the world's depth into its own half-resolution target after the scene
render. `billboard/anchor_occlusion.ts` patches the sprite material: the
vertex shader keeps the anchor's clip position from before the quad offset
(`modelViewMatrix[3]`, which is exactly the value three.js was already
using), and the fragment shader samples that copy at the anchor's screen
position and discards if the world is in front. Every fragment reads the same
texel, so the answer is all-or-nothing by construction — the thing depth
testing could not give.

Two properties worth stating because they are the design, not details:

- **The depth copy is one frame stale**, because a material inside the scene
  pass cannot read the texture that pass is writing. A frame of latency on
  "is this label visible" is not perceptible, and it is the entire price of
  not rendering the world twice.
- **It is not a by-product of an effect.** N8AO needs depth today and is
  expected to go once shading is baked into the textures, so the pass asks
  for depth on its own account and sits in the chain unconditionally. It is a
  renderer service; the next thing that needs to know whether a world point
  is visible reads the same texture.

`depthTest` itself stays off, now as a documented constraint rather than a
preference.

One thing from the failed attempt was kept, because it never depended on any
of this: a chat bubble draws over a POI badge (`BILLBOARD_ORDER.chat` 10001
vs `.poi` 10000). A bubble is something a person is saying right now and it
expires; a badge labels a fixed place and will still be there afterward.
Named at both call sites rather than defaulted, so the hierarchy is visible
where it is chosen.

## Status 2026-09-13

"TAA, and why not yet" became "tried and closed"
([20260911-taa-tried-and-closed.md](./20260911-taa-tried-and-closed.md));
the shimmer it was for is handled by MSAA, and by SMAA under DPR 2. The
rest — density as size, the stochastic handoff, the crease rules — is what
ARCHITECTURE.md now describes.
