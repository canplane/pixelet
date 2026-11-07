# The deployed build is the bar

**2026-11-07** — recorded with the user, in the session that compared the frame with the
deployed build and took out what the frame carried over frames. The measurements are in
[the frame on the S23](../experiment/20261105-the-frame-on-the-s23/journal.md), its 11-07
section; the decision on the frame itself is [a ray ends in its frame](./20261107-a-ray-ends-in-its-frame.md).

## What the user decided

**The deployed build is the bar, on every axis.** No part of the frame is to be worse than
the deployed build's (`e317a3644b`, what plei.me serves): the GPU's time, the picture,
the frame's interval, rest and convergence. The hot S23 is to be twice as fast; the 4K Mac
no slower. The deployed build's strengths are to be taken over where it has them.

**The gate is the repository's `script/compare-perf.py`, the profiler beside it.** Base and
change by turns, `pxl.perf.report()` and the trace for where the time goes. A measurement
another session took is not taken again. The deployed build's tree is a constant of the
comparison, kept between runs.

**The structure first, the measurement second.** Why a part costs what it costs is found in
the code; a measurement verifies it, and is taken only where the code cannot tell. The
fewest runs that verify.

**A refactor, not a layer.** Every part of the code is suspect. No setting or state is added
over an old one to keep it. The state is to be less than before; more state only where it
buys parallelism whose parts interlock exactly, as a pipeline's stages do. A part removed is
removed with its case: what it did wrong, and why the deployed build had no such part.

**The budgets are the agents'.** The user set none and judges none; what regressed goes.

**A test that holds a removed route is deleted or rewritten.**

## What was weighed

- The deployed build is `e317a3644b`, deployed on 11-04 before rust-gpu was taken out
  (`4399ef1ce5`, 11-05). Its run failed only at the smoke, on `/geo/search`; the client and
  the servers went out. The `deploy` branch still names it.
- Main's frame against it on the 4K Mac, before this session's change: the gate failed, GPU
  p95 47.9 to 78.3 ms. On the S23 at 550 MHz the deployed build took 252–324 ms a cast frame,
  main's renderer 92–100 ms.
- What the frame carried over frames held one fact, a ray not finished, in many places: two
  pools, owed and walked bits, owed words, a readback that woke frames for them, the rolled
  lattice, the light's carried tiles and its claim budget. The deployed build ended every
  ray in its frame and held none of it.
- A lane that took a word of hole bits walked up to 32 holes alone: holes that lay together
  ran on a 32nd of the lanes. The deployed build walked a hole a thread.

## What the agents decided

- The light's claim budget went with the rest of what crossed frames: every tile read and
  not cast in its epoch is claimed in the cast that reads it.
- An edit is sent with its turn's others, not after a delay (`SAVE_DELAY_MS` removed), so a
  page closed between them loses none.
- The eye's holes are a list, as the light's are; the node cache's keys and states lie apart
  from its entries.

## Open

- The S23 at 550 MHz: GPU p50 3.6× the deployed build's, p95 1.2×. The p95 is the light's,
  an epoch casting every tile it reads in one cast.
- A view left standing works more than the deployed build's: on the S23 three times the frames,
  casting the pixels again. The kernel's frame test finds it resting 18 to 28 turns after it
  stops, against a bound of `JITTERS + FRAME_LATENCY`.
- Fewer rocks at the far zoom in the drive's shots than the deployed build's.

## Accepted, 11-08

**The tick's p95, about 1 ms over the deployed build's, is the CPU fill's cost.** With the
bricks filled on the CPU, a landed tile's palette indices are cut in the turn it lands, as its
packet is kept no longer; the deployed build left that to the GPU. Within the turn's budget,
no turn past it. The user accepts it; holding packets past their turn to move the cut out is
not taken.

**Closed, 11-08.** Against `e317a3644b`: the Mac's gate passes, its GPU faster; the S23 at
719 MHz 6.1× at p50, and at p95 1.7–2× on the deployed build's best run of several, 5.7× on
one pair; the tick's tail accepted. What the S23's p95 has left is the light's walks, which
its list did not undercut ([the light's list](./20261108-the-light-s-list-measured.md)).

