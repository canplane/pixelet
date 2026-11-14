# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in the
discussion the line points to. A finished item is removed, not ticked — its
record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so it
is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

## P1

## P2

- **The data-spec generator, by one convention.** xpute-gen's data specs
  follow no single rule: a table's TypeScript shape turns on its content (an
  all-number table goes to a file of its own), and enums, flags, slot numbers
  and `laid_out` offsets are inferred or set by the caller's options. The
  direction discussed: DTCG's structure (`$value`, a `$type` inherited from the
  group, `$description`, a tool's data in `$extensions`) with our own scalar
  types; one fixed form per language (Rust modules, TypeScript ES modules,
  arrays for lists) so that no key survives the build; names only where a spec
  declares them, and an ordered table written as an array; each transform
  declared in `$extensions`; the generator a crate of its own outside xpute,
  which runs it only at development time. Open: the shape of the
  `$extensions` declarations.

## Parked, with the reason on file

- **A query over what is drawn, for an agent to read.** An agent debugging the
  view guesses a pixel's cause from its color in a screenshot. The profiler's
  half is answered: the console's `pxl.perf.report()` gives its window as
  text (`ts/client/src/ui/feature/debug/console.ts`). What a screen point draws is
  not: the block its ray met, with its face, zoom and distance, and where that
  block was read from, asked of the running page and answered as text. The
  answer is the GPU's, the ray's words read back (`pxl-engine ray.rs`), since
  the CPU's `first_hit` (`pxl-core domain/query/ray.rs`) can disagree where
  the samples miss a thin face
  ([20261024-a-proxy-only-starts-the-ray.md](./minutes/20261024-a-proxy-only-starts-the-ray.md)).
  Not urgent: no debugging under way needs it.
- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against; guests
  stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./minutes/20260930-wasi-and-the-boundary.md).
- **pxl-core's test build.** Tests are off for now, and the core's test
  build does not compile; when tests resume, these are rewritten or removed:
  - `rust/client/core/src/link/export.test.rs`, which writes an f64 for a u32
    reader and names `GpuSurfaceClose`, `ui::apply`,
    `abi::Command::CAMERA_GOTO` and `State`, none of which exist.
- **C++'s core, which does not build.** `xpute/cpp`'s io and conformance
  include headers its removed core wrote (`xpute/core/abi/`, gone since
  00503422be); they are written again with the C++ core. One file says it
  is generated though nothing writes it: `dev/conformance/core.spec.mk`,
  which the Makefile includes.
