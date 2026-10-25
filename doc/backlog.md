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
  debugging the view reads the profiler panel off a screenshot: small
  text misread, past frames gone, and a hole's cause guessed from its color
  (the bedrock folded black under far faces, 2026-10-12). A query asked of
  the running kernel and answered as text — what a screen point draws (its
  lane cell, the block's zoom and node, the tile window it was read from,
  its bound, the column's blocks), the drawn blocks matching a condition —
  would let it read the state instead. The AI DSL experiment could start here. First see what
  the profiler's report already gives
  (`apps/client/src/shell/debug/profiler/report.ts`).

## Parked, with the reason on file

- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against;
  guests stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./minutes/20260930-wasi-and-the-boundary.md).
