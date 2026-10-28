# A visit reads one entry

**2026-10-20** — ordered in the conversation: recorded, then carried out as
written and nothing beyond it. The checks below came back with their counts.

**Status (2026-10-24): built, as the observed pyramid** (pxl-spec-core
observed.rs). It is filled where the rays asked: the beam rays, the samples, and
the pixels whose reading walk missed (`ray::MISS`), walked again from their
cell's start in the same pass (pxl-shader cast.rs `miss_list_cs`, off the list
`cast_cs` makes of them). An entry's key is its walk's tag and the zooms asked
(`key(tag, [far, near])`); entries are kept between frames and forgotten over
the squares that moved (`forgets`), or whole where what every node is asked of
moved (`FORGETS_ALL`). It is not the open-addressed Morton table sketched under
The structure: nodes are kept in tiles of eight siblings under the world's roots
(`keeps`), each entry naming its children's tile. _Since 2026-10-26 it is a
keyed table after all: a bucket of four slots by the hash of the node's zoom and
index (`entry_of`), the node's words in the entry telling it from another of the
same bucket, one finding no free slot taking one over (`victim`); no tiles, no
stack, no children's word, and no larger than the tiles were._

A ray asks the world whether a node is empty
([a ray asks the world](./20261013-a-ray-asks-the-world-whether-a-node-is-empty.md)),
one a pixel on the GPU
([the GPU casts a ray a pixel](./20261014-the-gpu-casts-a-ray-a-pixel.md)). This
records why that cast misses the deploy gate, and the structure that would fix
it.

## What failed

The ray a pixel holds. What fails is one hypothesis on top of it: that a visit
may compose the query whole.

- **Visits are already log n.** At E120 (dpr 2, 8,129,352 rays) a ray visits 18
  nodes on average. That is 1.46 × 10⁸ visits in a cast of about 105 ms
  (`45a52595a`; the log was not kept).
- **A visit is heavy.**
  - On the M1 Max (4,096 lanes, about 1.3 GHz) that is about 3,800 lane-cycles a
    visit.
  - The cast's walk loop compiles to 4,697 MSL lines: 3,135 statements and 44
    buffer read sites a pass (the walk's MSL, lines 1333–6029, not kept).
  - A SIMD group runs every branch any of its lanes takes, so a visit costs
    about that whole body.
- **The budget.**
  - The gate is p95 ≤ 16.7 ms. A cast of 12 ms over the same visits leaves about
    435 lane-cycles a visit.
  - The gap is the 9× still needed, and it is all in the weight of a visit.
- **Why a visit is heavy.** At every visit `world::node`
  (`rust/spec/core/src/world.rs`) composes the world again from its sources:
  - what is placed;
  - the ground over the zooms asked (`window::bound_over`), its held entry, its
    fallback and `held_of`;
  - the caves' hash and the shard;
  - the ways' cursor, rerooted where the square changes;
  - the three slabs joined.

  > Since 2026-10-27: each source gives a box in the node's own steps, and the
  > three boxes are joined
  > ([a node is asked in its own units](./20261027-a-node-is-asked-in-its-own-units.md)).

  That is a fact of the node, not of the ray. Every ray through the node works
  it out again, every frame. Descent reuses what it established within one ray,
  and nothing between rays or between frames.
- **What is not the cause.** Trimming the walk's carried state (`45a52595a`)
  moved the cast about 4%, within load-to-load spread. Occupancy work on this
  body tops out at 384 → 512, an estimated 1.3–2×.

## The structure

**One place, one probe.** One table holds every node's answer, already composed:

- **Key:** the node's zoom and corner, as a Morton key. Open addressing; a visit
  probes it once.
- **Value:** the node word. It says whether the node is empty or split, and for
  a block, its value and its four looks.
- **Sparse.** A node is in the table only where its answer differs from its
  parent's: no leaf is made unconditionally. A missing child holds its parent's
  answer. The walk carries its parent's word down, so a miss needs no second
  question.
- **Log n, no re-query.**
  - A visit is one probe and then PUSH, ADVANCE or POP, as in ESVO.
  - What a walk passed is not walked again.
  - Value and four looks are read once, where the ray stops.
- **Source-blind.** The table holds only composed answers, so the ray cannot
  tell ground from ways from what is placed.
- **One function.** The table is filled by the same `world::node` the CPU asks.
  One rule stays one function, and the table is its output stored.

**Eye-independent answers.** Today a coarse node unions the zooms asked across
it, `[max(far, z + 1), near]`. Those zooms depend on the eye, so that answer
cannot be stored. What is stored is the node's own:

- the world at its zoom (its fold);
- where anything under it may fill.

The ray picks its zoom by distance, as now (`grid::cell_at`), and never descends
past the zoom it asks. So a looser bound can only add visits, and only above
that zoom. The block it stops on is the same.

**Filled where the sources land.** The table is composed in the passes that
already run when a square arrives on the GPU (`height_cs`, `bound_cs`,
`way_cs`, `joined_cs`), and where something placed changes. The image is then a
function of the world and the eye alone.

## Alternatives weighed

- **Filled where the sources land: chosen.** The image stays a function of the
  world and the eye. One table is written at arrival and read at every visit, as
  in voxel hashing (Nießner, Zollhöfer, Izadi, Stamminger 2013).
- **Filled on a ray's request: rejected.** In GigaVoxels (Crassin, Neyret,
  Lefebvre, Eisemann 2009) a ray that misses requests the node, a producer
  composes it into a node pool, and the next frame reads it.
  - **Gain:** only observed nodes are made.
  - **Loss:** a node filled a frame late shows a frame late.
  - Filling within the frame and resuming each ray from its stop avoids that,
    but adds passes. It is the structure taken if check 4 below rules out
    filling where the sources land.
- **Kept as is, query trimmed: rejected.** Reading only the window entry a
  visit, and the value and four looks at the stop, was estimated statically at
  1.3–2×. That is short of the gate.

## Checks before build

Each check comes back as a count or a counterexample. They are counted from code
and data, never estimated.

1. **The table's size, by source.** Count the nodes that differ from their
   parent in the eye's windows at E120, W120 and E600: the ground, the ways,
   what is placed and the caves.
   - Caves are the open risk. A carve under the ground differs from the full
     ground over it, wherever no ray can reach it.
   - If the caves alone make the table unbounded by the windows, that is the
     counterexample to filling where the sources land.
2. **One word holds every answer.** Every answer `world::node` gives must fit
   the node word: kind, value, four looks and slab. The state it threads must
   leave no answer behind: `Found`, the ways' trail, `Cave`, what is placed and
   the shard. An answer that does not fit is a counterexample.
3. **The stored bound against 20261013.** 20261013 lets a source "answer more
   cautiously, which only splits more", but never "from detail finer than the
   zoom asked".
   - A bound of anything under the node, at any zoom, is read from finer detail
     than a far ray asks.
   - Either show it only splits more and stops on the same block, or key the
     bound by the zoom asked. Neither is assumed.
4. **Filling at arrival against 20261012.** 20261012 says that "selecting a
   chunk does not authorize materializing its unobserved interior", and that
   cached results "must not change the final result".
   - The table stores the query's answers, not geometry. Whether that makes a
     square's unobserved nodes allowed to be stored is not settled here.
   - Check 1's count says how much of the table no ray observes.
   - If filling at arrival is ruled out, the table is filled within the frame
     from the rays' misses, each ray resuming from its stop (above).

## What the checks found

**1 and 4. Filling at arrival is ruled out, by the ground alone.**

- Under its face the ground is solid down to its bedrock, at least
  `BEDROCK_DEPTH` (64) leaf cells under `min(face, SEA_LEVEL)`
  (`ground::bedrock`), by depth (`ground::by_depth`).
- At its zoom each of those blocks is a block, and its parent is a split, so
  each one differs from its parent and is an entry. A child mask does not remove
  them: they are not empty.
- Count only the shells where each zoom is asked: zoom c from 96 to 192 of its
  cells from the eye (`grid::SPLIT`, `window::RADIUS`). Take flat ground at sea
  level and the E120 eye, 120 units from its focus at a polar of 0.9, so 74.6
  units up. That is still 11,765,311 blocks under the face:

  | Zoom | Blocks under the face |
  | ---- | --------------------- |
  | 24   | 6.29 M                |
  | 23   | 2.78 M                |
  | 22   | 1.39 M                |
  | 21   | 0.69 M                |
  | 20   | 0.35 M                |
  | 19   | 0.17 M                |
  | 18   | 0.09 M                |

- A ray from above stops at the face. Only a cave's opening or a cut face shows
  one of these blocks, and a frame has 8,129,352 rays in all.
- So filling at arrival stores over eleven million nodes that no ray observes.
  That is the unobserved interior 20261012 does not authorize.
- The caves, the ways and what is placed were not counted, since the ground's
  count already decides it.

**2. One record holds every answer.**

- `world::node` answers an `In<[u32; 2]>`: a `Kind` (`Split`, `Cell` or
  `Lacks`) and two words.
  - For a cell, the words are its asset and its four looks.
  - For a split, they are its slab word.

  > Since 2026-10-27: its box in its own steps on every axis (`answer::box_word`).
- Its `block` flag is the node's zoom against the zoom asked, which the ray has,
  so it is not stored.
- Each piece of state it threads follows from the node's line alone:
  - `Cave` is `Cave::child` from the parent's, and `Cave::of` gives the same
    from nothing.
  - `Shard` is `Shard::at`, the node's own.
  - `Found`, the ways' trail and what is placed (`overwrite::Held`) are where a
    lookup is.
- So the answer is a function of the node and the zooms asked. A record of the
  kind and two words holds it.

**3. The zooms asked, as the walk forms them.**

- `asking` is `[detail.at(t1²·|d|²), detail.at(|near|²)]` (`cast::walk`):
  - `near` is the asked zoom at the node's nearest point to the eye, a function
    of the node and the eye.
  - `far` is the asked zoom at the ray's exit from the node, which differs ray
    by ray.
- So no stored answer is exact for every ray through the node.
- An eye-independent bound would read every zoom under the node, finer than a
  far ray asks. 20261013 forbids that, and filling at arrival would need it.
- Filled within the frame, the entry can be keyed by the node and the eye's
  pair: `near` as now, and `far` at the node's farthest point from the eye.
  - That range holds every ray's range and adds only coarser zooms.
  - 20261013 allows that ("more cautiously, which only splits more").
  - No zoom finer than asked is read, and the ray stops on the same block.

**So** the structure taken is the one this record names for that case: one
table, filled within the frame from the rays' misses, each ray resuming from its
stop. Each entry is keyed by the node and its asked pair. Nothing is built yet:
what filling within the frame costs (how many passes a frame, how many misses)
is not counted here.

## Filling within the frame, counted

The E120 eye's rays were walked on the CPU over the r12 tiles
(`out/terrain.publish`), through the cast's own walk: `cast::start`, then
`cast::first_from` a ray, and `world::node`. The view was dpr 2, 3652 × 2226 =
8,129,352 rays. Each visit's key was recorded: the node, the `near` the walk
asks, and `far` at the node's farthest point from the eye. Neither the log nor
the probe was kept.

> Since renamed: `cast::start` and `cast::first_from` are gone; a walk from the
> world's root is `cast::first` (pxl-spec-core cast.rs, over `cast::walk`).

- **The walk matches the GPU's.** A ray visits 18.00 nodes on average, where the
  GPU's cast decodes 18.0 at E120 (the same walk log).
- **The table is small.** A frame's 146,351,342 visits read 68,677 distinct
  keys, so each key is visited about 2,100 times a frame.
  - The camera grid's cell spans about twelve drawn pixels (`target_cell_px`),
    so the rays through a node are many.

    > **Note (10-28):** `target_cell_px` is gone. It was the τ 1 cell,
    > 1/192 rad, in drawn pixels at ratio 2. The view's cell is 1/96 rad
    > (pxl-spec-core grid.rs `CELLS_A_RADIAN`), an angle, never pixels: on
    > a view 1080 CSS pixels high at 50°, about 24 drawn pixels at ratio 2.
  - The world is composed 68,677 times a frame, not 146 million.
- **A frame after a drag.** The eye moved 1.21 units (12 CSS px at the focus)
  and the table was the one the frame before left:
  - 145,391,503 of 146,524,411 visits (99.2%) read a key already there.
  - 996 keys were filled, in 6 fill passes.
  - 807,662 rays stopped at a miss in the first pass, 202,964 in the second,
    35,438 in the third, then 3,068, 674 and 117.
- **A frame from nothing** (the first, or after a jump) takes 21 fill passes. In
  the first nine every ray stops, one level of the shared nodes each pass.
- **Not counted here:**
  - nothing placed (an empty store);
  - the service area left undeclared;
  - 6 of the 42 tiles wanted absent from disk;
  - what a lean visit costs on the GPU;
  - when the table drops keys no frame reads any more.

## What disappears when it is built

_As built (Status), only the reading walk is free of these (pxl-shader
observed.rs `Reading`); the walks that write the pyramid still compose a node
its entry does not hold, from what was handed it, and keep that in the entry
(`Writing`, `world::At::words`)._

_Note (2026-10-27): no walk only reads the pyramid any more. The pixel pass
walks nothing, taking the raster's pick or the sky and listing the rest for
the walk again (pxl-shader cast.rs `cast_cs`), so the reading walk and its
`Reading` are gone; every walk writes it (`Writing`)._

- Every source's composition inside the cast: placed, ground, zoom loop,
  fallback, `held_of`, caves, shard, the ways' cursor and the slab join.
- The `At` a walk threads to feed that composition, and the stack of it.
- The per-visit branch bodies the SIMD group runs whole.

What stays: one ray a pixel, log n descent, the image as a function of the world
and the eye, and one function for the rule.
