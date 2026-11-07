# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in the
discussion the line points to. A finished item is removed, not ticked — its
record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so it
is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **Faster than the deployed build, on every axis**
  ([the deployed build is the bar](./minutes/20261107-the-deployed-build-is-the-bar.md)). The
  Mac's gate passes, its tick p95 2.1 to 3.2 ms the one axis worse, its cause not found. The S23 at 719 MHz: GPU p50
  6.1× the deployed build's, p95 5.7× on one pair, the deployed build's p95 reading 437–1282 ms
  ([the journal](./experiment/20261105-the-frame-on-the-s23/journal.md), 11-08).

## P1

- **The light by a list and the raster**, decided
  ([the light is listed](./minutes/20261108-the-light-is-listed.md)). First `list::light` on the
  CPU, held to `cast::first` texel by texel, which measures the real list; then the GPU's list
  and raster in place of the light's beam, samples and holes' walk.

## P2

- **The vocabulary, cleaned.** Agents have left words with several meanings
  (`level`, `chunk`, `frame`), several words for one thing, and names of what
  happened. In order: every domain noun listed with what it names (read-only, by
  area), a glossary decided with the user, conventional names first and cited, put
  in the naming skill; then the renames by area, one names commit each.

## Parked, with the reason on file

- **A query over what is drawn, for an agent to read.** An agent debugging the
  view guesses a pixel's cause from its color in a screenshot. The profiler's
  half is answered: the console's `pxl.perf.report()` gives its window as
  text (`ts/client/src/ui/debug/console.ts`). What a screen point draws is
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
