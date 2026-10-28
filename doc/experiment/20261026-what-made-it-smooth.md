# What made it smooth

**2026-10-26.** Not measured: what is on file, so it can be measured later
and a regression of it noticed. The user, at main with 1c5455121, after the
light-wedge fix folded into it: moving, turning and zooming had become very
smooth, with shadows on as well, where earlier builds that day were not, and
by feel the frame is better than the 2026-09-26 chunk-mesh build
(e78eeec55). Nothing in this record attributes it yet.

## The candidates

The user's suspicion, and the likeliest: three consecutive commits, all
caching or sharing of discovery, no raster in any of them.

- **8d5d3980e, the observed pyramid keyed by the node**, a bucket of four by
  its hash, a victim taken where none is free. Before it, nodes lived in
  tiles taken off a stack, found only by walking down from a root, and the
  stack could run out: a new view's nodes then went unwritten and every ray
  walked the world afresh, in the frames moving makes most of. A table never
  runs out, keeps the nodes asked every frame (four slots, not one, where a
  seventh of them evicted each other), and finds a node by its key alone.
- **2f6436d03, beam rays in three passes**, coarser corners first, each pass
  trusting what the earlier wrote. A node no earlier frame held was asked of
  the world by every beam ray that reached it in the pass; now once, by the
  coarsest pass. That is the cost of a frame after a move.
- **1c5455121, the sun's walks on the observed pyramid** and the eye's line
  machinery. The sun walked the world from scratch every frame its map was
  cast; it now reads what the eye's rays or an earlier frame asked. On the
  start view its spans are each under 1.5 ms p95 with 96.9% of slots taking a
  sample, where before the day's sun rework `shadow` was 60.2 ms p95 and the
  interval 216 ms against 77 with shadows off
  ([the shadow map](./20261026-the-shadow-map.md)).

Also before the felt change and possibly part of it: e04a3a855 (the shadow map
cast as the eye's rays are, its keep policy gone), 806af7525 (faces asked once
a box), de2f73c26 (missed pixels from a list), 21f72517b (the raster keeps),
7778b87f0 (four occlusion samples). After it, not part of the feel but of the
same kind: 403ddd70d (an edit forgets only the nodes it changed), f94ee59b2
(the sky taken off the samples, and beams passing a slab they clear: at the
horizon, polar 77°, GPU p50 40 to 29 ms, `beam` 17 to 10, `pixel` 18 to 5,
`miss` 10 to 1.6, 1d's loads).

## What reuses what, at f94ee59b2

Each is discovery shared or kept, none geometry per square or column. The
effect of none is measured alone.

- **The observed pyramid, across frames.** The world's answer per node, kept
  until what it read moves (forget_cs: moved squares, edit and cut boxes,
  families written). Off: forget all every frame.
- **The beams' passes, within a frame.** A node asked once a frame by the
  coarsest pass reaching it. Off: one pass.
- **Beams to cells to samples to pixels.** A pixel takes a sample's block as
  the visible sample (`taken`), or the samples' sky; only the rest walk,
  listed (`miss_list_cs`). Off: every pixel walks from its cell's start, 4.5
  times the cast ([a proxy only starts the ray](../minutes/20261024-a-proxy-only-starts-the-ray.md)).
- **Faces asked once a box**, for every pixel that shows it.
- **The sun on the eye's pyramid and lines**, and its map recast only when
  its inputs move (light frame, sun words, eye origin, τ, range, world).
- **Empty space skipped**: an air answer passed whole, a split passed whole
  where the ray clears its content slab (ESVO's contour), beams too since
  f94ee59b2.

## What holds it

What a change can quietly undo; each is in a commit message and the code.

- No key exhaustion but the frame tag's: `observed::FRAMES` with `PASSES` 8,
  a forget-all about every 36 minutes at 60 fps. A new pass tag shortens it.
- A slot holding the same node at another zoom is written over only where an
  earlier frame wrote it, so the eye and the sun do not evict each other
  every frame.
- The sun writes no filled block's answer (`Writing::blocks`), and writes air.
- A dropped frame carries its forgetting to the next, not a forget-all
  (403ddd70d); a forget-all on a moving frame is a full re-walk.
- A walk trusts no entry its own pass writes; each pass has its own tag.

## Measuring it later

On the fixed path ([the cast on a fixed path](./20261026-the-cast-on-a-fixed-path/README.md)),
shadows on and off, at e78eeec55 (9/26), 806af7525, e04a3a855, 8d5d3980e,
2f6436d03, 1c5455121 and the tip of the day: interval p50 / p95, GPU p50 /
p95, and the cast's and the sun's spans. Each reuse above, alone, by its
"off" against the tip. The pyramid has no hit counter; its ablation is the
forget-all every frame.
