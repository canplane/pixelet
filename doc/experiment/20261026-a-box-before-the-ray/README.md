# A box before the ray

**2026-10-26.** The conservative box a rasterized proxy would draw, defined and
held against the cast on the CPU, for
[a proxy only starts the ray](../../minutes/20261024-a-proxy-only-starts-the-ray.md).
No raster pass is built. The rule is pxl-spec-core `proxy.rs`, and the test is
pxl-kernel `domain/query.test.rs`
`a_proxy_s_box_is_entered_no_later_than_the_cast_stops`.

## The rule

- **Which nodes.** One cut through the pyramid, the camera grid's. A node is
  drawn where its zoom is at least `Detail::at` of its nearest point less
  `coarser`, held to the range's coarsest (`proxy::cut`). This is the rule by
  which the cast makes a node a block, `coarser` zooms coarser. With
  `coarser` ≥ 0, every block the cast stops at lies in one cut node.
- **What each is asked.** The world's one query (`world::node`) with nothing
  handed down (`world::At::of`), at the zooms the cast asks that node at
  (`proxy::asked`, the cast's own `asked`, a function of the node and the eye
  alone). It reads only what is already held: the windows' levels and folds,
  the objects' tree, the ways' pool and the caves' hash. The GPU's glue is the
  call `World::block` already makes.
- **The box** (`proxy::of`):
  - a split is the node clipped along z to its slab: the slab the cast's
    contour test passes a ray by, the ground's (`ground::slab`, every asked
    zoom's top rounded up), the ways' and what is placed, joined;
  - a block that fills, even one coarser than the cut, is its box as drawn
    (`cast::kept_bounds`);
  - air, or a slab that misses the node, is no box;
  - a lack is the node whole; a node not held coarser than a block is a split
    over all z, so it is the node whole too.
- **Why it holds.** A cut node or coarser holds every block the cast stops at
  under it, each asked at a zoom within the node's own asked range. The slab
  bounds every block at those zooms: the invariant the cast's own contour test
  already rests on. So a stop's entry lies on a box, and the ray enters that
  box no later than it stops.
- **Fallbacks.**
  - An eye inside a box starts there, at t = 0. Under the ground with no cave
    hollowing the whole node, a slab runs to the world's floor, so an eye under
    the ground is always inside one.
  - A ray that enters no box meets nothing (`Ends::On`), and the test holds
    that.
  - Squares not held, or held absent and read off a coarser window
    (`window::step`), are the query's answer as the cast gets it. That answer
    is a box over the node's whole height, or a lack box.
  - Caves only take away, so they only lower a slab.
- **Decor needs no box.** The GPU's cast does not strike it (pxl-shader
  world.rs `World`): decor is drawn in draws of its own. If a cast ever
  struck it, the margin would be the decor table's reach, 5 leaf cells over
  the face and 3 sideways (scatter.rs `CLAIM_REACH`).

## How

The kernel's test windows (`hold`), the real fill, over one leaf tile and every
tile above it. Each zoom's heights are a relief's mean over its cell, so the
coarser cells are the finer ones folded. A bridge and a tunnel lie over the
leaf, with a six-cube tower and a half cube by the eye.

- **Grounds:**
  - flat: 5;
  - rolling: hills 8 high and 96 apart, on a 0.05 ramp;
  - steep: hills 40 high and 48 apart, on a 0.3 ramp;
  - steep again, held only up to z12's tiles, so its far rays lack.
- **Eyes:** 2, 20 and 200 units over the face at the tile's middle.
- **Rays:** 200 a case, elevations −74° to +3°, reach 3000, τ 1. Each is cast
  by `cast::first` through the GPU's world (no decor). The nearest box it
  enters is found by descending the cut along it.
- **Checks:**
  - box front ≤ stop t + 10⁻⁴·t, lacks included;
  - a ray that enters no box ends `On`;
  - a walk started at the box front, from the root, as a pixel would start
    one, stops on the cast's node.

All 2400 rays hold at every `coarser`, 0 to 3, including 102 lack stops.

## Numbers

The gap is stop t less box front, in units and in the stop block's sides
(p50 / p90 / max).

| Ground, eye | `coarser` | gap, units      | gap, blocks    |
| ----------- | --------- | --------------- | -------------- |
| flat +2     | 2         | 0 / 0 / 1181    | 0 / 0 / 74     |
| flat +20    | 2         | 0 / 0 / 43      | 0 / 0 / 5.4    |
| flat +200   | 2         | 0.97 / 4.9 / 61 | 0.36 / 1.5 / 3.8 |
| rolling +20 | 2         | 0 / 4.1 / 103   | 0 / 2.6 / 52   |
| rolling +200 | 2        | 2.2 / 23 / 152  | 0.95 / 3.0 / 9.5 |
| steep +2    | 2         | 0 / 1.2 / 39    | 0 / 1.2 / 39   |
| steep +20   | 2         | 0 / 12 / 910    | 0 / 12 / 114   |
| steep +200  | 1         | 2.6 / 37 / 106  | 1.1 / 7.1 / 27 |
| steep +200  | 2         | 8.7 / 49 / 279  | 3.1 / 17 / 31  |
| steep +200  | 3         | 27 / 81 / 502   | 9.1 / 24 / 37  |

The test prints every case and every `coarser` (`cargo test-host -- a_proxy
--nocapture`). At `coarser` 0 the gap is 0, under 1.6 units on steep ground.

Visits a ray asks the query, mean over the rays that stop:

| Eye  | From the eye | From the box front, `coarser` 2 |
| ---- | ------------ | ------------------------------- |
| flat +2 / +200 | 32 / 43 | 27 / 25 |
| steep +2 / +200 | 42 / 54 | 27 / 32 |

The boxes the cut holds within 3000 units of the +20 eye, all directions,
counted whole and as tops (a box whose top is its slab's, about one a
column). The test counts `coarser` 3 only; the other rows come from the
same count widened to them.

| Ground        | `coarser` 1    | 2              | 3            |
| ------------- | -------------- | -------------- | ------------ |
| flat          | 9.2 M / 88 k   | 1.31 M / 33 k  | 186 k / 10 k |
| rolling       | 8.9 M / 55 k   | 1.23 M / 23 k  | 172 k / 7.8 k |
| steep         | 7.9 M / 53 k   | 1.08 M / 22 k  | 156 k / 7.3 k |
| steep, z12 up | 12.2 M / 27 k  | 1.64 M / 12 k  | 232 k / 4.2 k |

## What it showed

- **The rule is conservative with no new state.** It is the cast's own
  contour test, asked once a cut node instead of once a ray.
- **The boxes are tight where they matter.** Over flat ground the median ray
  starts on its stop. Looseness grows with height and slope: from 200 units
  over the steep ground, at `coarser` 2, a ray starts a median 3 blocks short
  of its stop, 17 at p90. The large maxima are grazing rays, which run along a
  box's top over lower ground.
- **A start from the root pays most of the walk again.** From the box front a
  ray still asks 25 to 38 nodes against 32 to 54 from the eye: the descent
  from the root to the start's depth is about 25 of them. The proxy pays only
  if the pixel resumes at its box's node, not at the root. That needs what
  each node over the box was handed (`cast::Over`), which `world::At::of` can
  give anew at each zoom without state, but no code does that yet.
- **The count is the buried stacks.** Under the ground the slab runs to the
  world's floor, so every column stacks boxes down to it. Tops are 1 to 5 % of
  the boxes. A bottom clip at the lowest face of a node and its eight
  neighbours (nine reads of the windows' levels, still no state) would drop
  the buried ones. It is conservative only for an eye over the ground: an eye
  in a cave reaches buried nodes without crossing a box, so it would fall back
  to starting at the eye. That clip is not built or proved here.

## Since: the floor, the GPU pass, the raster

The same day, in 41bc88c37 through 237be9197; measured on the CPU, as above,
except where the page is named.

- **The floor.** A box is cut off a block under the lowest face of its node
  and the eight about it (`proxy::floor`), read at the zooms the cast may draw
  a block at there (to the camera grid's zoom at the nearest point, or the
  node's own under it), off the windows the cast reads that ground from. A
  ray from an eye in the open (`proxy::aired`) reaches ground under it only
  through the ground's surface, which lies in a box; an eye not in the open
  has no proxy. Every cut node is a split, never a block (`proxy::cut`), so
  that surface is boxed even where it is carved. Ground no window holds at
  all is a lack at every height, entered only from the side: a node walled in
  by it is no box (`UNHELD`); past the service area the Earth faces at sea
  level. Read up to the finest zoom instead, a far node's floor fell to the
  world's bottom off windows holding its ground in part, and the box count
  swung 6× from one load to the next on the page.
- **Box counts at `coarser` 3** within 3000 units of the +20 eye: flat 17.2 k,
  rolling 18.4 k, steep 42.3 k, held only to z12 45 k (177 k before the
  unheld rule). At `coarser` 2: 60.5 k, 67.2 k, 178 k, 187 k.
- **The GPU pass** (ray_proxy, a dev setting; spans `proxy`, `raster`):
  seeded at zoom 8 over the eye's coarsest window, past which the cast lacks
  and a ray starts where it leaves it; a round a dispatch; boxes grown by two
  drawn pixels and drawn at every pixel, nearest front kept, reversed depth
  drawn 2⁻¹² nearer, back faces culled (`proxy::corner`, its winding held by
  a test). Descending three zooms a round asked three times the nodes and was
  slower (122 k nodes, 15.6 ms, against 41.8 k and 10.4): one a round stands.
  On the CPU the floor is about 70 % of a node's cost.
- **Its pipelines** are made one a turn where the stream has room: a turn
  whose modules pass its stream's room is lost whole.

## Tried: boxes off the window levels alone

Columns taken straight off the windows' level entries (`proxy::column`), the
objects' tree boxed apart, no `world::node`: conservative on every ray, but
too loose to start rays from. At `coarser` 2, within 3000 units of the +20
eye (gap p50 / p90, units):

| Ground        | Cut boxes | Column boxes | Cut gap   | Column gap |
| ------------- | --------- | ------------ | --------- | ---------- |
| flat          | 60.5 k    | 72.8 k       | 0 / 4.4   | 16 / 74    |
| rolling       | 67.2 k    | 72.8 k       | 0 / 4.2   | 19 / 75    |
| steep         | 178 k     | 72.9 k       | 1.3 / 34  | 5.3 / 57   |
| z12 up        | 187 k     | 54.2 k       | 1.0 / 44  | 90 / 116   |

Where they are looser than the cut, and why:

1. Nothing prunes air: every held entry is a column from its floor to its
   faces' top, where the cut drops a node its slab misses.
2. Each of the two or three windows the cast reads at a distance boxes its
   own column, as a window's faces bound its own reads alone.
3. Where the cast lacks, the entry's own ground at the coarsest zoom asked
   held nowhere or a neighbour held nowhere, the column stands to the world's
   top, at every height.
4. A neighbour read off another window leaves no floor known: the column
   runs to the world's bottom (one box still, not a stack).
5. What is placed is not in the levels: boxed off a descent of the objects'
   tree, so not wholly without a descent.
6. Earth past the service area is taken at sea level for ground no window
   holds; not tried, the fixture has no area's edge.

Caves, digs and tunnels hold as in the cut, as they only carve. Fewer boxes
on steep or partly held ground do not buy back a start 4 to 20 times
farther from the stop, which a walk pays for: the cut stays the producer.

## Where the code is

Not on main, whose code is as before the experiment. Every commit named here,
and the tree with the rule, the pass and the raster built, is on the branch
`experiment/20261026-proxy`.
