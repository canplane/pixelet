# rust-gpu goes

**2026-11-03** — recorded with the user. The branch was at `05e8ae4a5`, on
`d25f5b631`, the deployed commit.

## What the user decided

**rust-gpu is to be removed.** Asked whether that was a direction to confirm by a
measure or a decision, the user answered that it is a decision. It closes the
question left open by
[Rust and XTP, reconsidered](./20261103-rust-and-xtp-reconsidered.md), whether one
source for CPU and GPU must hold: not by rust-gpu.

The contract that changes is one source: `README.md`, and `ARCHITECTURE.md`'s
descent "by one function the CPU and the GPU compile alike". The word applies now;
those documents are changed once what replaces it is settled.

## What stood behind it

The user's doubt of the same day: rust-gpu kept to agree three entries, giving up
optimization and leaning on a black box
([the walk on a phone](./20261103-the-walk-on-a-phone-and-who-owns-it.md)). Not that
it was measured slow: no hand-written shader has been compared, and that remained
so at the decision. What was seen is control. A cut of the stack from 33 entries
to 8 left `sample.gen.wgsl` with 2,201 function-scope variables of its 2,208, whose
shape is naga's to choose, on a phone where 500 such variables that never run cost
400 times what 125 do in a synthetic pass.

## What is proposed, and not decided

The sessions' order: the traversal kernel by hand first, its keys apart from the
pyramid's words and no stack past what it needs, measured on the S23 against
`sample`'s 111 ns a node. This replaces moving the kernel apart in Rust first.

What the rest becomes is open:

| | Structure | The cost |
| --- | --- | --- |
| B | every shader by hand | the world defined twice, in Rust and WGSL, held equal to the bit by tests (the caves' noise) |
| C | the CPU builds a compact tree and uploads it; the GPU only traverses | the renderer redesigned; WASM's build time and the upload on a phone; an edit's latency |

A shader at a time, not 35 at once, so each step is measured and can be undone.
