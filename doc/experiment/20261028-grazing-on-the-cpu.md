# Grazing on the CPU

**2026-10-28.** E1 of the grazing rows
([what the grid cannot tell apart stands coarse](../minutes/20261028-what-the-grid-cannot-tell-apart-stands-coarse.md),
[a ray is read against the light that arrives](../minutes/20261028-a-ray-is-read-against-the-light-that-arrives.md)).
Grazing rays over synthetic ground, cast by pxl-spec-core's own walk
(`cast::first`) with today's prune and with no cone. Beside them, a search for
the first cell a ray's widened cone touches, which is not in the tree. One
question is whether today's prune treats a surface on a node's face otherwise
than one inside it. Another is what a touch rule costs and how far it stops
from the exact hit. The last is whether boxes grown by `g = r + ρ` leave a
pixel uncovered. Nothing here is on the GPU or the screen, and nothing is
timed.

Why it was run: rows of coarse cells went empty at grazing angles, and the
reading had narrowed to the eye's prune. If its sides shrink only inside a
node, ground on a node's face and ground inside the next would stop at
different heights, which would step at node boundaries. A deterministic count
could say so before anything was seen on a screen. What it found is read
together with the screen in
[the rows, seen with the eye's prune off](./20261028-the-rows-seen.md).

## How

A throwaway example built from an export of `458155604`, not committed
(`rust/target/session/01896805-…/e1/rust/spec/core/examples/grazing_probe.rs`
in pixelet-d3's session target).

**The ground.** A y-invariant heightfield implements `cast::Query`:
- It is solid under `h(x)` units, with columns a leaf cell wide.
- A node wholly over it is a cell of air.
- A node wholly under it is a split with `BOX_WHOLE`, or a cell of stone at
  the zoom asked.
- A node it crosses is a split whose box runs from its floor to the highest
  `h` over it.
- At the zoom asked (`zoom ≥ asked[1]`, as `world::node` has it), a crossed
  node is a cell of stone keeping the eighths its x–z section fills
  (`answer::eighths`, `kept_word`).

**The eye** is the shader's `Detail` (pxl-shader eye.rs `eye_origin`):
- τ 2 (`Tau::VIEW`);
- cone `beam::spacing(2)` = 0.010417 rad;
- range [13, 24].

The eye stands 20 units over the ground at x = 100.5, y = 0.5. For each scene,
1500 rays in the x–z plane run from 26.6° down to just under the ground's own
slope.

**Four runs**, all counting visits as `Query::node` calls:
- **Today's prune:** `cast::first` with the eye's cone.
- **Prune off:** `cast::first` with `Cone::EXACT`. Every offset below is
  against this run.
- **Dilate, crossed only:** a front-to-back search, not the GPU's walk. Each
  box is grown by `r = ½ · cone · near distance` on every side and tested over
  the ray's span in its node. The search descends into the children the ray
  crosses and stops at the first non-transparent cell whose grown box, kept
  box or cube, the ray enters.
- **Dilate, all children:** the same, but every box is tested over the whole
  ray, and a touched split's eight children are all tried, each grown by its
  own `r` (faaf39's fix).

**Coverage.** Samples a quarter cone apart in elevation run the all-children
touch, and each stopped cell's box is grown by `g`. Pixels a twelfth of a cone
apart are counted when their exact ray meets the ground and enters no grown
box. ρ is the 2D lattice's covering radius, (cone / 4) / √2, which is
conservative in one dimension.

## Numbers

Visits are a ray's count. `dz` is the stop's height less the exact stop's, in
units. `dt` is the stop's distance less the exact one's, in cones at the exact
distance.

| Scene | Run | Visits mean / p95 / max | Rays differing | dz p5 / p50 / p95 | dt p5 / p50 / p95 |
| --- | --- | --- | --- | --- | --- |
| flat on a face (z = 1024) | today's prune | 25.5 / 26 / 152 | 21 | 0 / 0 / 0 | 0 / 0 / 0 |
| | dilate, crossed | 25.2 / 26 / 29 | 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| | dilate, all | 25.2 / 26 / 26 | 1500 | 0.240 / 0.432 / 2.853 | −13.7 / −2.07 / −1.15 |
| | prune off | 25.2 / 26 / 26 | — | — | — |
| flat mid-node (z = 1365) | today's prune | 31.4 / 42 / 253 | 809 | −1.096 / −0.063 / 0 | 0 / 0.297 / 5.34 |
| | dilate, crossed | 30.1 / 37 / 44 | 664 | 0 / 0 / 8.000 | −30.7 / 0 / 0 |
| | dilate, all | 30.5 / 37 / 41 | 1497 | 0.235 / 0.412 / 8.270 | −31.8 / −1.98 / −1.13 |
| | prune off | 42.6 / 61 / 1159 | — | — | — |
| stepped, a cell every 16 | today's prune | 27.9 / 31 / 36 | 460 | −0.380 / 0 / 0 | 0 / 0 / 7.46 |
| | dilate, crossed | 29.1 / 36 / 45 | 170 | 0 / 0 / 0.063 | −3.62 / 0 / 0 |
| | dilate, all | 28.3 / 33 / 39 | 1500 | 0.038 / 0.185 / 0.450 | −7.38 / −1.56 / −0.50 |
| | prune off | 28.1 / 32 / 37 | — | — | — |
| stepped, a cell every 64 | today's prune | 28.6 / 32 / 65 | 681 | −1.293 / 0 / 0 | 0 / 0 / 10.24 |
| | dilate, crossed | 29.6 / 34 / 38 | 426 | 0 / 0 / 1.746 | −14.2 / 0 / 0 |
| | dilate, all | 29.1 / 33 / 35 | 1500 | 0.094 / 0.351 / 2.037 | −15.8 / −1.89 / −0.51 |
| | prune off | 30.1 / 35 / 111 | — | — | — |

Today's prune, split by whether the top of the cell the exact ray meets lies
on its parent's face:

| Scene | On the face, differing | Inside the parent, differing |
| --- | --- | --- |
| flat on a face | 23 / 1500 | 0 / 0 |
| flat mid-node | 0 / 86 | 809 / 1411 |
| stepped, every 16 | 125 / 871 | 335 / 629 |
| stepped, every 64 | 513 / 1241 | 168 / 259 |

Coverage, from −3° up to 0.98 of the ground's slope, pixels whose exact ray
meets the ground and enter no grown box:

| Scene | Samples | Pixels | g = r + ρ | g = r | g = ρ | g = 0 |
| --- | --- | --- | --- | --- | --- | --- |
| stepped, every 16 | 44 | 131 | 0 | 0 | 1 | 25 |
| stepped, every 64 | 26 | 78 | 0 | 0 | 8 | 27 |

## What it says

- **The face asymmetry is real in the code.** On the flats, today's prune
  moves almost no stop where the surface lies on a node's face (23 of 1500).
  Where it lies inside its parent, it moves 809 of 1411, and those stop up to
  1.1 units lower and up to 5 cones later. On the stepped fields, both kinds
  of cell differ. So the split is clean on a flat only, and whether the steps
  line up with the node grid on screen is the screen's to show (E2).
- **Dilating only what the ray crosses leaves a face unraised.** On the flat
  on a face it moves nothing (0 differ), as faaf39 read it. Testing all
  children of a touched split raises every surface alike (1500 of 1500).
- **The touch's offset is along the ray.** The all-children touch stops
  0.2–0.4 units over the exact stop at the median. Along the ray that's about
  2 cones earlier, and up to 14–32 cones at the grazing end (p5): the stop is
  where the ray first comes within `r`. On screen it is a fattening of about
  half a cone, but its depth is nearer by `r / sin θ`.
- **The cost is the prune's.** Both dilations' mean visits are within about a
  visit of today's prune in every scene, and their maxima are below it: 41–44
  against 253 on the mid-node flat. The prune off costs more there (mean 42.6,
  max 1159). The dilations are a search, not the GPU's walk, counted the same
  way.
- **The grown boxes cover.** `g = r + ρ` and `g = r` leave no pixel uncovered.
  `g = ρ` leaves 1 and 8, and exact boxes (`g = 0`) leave 25 and 27, the rows.

## Limits

- **Synthetic, y-invariant ground.** One slice at y = 0.5, rays in one plane.
  A kept box is inset on y too, so a slice can pass beside one. The mid-node
  flat's +8-unit `dz` is a coarse kept box the exact slice misses and the
  touch reaches.
- **A field, not `world::node`.** Its kept share is the x–z section's fill
  fraction, and it holds no ways, caves or anything placed.
- **The dilations aren't the walk.** They are a recursive search, front to
  back with the best stop so far bounding it. A walk built on them may visit
  in another order.
- **Coverage in one dimension.** Elevation only, with few samples (26–44).
  A column of samples covers within half their spacing. The lattice's worst
  case, ρ = spacing / √2, lies diagonally between four samples, so `g = r`
  covering here doesn't say the ρ term can go. The bound stays `g ≥ r + ρ`
  until a 2D run, the 4 × 4 lattice over a grid cell with pixels a twelfth
  of a cone apart both ways, shows otherwise (faaf39).
