---
name: performance
description: How a performance problem in pixelet is analyzed and a change to it judged — what is a rule and what is only code, the symmetries and boundaries a change keeps, measurement kept apart from the fix, removing work before speeding it, the conditions a gain is checked under, and the three-line report. Use for any profiling, performance analysis or optimization, in the renderer, the kernel, the shell or the tools.
---

# Performance

What this guards against is not measuring, nor a local change. It is fitting
one measured case while losing what the design solved at the same time:
another case, a constraint, a symmetry. The aim is a faster program that keeps
every condition the design met together.

## 1. Rules come from records, not from the code

The code holds both debt agents left and structure the user set against it.
Neither "as the code does" nor "the number got better" decides.

- Rules, in order: the user's instructions now; `AGENTS.md`;
  `ARCHITECTURE.md` and `xpute/ARCHITECTURE.md`; the minutes under
  `doc/minutes/` (not `archive/`) and `doc/experiment/`. Read a record's date
  and scope, and whether a later one replaced it.
- How often a shape repeats in the code proves nothing; debt repeats too. An
  asymmetry is not proof of debt either. Where the records do not settle it,
  leave it uncertain and say so.

## 2. Symmetries and boundaries are the default constraint

Before changing anything, name the symmetries, invariants and layer boundaries
the change touches, and what each solves. For example: the light's cast and
the screen as one mark → reach → claim; xpute knowing nothing of pixelet; the
host giving no number a meaning; the shell computing no coordinate or tile;
`ts/` and xpute's `ts/` configured alike.

- Symmetry is counterparts explained by one principle and one contract, not
  code of one shape. A difference their roles require is fine.
- A measured gain alone never weakens one. No condition, constant or path on
  one side that its counterpart lacks.
- Changing a symmetry or a boundary is a design change: propose it on its own,
  not inside a performance patch, and say what replaces the property it held.

## 3. A measurement is evidence, not a fix

- Write down what the structure predicts it costs and why. Measuring to find
  something not predicted is fine too; no fixed order.
- Keep three things apart: what was observed, what is supposed to cause it,
  what is confirmed to. A costly function is where time went, not proof that
  its body is wrong.
- Check that what was timed is the path that runs (the frame kind, the pass,
  the span). Where prediction and measurement disagree, doubt both the
  instrument and the understanding.

## 4. Remove the work before making it faster

- Ask first why the work happens: done twice, data not needed, a conversion
  at a boundary, a layout or an order that forces it. Do not turn every
  problem into a redesign.
- A cache, a pool or a separate fast path: state what it removes and what it
  adds — state, invalidation conditions, ownership, lifetime, coupling between
  settings. Never report the gain without that cost.
- The CPU↔GPU data contract is generated from `spec/` (`make spec`); change it
  there. Check the passes' reads and writes, when a resource is reused, and
  under what condition anything kept across frames (history, stencil, read
  backs, ages) stays valid.

## 5. One case's gain is not the whole's

- Name the assumptions the change brings, and check it where they could
  break: of render scale, camera moving, animated draws, time of day, shadows,
  clouds, device, pick the axes that bear on it. No fixed matrix of every
  combination.
- Faster only in some cases is fine. Breaking another case's correctness or
  its required speed is not, and neither is claiming an effect where it was
  not checked.
- Judge by the goal (frame time, GPU time per frame, input response), not by
  one span. Say what was not checked.

## 6. Where a local change is fine

- A small change that keeps the contracts, symmetries, boundaries and
  observable behavior is fine. Fewer allocations or copies is not safe by
  itself: check ownership and lifetime.
- An unrelated asymmetry found on the way: note it as a debt candidate, do not
  extend or copy it, and do not stop the task for it. Settle from the records
  what they settle; ask only when the doubt decides the change's direction.

## Report

Short, three parts:

- the structure kept, and the record it rests on;
- the cost removed, and the assumptions and state added;
- the effect checked, and the range not checked.
