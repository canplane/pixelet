---
name: performance
description: How a performance problem in pixelet is analyzed and a change to it judged — the contracts a change keeps, the cause found in the code, measurement only to confirm, and a three-line report. Use for any performance analysis or optimization, in the renderer, the core, the UI or the tools. Taking a measurement is the measure skill's; whether a shader is correct, the shader skill's.
---

# Performance

Make the program faster while keeping every condition the design met.

## What a change keeps

- Name the contracts the change touches first; changing one is a design change,
  proposed on its own.
- Judge whether a part exists by what the frame must produce, from the code. A
  measurement does not put a part in or take it out.
- Find the cause in the code: predict from the structure what it costs and why.
  A costly function is where the time went, not proof its body is wrong.
- A measurement confirms what the code predicted and is taken as the measure
  skill says ([measure](../measure/SKILL.md)).

## The record

- Each experiment gets a journal entry under `doc/experiment/`, or
  `doc/gpu/experiment/` for the GPU's: tried, came of it, how we know, kept or
  dropped.

## The report

Report in three lines: the structure kept; the cost removed and what was added;
the effect checked and the range not checked.
