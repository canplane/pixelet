# The GPU casts a ray a pixel

**2026-10-14** — decided in the conversation.

**Status (2026-10-24): holds, and is no longer an experiment.** A ray a
pixel of the render resolution, its start its cell's beam
([a cell casts a beam](./20261022-a-cell-casts-a-beam.md)); a pixel takes
the nearest block the samples about it stopped at that its ray enters, and
walks itself only where none is (pxl-shader `cast_cs`). The render
resolution is the display's times a scale the load sets by default: one of
`SCALE_STEPS` steps between whole and `SCALE_MIN`, each a like share of the
last; where the GPU's time is read, a window over its budget by more than
`SCALE_BAND` steps down at once, and a step up waits until the step above
fits for `SCALE_UP_WINDOWS` windows in a row (pxl-kernel
domain/view/frame/resolution.rs, domain/view/param.rs); the frame is upscaled within
itself. The `rays` setting below is gone, and the overwrites
run on the GPU (One kind, one side).

What is drawn is what the eye's rays reach
([the ground is a grid](./20261012-the-ground-is-a-grid-then-overwrites.md)),
and a ray reaches it by asking the world whether a node is empty
([a ray asks the world](./20261013-a-ray-asks-the-world-whether-a-node-is-empty.md)).
This decides where that ray runs: on the GPU, one a pixel, from one source
that the CPU compiles too. It is an experiment pushed on the web first. If it
does not pay, it is retreated from.

## Why a ray a pixel

One ray a camera-grid cell, turned back into world boxes, drew the ground in
stripes: rows of rays with sky between them. The screen-cells experiment
(`480e6b8f9`) proved the cause. Painting each ray's result over its own cell
of the screen closed the gaps. A ray a pixel on the GPU draws the ground
continuous, with nothing turned back into geometry. The same rule of detail
still picks the zoom a ray asks at a distance (`Tau::cell_at`). Only the rays
per screen change.

A Retina resolution is not required. Half the screen's pixels is a
legitimate target.

## The structure

**One source, two compilers.** `pxl-spec-core` (`rust/spec/core`,
`#![no_std]`, `gpu-portable` in its header) holds what the CPU and the GPU
both compute:

- the zooms and the lattices;
- a node's seed and the caves;
- the block catalog;
- the ground's one answer for a node (`ground::node`, `34baff12e`);
- the service area;
- the ray's descent (`cast::first`, generic over `Query`).

pxl-spec re-exports it. rust-gpu compiles the same crate to WGSL
(`pxl-tool bin/shader_wgsl.rs`, in a Linux container), and the generated
`rust/client/render/src/frame/shader/cast.gen.wgsl` is checked in. `--check`
says whether it is current, so no everyday build needs rust-gpu.

**What the source cannot hold.** WGSL has no u64, i64, f64, u8 or i16, no
infinity and no recursion, and rust-gpu lowers no enum with data. The source
is written to that:

- hashes are u32 over i32 cells (xpute-kit `math::rng` `derive_seed_u32`,
  `5e4115cd2`);
- zooms are u32;
- the descent keeps a fixed stack, one `Query::At` a zoom.

f32 on a GPU is not strictly IEEE, so a stop at an edge may differ in the
last bit between the two sides. No tolerance is added for it.

**The boundary.** The GPU reads the world only from what the kernel uploads,
and only data and call records cross:

- compute pipelines, passes and dispatches;
- one upload op that names its bytes by address and length in linear memory
  (`a42022dfd`).

The bytes come from their owner's range while it lives until the page replays
the turn, else from a copy into the stream. The ground goes up as a window a
zoom, origin and layer a tile, in `earth_window`'s shape. A tile not held is
answered by the coarser zoom.

_Superseded below: the separation (pxl-spec-core ways.rs, `ways*.gen.wgsl`),
what is placed (pxl-spec-core overwrite.rs) and decor (`decor*.gen.wgsl`)
now run on the GPU, and the CPU's cast draws nothing._

**One kind, one side.** The GPU's query answers the ground and the caves.
Separation, what is placed and decor are the CPU's alone for now. That is
this experiment's stated gap. The `rays` setting chooses the CPU's cast
(the default) or the GPU's at grid, half or full resolution.

**A known break on the device path.** Naga's Metal backend breaks a
`loop { … continuing { break if … } }` an iteration early
(gfx-rs/wgpu#4558, fix PR #10459 open). The web goes through the browser's
own compiler and is not affected. `node_seed` is unrolled for it (`[naga]`).

## Where it stands, measured

The setup:

- M1 Max, Chrome;
- the eye pinned by `?eye=lon,lat,dist,polar,azimuth` (`2ff4da2a9`);
- fps is rAF over 5 s;
- full is 2056 × 1113, 2,288,328 rays.

Parity at `34baff12e`: 0 differing rays of 2,288,328 (GPU full), 0 of
572,596 (GPU half) and 0 of 9,906 (CPU lane).

|      | fps            |
| ---- | -------------- |
| full | 2.5 → 3.5 → ~4 |
| half | ~13.5          |

Steps, the nodes a ray asks about: median 48, p99 about 134.

Where full's frame goes, each part added to the one before:

|                                         | ms   |
| --------------------------------------- | ---- |
| the descent alone (V1)                  | 14   |
| the node queries                        | ~89  |
| the ground's heights, bicubic per query | ~148 |
| the wall's neighbour reads              | ~18  |
| past the area                           | ~6   |

The walk is cheap. Its steps are the order of the references' below. The
cost is in what one step does. A node query reads a tile's raw samples,
4 × 4 of them, and runs the bicubic again: for every pixel, at every step.

## The references

Read and checked, numbers as published:

- **Dick, Krüger, Westermann, _GPU Ray-Casting for Scalable Terrain
  Rendering_, Eurographics 2009.** This is our picture:
  - 512 × 512 tiles;
  - a 16-bit height texture a tile, built on the GPU once the tile becomes
    visible, with a maximum mipmap over it (+1/3 memory);
  - a stackless walk that steps a level up or down;
  - heights piecewise constant. Below a pixel of screen error this loses
    nothing, so no intersection with an interpolated surface is computed.

  Results: about 30 fps at 1920 × 1080 on a GTX 280 (about 62 M rays/s, by
  that product).
  Steps mostly under 50. The maximum mipmap gave about 5× over a flat walk.

  Their counterexample: on coarser data (Utah, 5 m) the rasterizer was
  faster. A ray's cost follows the screen, not the data, and wins where the
  data is dense per pixel.

- **Laine and Karras, _Efficient Sparse Voxel Octrees_, NVIDIA
  NVR-2010-001.**
  - 28 to 151 M rays/s at 1024 × 768 on GT200, about 560 to 3,900
    instructions and 8 to 53 iterations a ray.
  - "50 million primary rays per second … enough for first-hit rendering in
    1080p resolution with 25 frames per second."
  - Beam optimization starts each ray from a coarse conservative distance
    image. It gave 2 to 25%, its coarse pass 4 to 18% of the frame.
- **Tevs, Ihrke, Seidel, _Maximum Mipmaps_, I3D 2008.** This is the
  hierarchy Dick builds on. Only the abstract was read.

We cast about 9 M rays/s on a GPU with roughly ten times a GTX 280's
arithmetic and three times its bandwidth. The walk matches theirs, and the
difference is the step's work.

## What follows, in order, each measured alone

_History: the order as set on 2026-10-14. Steps 1 to 3 are now a square's
fill on the GPU as it lands (pxl-spec-core fill.rs: `heights_cs`,
`bound_cs`, the area laid on a cell), 6 is the beam, and 7 the overwrites
above; what the code holds is in ARCHITECTURE.md (Windows and squares;
Drawing)._

The rule for every step: 0 differing rays, then fps at full and half. A step
that does not pay is reverted.

1. **The ground's cell grid.** When a tile becomes held, a compute pass runs
   the one `ground::read` over its 512 × 512 cells once and writes the
   result. A node query then reads one cell. This is not a cache: 20261012
   names the ground "a tile's 512 × 512 cells, each a block shifted up to
   its cell's height", so the grid is the ground's representation and the
   samples are its source. `read` stays the one producer of a cell's
   height. Ordered 2026-10-14: pixelet-01 the portable and shader half,
   pixelet-fb the residency and binding.
2. **The bound from the grid.** A maximum and minimum pyramid over a tile's
   cells, from the grid's own reads, takes the place of what `relief`
   derives.
3. **The wall in the grid.** The area's edge laid on a cell is a fact of
   the tile. It is read once, not as nine neighbours a query.
4. **The step's size.** The generated WGSL is 465 KB. A cave child builds
   all eight of its siblings to take one (`array<Cave,8>`, 22 times).
5. **Textures or storage buffers** for the grid, by the 2D cache.
6. **The beam**, last. The reference measures it at a fraction.
7. **Then the overwrites.** Separation, what is placed and decor go to the
   GPU's query, and paint parity follows. Only then does the CPU's drawing
   cast go, with the `rays` setting.
