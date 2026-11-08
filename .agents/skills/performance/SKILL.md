---
name: performance
description: How a performance problem in pixelet is analyzed and a change to it judged — the contracts a change keeps, the cause found in the code, measurement only to confirm, and a three-line report. Use for any profiling, performance analysis or optimization, in the renderer, the core, the UI or the tools.
---

# Performance

Make the program faster while keeping every condition the design met.

- Name the contracts the change touches first; changing one is a design change,
  proposed on its own.
- Judge whether a part exists by what the frame must produce, from the code. A
  measurement does not put a part in or take it out.
- Find the cause in the code: predict from the structure what it costs and why.
  A costly function is where the time went, not proof its body is wrong.
- `script/compare-perf.py <base url> <change url> [rounds]` compares a build's
  page against a base's.
- The devices: the S23's Chrome (Tint, Vulkan, a weak mobile GPU) for
  performance, hot; Safari on the Mac (WebKit's compiler, Metal) for the iPhone.
  Mac Chrome only shows a page running. Mobile web is the minimum; no polyfill
  or fallback for what time will fix.
- Each experiment gets a journal entry under `doc/experiment/`: tried, came of
  it, how we know, kept or dropped.

Report in three lines: the structure kept; the cost removed and what was added;
the effect checked and the range not checked.
