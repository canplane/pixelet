# rust-gpu goes

**2026-11-03** — recorded with the user. The branch was one commit past
`d25f5b631`, the deployed commit; that commit's change is `5e908d68e8` on
`original`.

## What the user decided

**rust-gpu is to be removed.** Asked whether that was a direction to confirm by a
measure or a decision, the user answered that it is a decision. It closes the
question left open by
[Rust and XTP, reconsidered](./20261103-rust-and-xtp-reconsidered.md), whether one
source for CPU and GPU must hold: not by rust-gpu.

The contract that changes is one source: `README.md`, and `ARCHITECTURE.md`'s
descent "by one function the CPU and the GPU compile alike". The word applies now;
those documents are changed once what replaces it is settled.

**Freedom of expression is what makes the black box.** The user's word, against
`encase` as against rust-gpu: what lets a shader be written in free form leaves a
layer behind it that nobody reads, and the layouts stay explicit, in words and vec4s
with their offsets generated from `spec/` and pxl-engine. That freedom was worth its
price for the three weeks the structure kept turning over; the hot path now wants
control, and what still changes often (`world::node`) may keep it longer.

**The web build stays.** The user will not give it up: an app has to be installed,
which is a barrier to entering. So what replaces rust-gpu has to run well through
WebGPU in a browser, whatever the native app might do besides.

## What stood behind it

The user's doubt of the same day: rust-gpu kept to agree three entries, giving up
optimization and leaning on a black box
([the walk on a phone](./20261103-the-walk-on-a-phone-and-who-owns-it.md)). Not that
it was measured slow: no hand-written shader has been compared, and that remained
so at the decision. What was seen is control. A cut of the stack from 33 entries
to 8 left `sample.gen.wgsl` with 2,201 function-scope variables of its 2,208, whose
shape is naga's to choose. Later that day the S23 said what counts: not the count
of locals, 6,000 of them scoped to code that never runs costing nothing, but the
words live across the loop. It holds to 150, rises at 175, refuses a pipeline at
200 and takes 80 to 240 ns a visit past that, in a synthetic pass. `sample` peaks at
301 at `d25f5b631` and at 207 with the 8 entries, its 111 ns a node unmoved
([the record](../experiment/20261103-the-frame-on-the-s23/README.md)). So the shape
that matters is what stays live, which naga and the inlining made for Adreno both
bear on; rust-gpu's share of it is not measured.

## What is proposed, and not decided

The sessions' order: the traversal kernel by hand first, its keys apart from the
pyramid's words and no stack past what it needs, measured on the S23 against
`sample`'s 111 ns a node. This replaces moving the kernel apart in Rust first.

What must agree between CPU and GPU is about 1,900 lines, `world::node` and
`placement`; the walk need not (`cast::first`). What the rest becomes is open: B or C
of [the walk on a phone](./20261103-the-walk-on-a-phone-and-who-owns-it.md), every
shader by hand or a compact tree the CPU builds, or D:

| | Structure | The cost |
| --- | --- | --- |
| D | no CPU entry: picks off the visibility buffer, edits and decor by readback; the world defined on the GPU alone | a click, the camera's collision and an edit asynchronous, a frame or more late; the sync queries not yet counted |

A shader at a time, not 35 at once, so each step is measured and can be undone.

On 2026-11-05 the user decided every compute pass is WGSL by hand, held to pxl-engine
word for word ([the shaders by hand](./20261105-the-shaders-by-hand-held-to-the-engine.md)).
