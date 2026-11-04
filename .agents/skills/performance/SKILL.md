---
name: performance
description: How a performance problem in pixelet is analyzed and a change to it judged — the contracts a change keeps, the cause found in the code, measurement only to confirm, the work removed before it is sped up, and a three-line report. Use for any profiling, performance analysis or optimization, in the renderer, the kernel, the shell or the tools.
---

# Performance

The aim is a faster program that keeps every condition the design met, not
one measured case fitted at the cost of another.

- Name the contracts and symmetries the change touches first. A measured gain alone doesn't weaken one; changing
  one is a design change, proposed on its own.
- Find the cause in the code: predict from the structure what it costs and
  why. A costly function is where the time went, not proof its body is wrong.
- `script/compare-perf.py <base url> <change url> [rounds]`
  compares a build's page performance against a base's. Close every browser
  and server in the step that opened it.
- Ask why the work happens before making it faster: done twice, data not
  needed, a conversion at a boundary, an order that forces it. A cache or a
  fast path says what it adds: state, invalidation, ownership, lifetime.
- Faster in some cases is fine; breaking another case's correctness or its
  required speed is not. Judge by the goal (frame time, GPU time, input
  response), and say what wasn't checked.
- A small change that keeps the contracts and the behaviour is fine; fewer
  copies is not safe by itself.
- The CPU↔GPU data contract is generated from `spec/` (`make spec`).
- In gpu-portable code, index no large const array at run time: the WGSL
  builds it as a local array, filled on every call.

Report in three lines: the structure kept; the cost removed and what was
added; the effect checked and the range not checked.
