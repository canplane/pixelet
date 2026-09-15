# The safety valves the module did not have

**2026-09-14** — With the builds, the scheduler, the range mount, decor and
the raycast in the module, the dev build was slower than the deployed b7
and left pages empty: a page never rendered, a placement was in the store
and under the raycast but not on screen, the fill after a load did not
spread out from the centre, and a zoom stalled until the next input. None
of it was the Rust being slow. Each was a thing the TypeScript had relied
on without naming it, which the port had not carried because it lived in
the runtime around the code rather than in the code. This records what
they were, how each was found, and the numbers, so the next move does
not rediscover them.

## What was found, in order

1. **Nobody asked for the next frame.** R3F runs `frameloop="demand"`;
   a chunk node used to be a React child, so mounting one invalidated the
   canvas. Now a mount happens inside the module's tick and a node is a
   plain object. The tick asked for a frame only while something was
   pending, committed or blending — and a chunk just mounted is none of
   those until the full recheck, which accumulates 0.15 s of _frame_
   time. So: mount tick, nothing pending, no frame, nothing until the
   user moved. Found by driving the tick in a background tab with
   `requestAnimationFrame` polyfilled and reading the census. Fix:
   `view_dirty` in the scheduler — the view moving, a chunk coming or
   going, a source told, a visibility change — held until a full recheck
   runs under budget; `draw` while it stands.

2. **One page past the page section.** 32 resident pages at 4 MB of
   ground lanes and a 16 KB elevation block each is 128.5 MB; the desktop
   section was two regions, 128 MB. The last page to arrive spilled to a
   plain region. The TypeScript build painted from such a page; the
   module cannot read it, and `core_build.source` sent `page: None`, so
   every chunk of that page waited on a page that never came. Found by
   listing the pending chunks: 35, all one page, runs 0, tiers 0, no slot
   holder, no refusal. On a phone the section was one region: 15 of 32
   pages would have been outside. Fix: three regions on the desktop and
   two on a phone, and `RESIDENT_PAGES` derived from the section's size
   so the count cannot exceed what the module reaches; a warning once if
   a page is out of reach anyway.

3. **The pool refused and nothing answered.** The TypeScript pool never
   refused: past its range it spilled to the JavaScript heap ("heap
   peaked at 522 MB"). The module's pool refuses, and the reclaim answered
   to the retain budget and to the section being 90 % full — neither was
   true with a hundred megabytes free in pieces too small for a fine
   tier's block. `build … 88472 refused a block` in a minute; on-screen
   chunks waited six seconds. Fix: a refusal since the last pass is
   pressure — off-screen work reclaimed farthest first, and when there is
   none (a zoomed-out view has a thousand chunks in the frustum, each
   keeping its runs and up to four tiers), the spare tiers and runs of
   on-screen chunks not mid-build, never the tiers a layout shows. The
   retain budget is half the mesh section rather than a quarter of the
   whole reservation. After: 43 refusals in the same window; a zoom-out
   to 1,167 chunks in the frustum settles in 3 s with 487 refusals, then
   none.

4. **Mount storms and a full ring.** A page came into range as 256 nodes
   in one tick — 99 ms in the visible step. 64 a frame now. The
   completion ring held 1024 entries and a tick that found events still
   queued did nothing; 4096 entries, and the glue reads the ring dry
   after a tick.

## What was not the cause

- **three.js.** The scene graph costs 1.9 s of a 27 s profile
  (`updateMatrixWorld` and friends over 2,500 groups) and goes with the
  WebGPU renderer; it made nothing stall.
- **The Rust.** Release build; a bake is 2–5 ms; the tick is 0.22 ms at
  rest after the sort keys were aged once and the slots walked by index.
- **The dev proxy** is slow on a cold edge — terrain p50 5.5 s on one
  load, 121–149 ms warm — which is why the centre page arrived last and
  the fill did not spread from it. That is `dev:remote`, not the client.

## How to look next time

- In a background tab, `requestAnimationFrame` polyfilled with
  `setTimeout` keeps the tick running; `CORE_BUILD.census()` and
  `.stat()` read the module.
- `__pxl_ledger_log`, `chunk_nodes` and `landing_log` went with the
  TypeScript client on 2026-09-15 (apps/web1 keeps them); the ledger's
  report is the kernel's (`module/world/sched/log.rs`).
