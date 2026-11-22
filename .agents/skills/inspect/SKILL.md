---
name: inspect
description: Find what the running app draws at a point and where that was read from, by asking the page instead of reading a screenshot's colors — the console's query, what its answer says and where it stops being exact. Use when a picture looks wrong, before a cause is guessed, and when a change is to be seen in the app. Whether a shader is correct is the shader skill's; what a frame costs, the performance skill's; taking a measurement, the measure skill's.
---

# Inspecting what is drawn

A pixel's cause is asked of the page. A color in a screenshot says what was
drawn and nothing of why.

## Asking

Run the app on the web ([CONTRIBUTING.md](../../../CONTRIBUTING.md#running-on-the-web)).
Ask the user before taking the browser (AGENTS.md). In the page's console:

```js
await pxl.view.at(x, y)   // a point of the view, in CSS pixels from its top left
await pxl.perf.report()   // the profiler's summary, with that line for the pointer
```

`at` moves the pointer to the point and answers once the words of a frame no
older than the one then drawn have landed, or after two seconds that none
did. It needs no recording. `report` reads once and does not wait, so its
line may be empty until a later call.

## The answer

One line, the GPU's own words for the ray at that drawn pixel (pxl-engine
`ray.rs`), so it is what was drawn:

- `coord`, the point met, and `distance`, in units from the eye; `nothing
  met` where the ray met nothing.
- `asset`, the id of what it stopped on (its kind over its index), and `fill`,
  the eighths of its cube the cell keeps.
- `drawn at`, the zoom of the cell, `-` where nothing was met: coarser than
  the place deserves means the eye asked it so or the finer tile is not held.
- `face`, the side the ray entered by, then the word of what lies around it.
- `read from`: how the world answers that cell, `exact`, or `coarser` or
  `lack` with the zoom of the tile it lacked; `-` for a cell coarser than
  any the query is asked.
- `frame`, the renderer's number of the frame the words are of.

## Where it stops

- The point lies within half a drawn pixel of what the ray met: the cast aims
  through the pixel's middle moved by a jitter that is not kept.
- `read from` is the world as it is at the read, not as at `frame`: a tile may
  have come between.
- While frames are drawn the words are a frame or two behind the screen;
  `frame` says which.
- One point an ask. A region is asked point by point.

## Then

Take the cell and its answer to the code: a wrong `asset` is the world's
answer or the bake's, a wrong `drawn at` the eye's size or a tile not held, a
right answer drawn wrong the shader's ([shader](../shader/SKILL.md)).
