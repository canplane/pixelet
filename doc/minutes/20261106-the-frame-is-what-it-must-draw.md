# The frame is what it must draw

**2026-11-06** — recorded with the user, in the session that runs the S23 batch
(eff52c1d). The measurements and the patches, as they happened, are in
[the journal](../gpu/experiment/20261103-the-frame-on-the-s23/README.md); the batch's
plan is [beside it](../gpu/experiment/20261103-the-frame-on-the-s23/README.md).

## What the user decided

**The target is the hot S23 at 30–40 Hz on the user's drive.** The drive is
`pxl.perf.drive` from script/compare-perf.py: Seoraksan, farthest zoom just over the
horizon, turning while the zoom sweeps. The run uses shadows on and `res fixed`.
Cool numbers count for nothing. No new scenario is invented while the user's exists.

**The work is an architecture audit, top down.** It runs in three levels, in this
order:
1. **L1, the system:** what the frame must produce, and whether each part is needed
   for it.
2. **L2, ownership and contracts.**
3. **L3, the code.**

What the pipeline doesn't need is removed whole, with its tests. Only what stays is
improved. In the user's words: a processor whose workload needs no floating point
carries no FPU, and the FPU's input/output tests are beside the point.

**Contracts are argued, not tested.** A change is judged on its contract argument:
who writes, who reads, in what order, and what equals what. A/B measurement does not decide
whether a part exists.

**The method:**
- Work deductively, from a cost model of the frame, not run-and-see.
- Change everything at once and bisect only if the batch fails. No per-patch GPU runs.
- Conventions (OS, industry) are applied without asking. xpute is fixed by the agents
  themselves.
- Only the layers' boundaries are fixed. Every other structure in the code is suspect.

**The lead's role:** hold the goal, decide, and challenge the premises of sub-work.
The lead does not forward messages and does not read code itself.

**Rays record, the raster draws.** The rays recognize the world to be drawn and
record it voxel by voxel, by the voxel's identifier. The raster draws, at once,
everything the rays collected. A pixel or a light texel that walks the world on its own
is a gap in the record, not a way of drawing.

**The rays' density stays.** Before, light that did not reach left horizontal
stripes of empty ground; the beams' and samples' density closed them, and it is not
to be lowered. The user's question is what happens when what is found is drawn
bigger, not when fewer rays are cast.

**Pixels only at the boundary.** The user likes axioms: inside the black box the
measure is the camera's geometry (angle, distance, the world's coordinates), and pixels,
DPR and CSS size live only where the canvas is met. As agreed in the discussion:
- The world's meaning does not depend on the output resolution: what is found, its
  zoom, when two found things are the same, and what is visible.
- Pixels are used only where the projection is sampled (the raster, screen shading)
  and as GPU execution units (workgroups, tiles), and there they decide nothing about
  the world.
- A unit is converted to another only at a named boundary: a pixel becomes a ray
  direction, and geometry becomes fragments.

This is the contract the discovery is to meet, not yet what the code does. Today the
miss path casts a ray for each pixel of the output, so what is found changes with the
render resolution. That dependence is to be removed or confined to the boundary.

**Discovery and drawing run apart.** The user's design: rays descend as far as each
frame lets them and overwrite what they found into a record of voxels kept by the
world's key; the raster draws the record every frame, whatever the rays have reached,
and waits for nothing. What has not been found yet is the next work for the rays and
blocks no frame. Fixed roles that pass finished work on a steady beat, and as little
mutual blocking as possible. Wavefront or systolic execution only raises the rays'
throughput later; it is not the design.

This names two contract changes:
- A frame's picture is no longer each ray's answer equal to the engine's. It converges
  to it while the view and the world stand, and lags while they move.
- The kept record is a state beside the world with its own invalidation. 10-24's second
  condition is reworded by this decision: the record is keyed by the world's nodes,
  forgotten by the node cache's own rule, and holds only what the World Query answered.

"Rays descend as far as each frame lets them" was replaced on 2026-11-07: every ray ends
in its frame, and what a frame defers is detail, never its answer
([a ray ends in its frame](./20261107-a-ray-ends-in-its-frame.md)).

**Stability by declaration, not by tests alone.** Agreed with the user after the
batch's whole-frame defects:
- Each pass declares every binding's role (args, list, state) and access (read, write,
  atomic), and the build checks the hazards over all passes: no args read indirect where
  they are bound writable, one writer a word, readers after writers.
- One storage place per fact.
- Schedulers and protocols are pure state machines whose reachable states are explored
  whole: frame kinds, owed and parked work, the record's claim, forget and eviction.
- A stage's interface (what it takes, gives and owns, and its invariants) is written
  beside its code once the stage stands. Changing it is a named contract change.
- Modules follow these roles, not file counts.

## What the renderer is (asked by the user)

The user asked whether cells aren't already drawn by raster: rays find the world, and
what they find is gathered and drawn by raster each frame. "Why per pixel? The voxels
are what must be drawn."

Today's eye is a hybrid. An outside reading the user pasted agrees, and the frame's
deduced model below holds the same:
1. **Beam rays** on the camera's angular grid (about 20 px a cell) walk the world and
   leave where they stopped.
2. **`cell_cs`** takes, for each angular cell, the earliest restart among its window
   of beam rays. It is a compute pass, not a raster.
3. **Sample rays**, about 5 px apart, resume from there and find a voxel node each.
4. **`sample_box_cs`** turns each found node into a box.
5. **`raster.wgsl`** draws the boxes with the hardware rasterizer. Each fragment
   checks that its pixel's ray really enters the box (`raster_enters`), and depth
   keeps the nearest. The result is a sample id per pixel in a visibility texture.
6. **`cast_cs`** reads that id and computes the pixel's entry and face, with no walk.
   The pixels no box answers are listed as misses.
7. **The miss passes** walk those pixels again from their angular cell's start.

So voxels found by rays are already drawn by raster. Two different things were both
called "cell", which confused the term:
- the world's cell (a voxel node);
- the camera grid's angular cell.

"Cell raster" in the lead's proposal meant something else. Instead of finding the
voxels by rays, every voxel node the zoom rule shows is listed, and all of them are
drawn by raster.

The light map works the same way at texel scale. Light beam rays, light samples and a
box raster are followed by a walk per texel that its sample doesn't answer.

## Where the frame's time goes (deduced, not measured)

A cost model of the drive frame was derived from the code on main, with nothing run.
It assumes a hot S23 at about 60% clock and a dawn sun about 1.7° up.

| | near end of the sweep | far end |
|---|---|---|
| frame | ~80 ms | ~21 ms |
| bound | 16–19 ms | ~14 ms |

Most of the near frame's excess is in two places:
- **Light walks per texel, ~30 ms.** At a low sun a light ray hugs the ground for
  200–265 u. Light samples miss tops foreshortened by sin e, so about 80% of slots walk
  again.
- **Eye misses, ~15 ms.** Samples are uniform in angle, but voxel nodes are uniform in
  log distance. Below about 9.5° of depression a node's top is shorter on screen than
  the sample spacing, so about 0.26 M pixels miss and walk about 74 nodes each.

The rest:

| part | ms |
|---|---|
| light_receivers (2.1 M threads to mark a few thousand tiles) | ~3 |
| px written and read back | ~2 |
| the shadow map laid whole | ~1.5 |
| face asked again every frame | ~1–1.5 |
| frag_depth disabling early-Z | ~1–2 |

The user's question names the problem. The frame draws voxels, yet most of its time
goes to rays walking again per pixel and per texel where the found voxels did not
answer.

## Done on 11-06

Removed whole, since no product pixel depended on them:
- the ray-steps view;
- the camera gizmo.

Each walk had carried a visits count for the ray-steps view.

Dropped as patches on a structure being replaced:
- the shadow depth slide;
- the map gate;
- the eye's list budget;
- px folded into the world pass;
- resolve and finish fused (a second path that only `fixed` res would take).

Landed:
- light budget;
- atomics only where read back;
- needless work;
- decor gate;
- census-a: far rays asked by their footprint, asks from the parent, the ways' fill
  share as the engine's;
- census-b: one list append, eye and light alike;
- the submit rule: no indirect read in the command buffer that wrote its args, every
  GPU count clamped to its room, the page's frames in flight bounded as native's;
- native wakes through the program's strobe.

In progress:
- **Receiver tiles derived per tile from the eye's samples** (pixelet-59). Every light
  pass and the map's lay go indirect off their count, so with no receiver the light
  path runs nothing.
- **A defect found in census-a's footprint zoom.** The slant takes each ray's own
  direction, so rays through one face can ask different zooms. It is being made a
  function of the node and the eye alone.

## Open, the user's

**Draw the voxels, not walk per pixel.** There are three readings:
- **The light map:** list each claimed tile's voxel nodes at its band and draw them by
  raster. The answer is exact, since a tile has one zoom. The cost is a list of
  0.25–1 M nodes at a low sun, up to about 64 MB.
- **The eye, listed whole:** cost follows the voxels drawn, hidden ones too, not the
  screen. This changes 10-31's "cost follows the screen"
  ([why rays](./20261031-why-rays-and-what-makes-them-fast.md)). It needs a "solid"
  answer for ground under the surface, or the list grows to about 1.7 M. Eye and light
  together, about 1.2 M, overrun the node cache.
- **The eye, kept as rays:** the sample lattice is made denser in rows by their
  depression, so the found voxels answer the grazing rows and the misses go.

The lead recommended the light by raster and the eye kept as rays with that lattice.
The user has not decided.

The full contract and its sizes are in [cell raster](./20261106-cell-raster.md). The
model's sun angle (1.7°) and the contract's (10–15°) disagree, and the drive's actual
time of day settles it.
