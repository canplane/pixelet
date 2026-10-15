# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

Nothing now.

## P1

Nothing now.

## P2

- **A query language over what is drawn, for an agent to read.** An agent
  debugging the view reads the performance overlay off a screenshot: small
  text misread, past frames gone, and a hole's cause guessed from its color
  (the bedrock folded black under far faces, 2026-10-12). A query asked of
  the running kernel and answered as text — what a screen point draws (its
  lane cell, the block's zoom and node, the tile window it was read from,
  its bound, the column's blocks), the drawn blocks matching a condition —
  would let it read the state instead. The AI DSL experiment could start here. First see what
  the overlay's report already gives
  (`apps/web/src/feature/debug/profiler/report.ts`).

## Parked, with the reason on file

- **How the samples are drawn**, held until the user says the time is
  right. On the iPhone 13 mini, four shapes over the same tiles and views: today's boxes, the same as masked faces, a ray a
  pixel, and a bounded observation a cell — what the rule asks for, a cell
  being an angular footprint and not a sample point. Each shape is built
  whole on the CPU and whole on the GPU, one representation either way:
  CPU or GPU is a question of the whole drawing, never of one kind of
  thing sent to the other side. The steps a cast pays
  are measured already, and where on the screen they lie
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./decision/20260928-what-is-drawn-is-what-the-screen-covers.md),
  "the steps, measured").
- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./decision/20260930-wasi-and-the-boundary.md).
