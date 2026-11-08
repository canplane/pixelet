# The walk on a phone, and who owns it

**2026-11-03** — recorded with the user, over two sessions that compared their
numbers. Main was at `661f15a75d`, as `original` keeps it; what was measured is
`d25f5b631`, the page on both phones since
[the shaders on two phones](../experiment/20261103-the-shaders-by-hand/README.md).

Nothing was resolved. What the user said is under its own heading and is the
user's view, no order yet. Everything under The sessions' proposal is the
sessions', a proposal and no decision. No shader or renderer changed.

## What was asked

The page now draws on the Galaxy S23, too slowly to use. The Mac is fine and the
native app is unmeasured. The user reopened
[Rust and XTP, reconsidered](./20261103-rust-and-xtp-reconsidered.md)'s open
question, whether one source for CPU and GPU must hold, to break through that
limit: whether rust-gpu is worth keeping, how the walk can branch less on a
GPU's SIMT threads, and whether web and app differ.

## What the user said

- The S23's web is too slow to use, the Mac is fine, the app is unknown; this
  review is to break through that limit.
- Reduce the work as ESVO does, not by lowering resolution: why must the walk
  branch at all. Bit masks where they help.
- Most rays are pruned early. Those left could be gathered and done together
  later, a kind of priority queue: near things come quickly and far things
  roughly.
- What matters now is owning the shaders directly in place of rust-gpu, and
  optimizing the branches for SIMT, by wavefront or ESVO's way.
- Doubt, in the user's words: is rust-gpu used just to keep three entries
  agreed, giving up optimization and leaning on a black box?

The word wavefront was the session's first, not the user's.

## What was measured

Both on Chrome. The S23 is an Adreno 740, a dev build; the Mac an M1 Max, the
deployed build of the same commit. Same place (38.1193, 128.4656), canvas about
1080×1969 (2.1 Mpx), Fixed, default view (distance 16 u, polar 60), a right-drag
circle of radius 100 turning every 6 s for 25 s. The S23 ran with shadows off,
the setting's default, the Mac with them on, so the frame's ratio understates
the eye's; the spans below are the eye's on both.

| | S23 | Mac | ratio |
| --- | --- | --- | --- |
| fps | 5.9 | 60.4 | |
| GPU busy p50, ms | 154.7 | 11.5 | 13 |
| `px` p95 | 1.7 | 0.9 | 1.9 |
| `sraster` p95 | 2.2 | 0.8 | 2.8 |
| `world` p95 | 10.4 | 2.8 | 3.7 |
| `face` p95 | 6.7 | 0.6 | 11 |
| `generate` p95 | 24.9 | 1.7 | 15 |
| `beam` p95 | 55.4 | 2.8 | 20 |
| `miss` p95 | 92.1 | 2.2 | 42 |
| `sample` p95 | 133.2 | 2.7 | 49 |

- **The limit is the GPU.** The S23's turn is 6–7 ms busy, no long turn; thermal
  status 0, AP 46–47 °C.
- **The simple passes differ 2–4 times, the walks 20–49.** What separates them
  is open (below).
- **Pixels barely help.** The S23 at 7 times fewer pixels: GPU p50 154.7 to
  139.2. The Mac at 4 times fewer: 11.5 to 8.7. `beam` holds on both and `sample`
  on the S23; `miss` follows pixels on both. `Grid::of` (pxl-engine `beam.rs`)
  lays beam rays and samples from the screen's tangents and the lattice alone:
  217×389 samples at this aspect, computed from `Grid::of`, whatever the device
  pixel ratio. The lattice stayed 1 throughout. Auto's 25 s reached a scale of
  0.71, and Auto steps the scale to its least before the lattice, so that window
  says nothing of a lattice of 2.
- **The drive** (`pxl.perf.drive`'s motion, shadows on) gives GPU p50 415.6 against
  7.2, and `miss` 361.1 against 2.4, `light_walk` 324.3 against 1.6. No pure
  cost: `beam` ran on 134 of 1502 timed frames on the Mac and on 46 of 46 on the
  S23, so the slow phone redoes what the Mac's frames skip.
- **Still**, the S23 holds 60 fps at about 10 ms. The first load to the world is
  about 90 s.

**A node costs the S23 26–62 times what it costs the Mac, over the same nodes**
([the walk on a phone](../experiment/20261103-the-frame-on-the-s23/README.md), both
on one dev build with the shadows off, the Mac's GPU p50 then 7.7 ms). A cast
visits 1.7 million nodes on the S23 and 1.9 million on the Mac, 13–14 a sample.
`sample` takes 111 ns a node against 1.8, `miss` 260 against 5.8, `beam` 138
against 5.3. So the count of nodes does not tell the phone from the Mac, though a
16.7 ms frame asks under 10 ns a node at that count. A lattice of 2 halves `beam`,
adds 50–70 ms to `sample` and gains the frame nothing; Auto, both knobs at their
floor, holds 6–8 fps.

**What a visit pays** ([the same record](../experiment/20261103-the-frame-on-the-s23/README.md),
synthetic, no renderer): on the S23 a visit of 13 plain loads takes 0.53 ns, the
keys alone atomic 1.42, all atomic 9–29, a 33-entry stack at a run-time index 20
(8 entries, none), and locals that never run go from 1.09 at 125 to 434 at 500,
against the Mac's 0.30 to 0.61. Only the last reaches the real 111 ns, and it was
not measured in `sample`. Every read of the pyramid in `sample.gen.wgsl` is an
`atomicLoad`, 13 on a hit at the first slot, its buffer being
`array<atomic<u32>>`. One variant, 500 locals and all atomic, read back 0 hits of
14 on the S23, once, not traced.

Not measured: the distribution of a ray's steps, the native app, any hand-written
entry.

## What the code holds

- **A step of `cast::walk`** (pxl-engine `cast.rs`): the slab test on three axes
  and the zooms by distance; the observed pyramid's bucket probed by its key
  ([a visit reads one entry](./20261020-a-visit-reads-one-entry.md)); on a miss,
  `world::node` whole, what is placed, the ground's clip levels, the ways' tree
  and the cuts; the box test and `Until`; PUSH or POP; `Query::child` moving the
  cursors. It is already Laine & Karras's PUSH and POP. What it lacks of ESVO is
  a visit that is a word and a mask (from memory: a child descriptor, valid and
  leaf masks, coordinates mirrored so a ray's direction is positive).
- **The stack**: `[at; DEPTH]` (`cast.rs`, `walk`), 33 entries of 16 bytes, filled
  at each walk's start and indexed at run time.
- **As the phones get it**, with every entry inlined for Adreno:
  `sample.gen.wgsl` is 12,640 lines, 762 `if` lines, 448 `select`, 49 `loop`,
  2,208 function-scope `var` of which 2,009 are `phi_*`. `beam` and `miss` are
  the same size. All generated: 35 files, 143,767 lines, 8,988,078 bytes,
  against 4,603 lines of hand-written WGSL and 34,589 lines of vendored codegen.
- **The path**: Rust, SPIR-V, spirv-opt, naga, WGSL, `rewrite.rs`, Tint, the
  driver. `rewrite.rs` alone is ours to shape, after the fact.

### What one source is for

Static reach over rust-analyzer's SCIP index, non-test functions of pxl-engine
and pxl-content, 7,313 lines (no run-time count; traits matched by name). The GPU
reaches 6,224: 4,136 also on the client's CPU, 1,017 also on the server or a
bake, 1,071 on the GPU alone. Of the 4,136, about 2,758 are lost to the client
when its `domain/query` is taken from the graph: the same computation on both,
entered at three places. The rest is mostly layouts and small math, with some
real computation among it (`clipmap::fold`, `bound_over`). A row's lines are what
that file alone being taken loses.

| CPU entry | Shared | Lines (overlapping) | For |
| --- | --- | --- | --- |
| `query/node.rs` | `world::node` | 1,464 | every CPU query of a node: edits, the ghost, and the walks of `query/ray.rs` (its `Walker`'s `node`) |
| `query/ray.rs` | `cast::first`, `support::column_surface` | 747 | picking, the camera's collision, markers |
| `query/scatter.rs` | `generation::placement`, `scatter_at` | 433 | decor |

## The sessions' proposal

**The walk need not be the one source.** A pick and the cast must read one world
and agree on what it answers; they need not run one loop. So `cast::first` can
leave what binds the GPU's walk to the CPU's, and what still asks for one source
is the world's definition, `world::node` and `placement`, about 1,900 lines.

**Owning the shaders and branching less are one job.** Take `world::node` out of
the hot loop and what is owned by hand is a few hundred lines.

- **A traversal kernel apart.** It reads the observed pyramid alone. Rays that
  miss go on a queue; a separate dispatch asks `world::node`, fills the pyramid,
  and the rays go on. A hit no longer carries a miss's code.
- **N steps, then compaction.** All rays walk N steps, those left are gathered
  and go on, so a long ray holds no group back. The user's priority queue sits on
  this: what is near first. This is the sense the session first meant by
  wavefront; the kernel apart is another, and the two go together.
- **No stack in the kernel.** An entry of the pyramid holds its `Inheritance`, so
  an ancestor may be found by its key. Unchecked.
- **ESVO's mask.** A child-occupancy mask in the entry skips air by bits;
  mirroring removes the branches on direction (`past`, `crossing`, `up`). Its
  worth rests on what a visit costs and how many it skips; the mean is 13–14 nodes
  a sample, the distribution unmeasured.

Against it: dispatches and queues are added, whose cost on the S23 is unmeasured.

| | Structure | rust-gpu | The cost |
| --- | --- | --- | --- |
| A | The traversal kernel by hand | on `world::node` and the cold passes | two walks, held equal by tests |
| B | The GPU all by hand | gone, with the vendored codegen | the world defined twice, held equal to the bit (the caves' noise) |
| C | The CPU builds a compact tree and uploads it; the GPU only traverses | gone, and no second definition | the renderer redesigned; WASM's build time and the upload on a phone; an edit's latency |

C dissolves the question: the world is defined in Rust alone and the GPU's walk
is a few hundred lines. It reverses that the GPU generates the world
([the GPU casts a ray a pixel](./20261014-the-gpu-casts-a-ray-a-pixel.md)).

**Recommended next**, each on the real frame in a worktree, `sample` on the S23
against its 111 ns a node. The stack, the atomics and the cold code all cost past
a cliff, not by the step, which reads as a shader outgrowing what the driver keeps
in registers; that is a reading, not measured.

1. **A short stack and the keys apart, within rust-gpu.** `cast::walk` keeps a few
   entries and, past them, finds what was handed a node again from the top
   (`Inheritance::of`), which draws the same; the keys take their own atomic buffer
   and the rest a read-only binding. No contract moves. What stays of the 111 ns is
   then the cold code's.
2. **The miss apart, within rust-gpu.** The kernel apart of the proposal, as Rust:
   a hit's loop in an entry of its own and `world::node` in another. It removes the
   cold code with one source kept for all but the walk's shape.
3. **The same kernel by hand**, against step 2: what is left between them is what
   rust-gpu and naga's shape costs, the measure A, B and C wait on.

### What it touches

The stated contract is one source: `README.md`, and ARCHITECTURE.md's descent "by
one function the CPU and the GPU compile alike". A and B change it, a design
change on its own. C keeps its point, one definition, and drops its mechanism.
"One answer, every reader"
([why rays](./20261031-why-rays-and-what-makes-them-fast.md)) holds under all three
only if the tests hold it.

[Rust and XTP, reconsidered](./20261103-rust-and-xtp-reconsidered.md) recommended
keeping rust-gpu because nothing mature replaces it. That weighed tools for one
source; this asks whether the hot path needs one.

## Open

- A's experiment, and whether to weigh C (first, what a CPU build costs on a
  phone).
- Which cause the walks' 26–62 times a node is, the count of nodes being the same
  on both: Chrome's path (Tint, robustness, zero-init), the shape rust-gpu and naga
  give the code, the visit's atomics, or Adreno itself on such a loop. No
  evidence yet blames rust-gpu's run time; what inlining every entry cost a frame
  is unmeasured, as the S23 ran nothing before it.
- How much of `sample`'s 111 ns is its stack, its atomics and its cold code. It
  reaches `world::node` by `observer::sample`, `resume_walk`, `cast::walk` over
  `Writing` (pxl-shader `pyramid.rs`), and on a miss asks it and claims a slot. How
  many of its 2,208 locals are live across the loop is unmeasured, and how far
  `split_phis` and inlining every entry, made to pass Adreno's pipeline, raised them.
- Whether the S23 answers wrong past the cliff: the 0 hits of 14 above.
- Web against app on the S23, the same WGSL.
- `doc/tip/shaders-phones-take.md`'s 5.5 MB is stale; the count above is the
  present one.
