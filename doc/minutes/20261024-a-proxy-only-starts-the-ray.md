# A rasterized proxy may start a ray, and nothing else

**2026-10-24** — decided in the conversation ("혼합형으로 가도는 되는데"),
on one condition the user set: none of the machinery the chunk meshes needed
comes back. Not built. What is built first is the clean-up below, which
holds whichever way the start of a ray goes.

A ray costs what it does because every pixel walks the world's tree on its
own, though thousands of pixels looking at one face end on the same block. A
rasterizer shares that work in hardware: one box covers the pixels it covers
and each pays a depth test. So a rasterized proxy may tell each pixel where
its ray starts. It may not tell it what the ray meets. That stays the query's
answer, the one every reader asks
([a ray asks the world](./20261013-a-ray-asks-the-world-whether-a-node-is-empty.md)).

## What may not come back

At 10-12 the world was drawn from chunk meshes the CPU built. The meshes were
cheap to draw. What cost was everything that kept them: the run sets a chunk
was baked from, the frontier and the cover that chose which chunks to build,
the chunk table, the mesh slots, the build queue, and the invalidation that
told a mesh it was stale. Each was a state held beside the world, and each
needed its own rule for when it was right. None of them may return, under
any name.

## The conditions

1. **The proxy holds no fact of its own.** Its shape comes from what already
   bounds the world: the ground's per-square height bounds the eye's windows
   already hold (`bound_cs`, the levels of crates/spec/core/src/window.rs),
   and the objects' tree's nodes that hold something. Nothing is baked,
   meshed or cached for the proxy.
2. **It is made on the GPU each frame, from those bounds as they stand.**
   There is no CPU partition, no queue and no slot, so there is nothing to
   invalidate: a bound that changes draws differently on the next frame.
3. **It is conservative, and it only starts the ray.** A box over the
   highest ground in its square is never farther than the first block a ray
   through it meets, so a ray started at the box's front face misses
   nothing. The ray then walks as it does now, through the same query, and
   what it stops on is the pixel's.
4. **The camera grid picks the boxes.** At each distance the proxy draws the
   bound level whose cell spans a camera cell there, the rule a ray already
   asks by (`Tau::cell_at`), so the level of detail stays one rule.

## What it would replace

A ray's start is set today by three passes over a buffer sized by the screen
(crates/spec/core/src/beam.rs): beam rays at the camera grid's corners
(`beam_cs`), a start for each cell off them (`cells_cs`), and samples a quarter
cell apart that pixels take their block from (`sample_cs`). That buffer is
the one that met the 128 MiB per-binding floor on a 4K surface. A depth a
pixel from a rasterized proxy gives each pixel its own start directly, so
the proxy is a reason to remove those passes, not to add beside them. If it
cannot remove them, it is not taken.

## What is done first

Each of these holds whether or not the proxy is taken, because the ray stays
what decides the pixel. They are in, or being finished:

- a block's face bits asked of the query once a block, by the sample that
  stops on it (crates/spec/core/src/faces.rs);
- the camera grid computed once a frame and handed to the GPU in the eye's
  words (`beam::Grid`);
- the per-pixel miss list gone: a pixel that missed says so in its own ray
  word (`ray::MISS`);
- one depth rule for what is laid on a face, owned by the screen pass;
- the sun's map cast incrementally, only where it moved or the world changed.

Landing them first means a proxy, if built, is measured against a cast whose
start is the only thing it changes.

## What has to be measured

- What drawing the boxes costs: a square's boxes instanced over every window.
- What a ray still pays from a near start, since it descends from the root
  again at that depth.
- How loose the boxes are on steep ground, where a square's highest point
  stands well above most of it and the start gains little.
