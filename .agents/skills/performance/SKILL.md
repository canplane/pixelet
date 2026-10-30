---
name: performance
description: How a performance problem in pixelet is analyzed and a change to it judged — the contracts a change keeps (by AGENTS.md's Reading and changing), measurement kept apart from the fix, where a cost comes from and removing the work before speeding it, the state and invalidation a change adds, the conditions a gain is checked under, and the three-line report. Use for any profiling, performance analysis or optimization, in the renderer, the kernel, the shell or the tools.
---

# Performance

What this guards against is not measuring, nor a local change. It is fitting
one measured case while losing what the design solved at the same time:
another case, a constraint, a symmetry. The aim is a faster program that keeps
every condition the design met together.

## 1. The contracts come first

Which records bind, how the code is read against them, and how a symmetry or
a boundary is kept are `AGENTS.md`'s (Reading and changing). For performance
work that means "the number got better" decides no more than "the code does
it".

Before changing anything, name the contracts the change touches and what each
solves. In the renderer, for example: the light's cast and the screen as one
mark → reach → claim, each deciding on the CPU whether to work at all and on
the GPU which tiles; the host giving no number a meaning; the shell computing
no coordinate or tile.

- A measured gain alone does not justify weakening a shared contract or
  symmetry. For a condition, a constant or a path on one side only, say why
  the difference is needed and how the shared contract is kept. If it changes
  that contract, it is a design change, as `AGENTS.md` says.

## 2. A measurement is evidence, not a fix

- Write down what the structure predicts it costs and why. Measuring to find
  something not predicted is fine too; no fixed order.
- Keep three things apart: what was observed, what is supposed to cause it,
  what is confirmed to. A costly function is where time went, not proof that
  its body is wrong.
- Check that what was timed is the path that runs (the frame kind, the pass,
  the span). Where prediction and measurement disagree, doubt both the
  instrument and the understanding.

## 3. Where the cost comes from, and the work removed first

- Ask first why the work happens: done twice, data not needed, a conversion
  at a boundary, a layout or an order that forces it. Do not turn every
  problem into a redesign.
- A cache, a pool or a separate fast path: state what it removes and what it
  adds — state, invalidation conditions, ownership, lifetime, coupling between
  settings. Never report the gain without that cost.
- Each invalidation condition is a place the change can be wrong: check each
  on the axes below that can trip it (a resize, a dropped frame, a counter
  that wraps, a read back that lands late).
- The CPU↔GPU data contract is generated from `spec/` (`make spec`); change it
  there. Check the passes' reads and writes, when a resource is reused, and
  under what condition anything kept across frames (history, stencil, read
  backs, ages) stays valid.

## 4. One case's gain is not the whole's

- Name the assumptions the change brings, and check it where they could
  break: of render scale, camera moving, animated draws, time of day, shadows,
  clouds, device, pick the axes that bear on it. No fixed matrix of every
  combination.
- Faster only in some cases is fine. Breaking another case's correctness or
  its required speed is not, and neither is claiming an effect where it was
  not checked.
- Judge by the goal (frame time, GPU time per frame, input response), not by
  one span. Say what was not checked.

## 5. Where a local change is fine

A small change that keeps the contracts, symmetries, boundaries and observable
behavior is fine. Fewer allocations or copies is not safe by itself: check
ownership and lifetime.

## Measuring in the app

- Serve the client from your own checkout or worktree on a port of its own,
  so no other session's dev server or tab reloads: `make install`, then
  `CARGO_TARGET_DIR=rust/target/session/<id> pnpm --dir ts/app/web dev
  --port <port> --strictPort`. Tiles and assets come from the published
  deployment, so the server needs the network (outside the sandbox).
- A new origin has no location permission: place the view by the search.
- Turn the profiler on (Settings, debug, profiler); it records only while on.
  In the page's console, `pxl.profiler.report()` is its window as text (frames
  by kind, GPU p50 and p95, each span's p95 over the frames that ran it), and
  `pxl.profiler.trace()` saves the window for ui.perfetto.dev, a download.
- An A/B is one change apart, the baseline built from the same tree with that
  change undone, the dev server rebuilding the guest: the same place, zoom,
  moves and window length on both. A span runs only on the frames that need it
  (decor's `generate` as the view moves, not at rest), so the moves decide
  what is measured. Say how many runs and what was not held equal.

## Report

Short, three parts:

- the structure kept, and the record it rests on;
- the cost removed, and the assumptions and state added;
- the effect checked, and the range not checked.
