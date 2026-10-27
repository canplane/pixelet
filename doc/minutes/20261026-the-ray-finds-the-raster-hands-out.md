# The ray finds, the raster hands out

**2026-10-26** — decided with the user. It records where the drawing has
settled, what its next changes are judged by, and what is still open. It is a
decision record, not a rule each change is held to.

> The drawing does not implement the world's meaning twice. Rays discover the
> world; the rasterizer hands what they found out to the screen. Where one stops
> and the other starts is set by cost, not by habit. Reusing an intermediate
> result is allowed; an independent consistency state kept for that reuse has to
> justify its cost and complexity. Performance comes first, and the drawing
> converges by removing computations and mechanisms it does not need.

## Decided: in the code since 21f72517b

- **The World Query owns what the world is.** Whether a ray stops on a block is
  the query's answer (`world::node`, the descent in rust/spec/core/src/cast.rs),
  one source the CPU and the GPU both compile. The raster orders what the query
  answered; it never decides what a block is.
- **The eye's drawing is ray → raster → ray.** Beams and samples find the blocks
  through the query, sharing what pixels share (pxl-shader beam.rs); each block
  a sample stopped at becomes a box (raster.rs `sample_box_cs`); the rasterizer keeps
  each pixel's nearest box its own ray enters (pxl-render raster.wgsl); the
  pixel takes that block and walks only where it holds none (cast.rs `cast_cs`,
  `miss_list_cs`).
- **The raster comes from the rays' results alone.** The frontier, the runs, the
  chunk meshes and their tables do not come back
  ([a proxy only starts the ray](./20261024-a-proxy-only-starts-the-ray.md)).
  The boxes are an intermediate result, made each frame.

## What the next changes are judged by

- **The structure is the optimization.** Rays discover at a coarse unit and
  share what they find; the raster projects what was found; only what is missed
  walks again. That division is how the drawing is made fast, not a tidy
  division of roles. A change is judged by the repeated computation it removes.
- **The structure alone has to be lighter than 2026-09-26** (e78eeec55, the
  chunk meshes), in the whole cost: the GPU, steadiness while moving, the CPU,
  memory, and how soon what is needed shows. Profiling is not how that level is
  reached; it is how the drawing goes well past it. A finished structure that is
  not lighter means the computation model is wrong: something still repeats, or
  the ray and the raster meet in the wrong place.
- **No mesher, no runs, no frontier**, under any name: no geometry kept per
  square, chunk or column across frames. The world has one representation, the
  query, and each frame's geometry is found by rays. Caching and reusing shared
  discovery is allowed and expected: a beam's descent, the observed pyramid's
  answers, across rays and across frames.
- **Each computation goes to what does it better.** "It is already a ray" or "it
  is already a raster" is no reason either way, and no paper's design is taken
  whole.
- **One role, one mechanism**
  ([one role, one mechanism](./20261025-one-role-one-mechanism.md)): a job moves
  to a mechanism that already holds its role, not to a second path beside the
  first.
- **The layers are the code's.** pxl-spec-core is "one source for the CPU and,
  through rust-gpu, WGSL", pxl-render's `gpu/` "decides nothing", and pxl-kernel
  decides. A rule only WGSL states, or a decision moved into the renderer, works
  against that.
- **Performance before exactness.** What is felt is the cost while moving. An
  approximation that looks right to the eye is good enough; its difference from
  the exact answer is not a reason to refuse it.

## Open

Nothing now. What was open here, settled the same day:

- **The sun's map took the eye's division** (e04a3a855): beams, samples a few
  texels apart, their blocks drawn from the sun and each texel taking the
  nearest it enters, walks only where none is entered. The policy that kept its
  clipmap between frames went with it, and a frame where nothing it reads moved
  casts nothing. Its walks read and write the eye's observed pyramid through
  the eye's line machinery (1c5455121).
- **The beams reuse the descent through the pyramid and their passes**
  (8d5d3980e, 2f6436d03): a node is asked of the world once a frame by the
  coarsest pass reaching it, and read after. A start below the root was built
  and not taken: the nodes holding the eye, asked once a frame and every beam
  ray resumed under them, measured no lighter on the fixed path (`beam` p95
  5.5 to 6.9 ms against 5.1, rough, one load each). Those nodes are the same
  few entries for every ray, read hot, and a ray to the sky climbs back through
  the nodes a walk from the root passes whole.
