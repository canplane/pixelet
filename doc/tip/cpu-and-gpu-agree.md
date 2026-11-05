# When the CPU and a GPU must give one answer

**2026-11-05.** Why the same formula, in the same source, gives the CPU and a
GPU different answers, when that matters, and how a rule is written so it does
not. Met while holding the hand WGSL to pxl-engine word for word
([the journal](../experiment/20261104-taking-rust-gpu-out/journal.md),
[the minutes](../minutes/20261105-the-shaders-by-hand-held-to-the-engine.md)).
It is a general problem, known from games run in lockstep and from GPU
computing; the references are at the end.

## What goes wrong

The world is discrete: a cell is filled or not, a road's top is the third cell
or the fourth. Each such decision is made by comparing or rounding a value
computed in floats. The CPU and a GPU compute that value with different last
bits:

- **A product fused into a sum** (FMA) rounds once where the CPU rounds twice.
  WGSL cannot forbid it.
- **A sum reordered or simplified.** Metal's default fast math does it; the
  compiler is the browser's (Tint, WebKit) or the driver's, never the
  program's, so no flag turns it off. ALICE at CERN matches GPU and CPU bit for
  bit only by turning FMA and fast math off in its own compiler, which a page
  cannot do.
- **Division, square root, transcendentals** are correctly rounded on the CPU
  and bounded only on a GPU (division 2.5 ulps in WGSL; 13 measured on Metal).
- Even the CPUs differ: baseline x86-64 has no FMA, aarch64 does.

Two things follow, and the second is the deeper: the CPU and a GPU differ on
one formula; and even one side alone cannot promise the same bits from one
source, since what decides them is the instructions compiled (another compiler,
version, target or optimization level; another browser or driver on a GPU).
The rounding rule is the same on both sides (half to even). What differs is
the value rounded. A 1-ulp difference changes nothing for 3.2, but turns an
exact 3.5 into 3.4999998, and the cell becomes 3 instead of 4. **Only a value
within an ulp of a decision's boundary flips**, so the mismatch is rare and
silent: nothing shows it unless the two are compared.

Where it showed here, once a test compared them:

- the camera grid's zoom: rust-gpu had compiled the Rust's division into a
  product by a reciprocal, so the two had parted all along;
- the light's band: a square root and a fused sum;
- the ground's top (`ground::bicubic`): 214 tops within 4 ulps of a half, 13
  rounded the other way;
- the ways' road tops: 96 columns, where quantized inputs (whole tops, eighths,
  half-cell centres) and linear interpolation land exactly on a half.

The ground's top and the ways' crossing are exact since, in integers
(pxl-engine ground.rs `height`, way.rs `on_way`, `crossing`, `pier_covers`, each
with its bits counted), and their tests compare word for word.

## When it matters

Only when **two places decide the same thing on their own and must agree**.
On one machine the CPU does not decide the ground or the ways again: the
kernel reads back the cells the GPU filled (pxl-kernel
domain/layer/terrain/clipmap.rs `CpuClipmap`) and keeps the GPU's ways' tree
as the GPU laid it (mirror.rs), and decides where a walker stands over them
(domain/query/support.rs). The ground and the ways are exact because two
machines must agree: each machine's GPU fills the same brick, and must fill it
alike; and geo-bake, on the server, computes the same rules. Where two
machines part, a block stands in one place on one and in another on the other.
pxl-engine holds each rule, and a reference test holds the GPU to it word for
word.

The line runs between the world's state and its drawing:

- **The world's state must agree, exactly.** What fills a cell, the ground's
  and the roads' tops, what a walker stands on or collides with, what is
  placed and baked: every GPU and the server decide it in Q3, in Morton cells
  and zooms, in integers.
- **The drawing need not agree with the CPU.** A ray parted by an ulp colours
  one pixel on a cell's edge from the cell beside: nothing sees it and nothing
  depends on it. The ray's walk, the zoom bands, the light's bands, the
  projection, depth and light stay in floats, where the GPU is fastest. A value
  only the GPU uses (the eye's ray, `spacing_width`) needs only to agree with
  itself.
- **The GPU must agree with itself**: passes leave no cracks between them; the
  same bits frame after frame, so a still view does not flicker; and what a
  cache keeps is the same as what would be worked out again. One function
  does not by itself make it agree across pipelines: each pipeline compiles
  the function with its own fusion and order, so two pipelines may decide one
  value apart at an edge. A value several pipelines read and decide at an edge
  is therefore exact: the light ray's start is in integers (light_aim.wgsl
  `texel_start`, read by light_beam, light, light_miss, light_miss_hit,
  light_sample_box and sample.light); so is the light's band, which seven
  passes read: its gap is whole texels summed in integers, times one word the
  host rounds (`light::band_texels2`, `W_BAND_SCALE`; tests/light.rs
  `every_pass_bands_a_tile_alike`).
- **What an edit lands on is decided once and shared as cells.** A click or
  an edit resolves to integer cells (Morton, zoom) on the machine where it is
  made, and those cells, not the pointer, go to the others; no machine casts
  another's click again. Where machines must pick alike from the same input,
  they pick by an exact rule from integer inputs, never by their GPUs' rays.
- A reference test holds a drawing pass to pxl-engine to prove the
  translation, not because the product needs the CPU's bits; it names the
  GPU's rounding it allows and counts the cases.

Decor's footing was checked and is exact on both sides: the GPU's is in
integers, and the CPU's walk goes straight down a column's middle, where every
plane is a whole number of Q3 steps under 2^24 (pxl-engine support.rs
`column_surface_is_exact_to_the_worlds_edges`).

## How a rule is written

- **A rule of the world's state is exact.** Every intermediate value is
  represented exactly, so no operation rounds and neither fusion nor order
  can change it (the ground's top, ground.rs `height`; the ways' crossing,
  way.rs `on_way`, `crossing`, `pier_covers`):
  - in integers or fixed point;
  - or in floats that stay exact: f32 holds every integer to 2^24 and every
    fraction with a power-of-two denominator in that range, so a product of two
    12-bit values is exact.
- **Count the bits first.** A product needs the sum of its factors' bits: the
  budget is the range (how far the coordinates reach; take them local to a
  brick, not the world) plus the precision (the fraction the data really
  holds). Under 24 bits, floats stay exact; under 32, integers; beyond, a
  64-bit product from 32-bit halves (`mul_hi`, decor.wgsl).
- **No division in a decision.** `a / b < c` is `a < c · b` for `b > 0`:
  multiplication alone, exact. (Integer division is exact in WGSL but slow on
  a GPU: no hardware divide.)
- **A drawing rule goes through one function**, and its test names the ulps it
  allows and counts the cases at the edge. Two kinds go further, each for its
  own end:
  - a value several pipelines decide alike is exact, for agreement across
    pipelines (the light ray's start, in fixed point summed in i32);
  - a reciprocal or a root a frame holds constant is rounded once on the host
    and handed over in a word, which the rule multiplies by: fewer divisions,
    so cheaper (the light's `W_WIDTH` and `W_INVERSE` … `W_WIDTH_INVERSE`, the
    grid's `GRID_S_INVERSE` and `GRID_SLOPE`).
- **One source of truth** is the other way out: one side decides and the other
  reads its answer (an upload, or a readback with its latency), as the kernel
  reads the GPU's cells and ways.

## What it costs

On a GPU an integer add costs what a float add costs; an integer multiply may
be several times slower on a phone's GPU; a division is tens of instructions.
What decides is how often the code runs: filling a brick's ways or ground runs
once a brick as it lands, so exact arithmetic there costs the frame nearly
nothing; a walk's inner loop runs for every pixel every frame, and is changed
only on a measurement (the performance skill).

## How to check

- A reference test compares the GPU's words with pxl-engine's, word for word;
  every allowance is named, its cause said, its cases counted.
- grep the rule for `/`, `sqrt`, `length`, `normalize` and for float sums
  rounded or compared at a boundary.
- A survivor of a division put back means the allowance is as wide as the
  division's error: the test cannot see it.

## References

- NVIDIA, [Floating point and IEEE 754 compliance for NVIDIA GPUs](https://www.sci.utah.edu/~beiwang/teaching/cs6210-fall-2016/NVIDIA-CUDA-Floating-Point.pdf):
  FMA's one rounding, and when a compiler contracts.
- Glenn Fiedler, [Floating point determinism](http://gafferongames.com/post/floating_point_determinism/):
  what lockstep games did, and how fragile it is.
- [How to debug floating point determinism across platforms](https://bugnet.io/blog/how-to-debug-floating-point-determinism-across-platforms):
  fixed point for every path that must match.
- ALICE, [Improvements of the GPU processing framework](https://arxiv.org/pdf/2511.17018):
  bitwise CPU/GPU comparison with FMA and fast math off.
- Marinescu, [procedural terrain on CPU and GPU](https://www.cs.ubbcluj.ro/~studia-i/contents/2012-2/05-Marinescu.pdf):
  the same noise never coincides on the two.
- WGSL, [floating point evaluation](https://gpuweb.github.io/gpuweb/wgsl/#floating-point-evaluation):
  accuracy, reassociation and fusion.
