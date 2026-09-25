# What a view costs is the eye's

**2026-09-25** — a design conversation, not built yet. It replaces how the
pyramid decides which level is drawn where
([level, step and zoom are one number](./20260921-level-step-and-zoom-are-one-number.md)
set the window and the rings this takes apart).

## What was found

The question put was whether, under perspective, a cell drawn at any level
can be held under an upper bound on the pixels it covers. It cannot, as the
level is chosen today.

The window (`pxl-kernel world/pyramid/window.rs`) picks its levels from the
eye: its base from the nearest ground the frustum shows, its top from the
farthest, `level_at(d) = ceil(log2(d / 192)) − 1`. Inside the window a chunk
is wanted at level k where it lies in level k's 3 × 3 page ring and not
wholly in level k − 1's (`Domain::splits`, `wanted_over`). The rings are
centered on the range anchor, and the range anchor is the map center
(`world/frame/range_anchor.rs`), not the eye.

A level-k cell, side 2^k, covers at most `P · 2^k / D` pixels at distance D
from the eye, face on; `P = H / (2 tan(fov / 2))`, 1158 at the reference
1080 px and 50°. Two cases:

- **Drawn at the window's base.** `near > 192 · 2^k`, and every ground point
  shown is at least `near` away, so `px < P / 192 = 6.03`. Bounded.
- **Drawn coarser by a ring.** The point lies outside level k − 1's ring, so
  at least a level-(k − 1) page, `256 · 2^k`, from the map center. The map
  center stands ahead of the eye, and a point that far from it toward the eye
  can be as near the eye as the frustum's lower edge. At that edge, with the
  eye `h` up, tilt `t` and half field of view `f`: `2^k ≤ (h cot t − h
  cot(t + f)) / 256` and `D = h / sin(t + f)`, so
  `px ≤ P · sin f / (256 · sin t)`.

| tilt | derived bound | simulated worst |
| ---- | ------------- | --------------- |
| 90°  | 6.03          | 6.00            |
| 30°  | 6.03          | 5.91            |
| 20°  | 6.03          | 5.95            |
| 10°  | 11.0          | 9.30            |
| 5°   | 21.9          | 19.88           |

(The simulation reproduced `window_for` and the ring rule over flat ground and
sampled the frustum; levels above 0 only.) So the bound depends on the tilt:
3.6 times the target at the lowest tilt the camera allows, and without a
bound as the tilt goes to zero. Besides that, `level_at` does not scale with
the viewport, so the pixels grow with its height, and the ground under the
pivot stands in for all the ground, so a hill nearer the eye breaks even the
base's bound.

## What is required

- **The cost of perspective does not depend on where the observer is.** The
  center is the camera — the eye — and never the map center, which is only a
  handle for moving the camera.
- **Tilt plays no part.** A level is a function of the distance from the eye,
  a sphere around it.
- **Three dimensions, one rule.** 2.5D is not a basis: terrain, places and
  objects are selected by the same rule in 3D.
- **No floor at z16.** An object's sub-cell tree has levels below the
  cell, −1, −2, −3; the rule reaches them as it reaches every other level.
- **One formula, a step function, the same at every level.** 192, 384, the
  window's `− 1`, the decor reach that pins the base to 0 and the five
  levels of the window are each a special case to be removed.
- **The formula is the budget.** Within one device, what a view costs is
  held between a lower and an upper bound by the structure, wherever the
  camera is and however it is turned.
- **Hyperparameters are measured, not designed.** The target pixels per cell,
  whether the work is equal across devices, and any hysteresis at a level's
  edge are numbers to be measured later; the structure is what is decided
  now.

## The rule

Every node of every layer — a terrain or POI chunk, an object's chunk or
sub-cell block — has a 3D bounding box: its XY and its height range. Let
`D_min` be the nearest 3D distance from the eye to that box, and

```text
r_L = (P / τ) · 2^(s + L)
```

the distance at which a cell of side `2^(s + L)` covers τ pixels; `s` is the
lattice's z16 cell as a shift (the ground's CELL_SHAMT, the places' two
more), `L` any integer.

**A level-L node is split into its children while `D_min < r_L`.** This is
Cesium's screen-space error, with the eye as the one origin. Splitting stops
only where the data does: terrain has nothing under z16, an object has
its tiers down to −3. Where the data stops above the formula's level, it is
the data drawn exactly, larger than τ, and not an approximation.

## The bounds

A node selected at level L was not split, so `D_min ≥ r_L`, and its parent
was, so the parent's `D_min < r_{L+1}`. With a node `c` cells on a side:

1. **A cell's pixels.** Every point of the node is at least `r_L` from the
   eye, so `px ≤ τ`. Every point lies in the parent's box, less than
   `r_{L+1}` plus the parent's diagonal away, so
   `px > (τ / 2) / (1 + √3 · c / (P / τ))`. Neither depends on L, on where
   the eye is or on the tilt.
2. **Nodes a level.** Every selected level-L node lies in the ball of radius
   `r_{L+1}` plus a diagonal around the eye, so there are at most
   `(4/3) π (2P / (τ c) + √3)³` of them, a constant; a 2.5D layer's lie on the
   ground inside that ball's disc, at most `π (2P / (τ c) + √2)²`.
3. **Levels.** From the finest the data has — the ball under it is one level
   at the same cost — to the pyramid's top. A fixed count, so the whole is a
   fixed count times a constant, the same wherever the observer stands.
4. **Perspective.** What the frustum takes of each level is the solid angle
   its field of view and aspect make, Ω / 4π of the above: a constant again.
   Turning the camera changes which cells, not how many.

These constants are the budget. The records a layer holds, `.fixed`, and the
chunk table's size are derived from them rather than set, as the window's
five levels and the held ring are today.

The lower bound depends on the node's side `c`: the larger a node, the more
its own depth spreads the distances inside it. At the 64-cell chunk it is
about τ / 3; a smaller node tightens it toward τ / 2 at the cost of more
nodes. That is a constant of the structure too, to be kept at the chunk and
revisited if measured.

## What changes

- **The window and the rings go.** `level_at`, `window_for`, `ground_reach`
  and `Domain`'s rings are replaced by the rule above, from the camera's
  position; the range anchor is no longer the map center.
- **A 2.5D page's height range.** A node's box needs its z extent to
  measure `D_min`. A level's chunks are drawn from that level's own
  samples, so a resident page's range — its samples', widened by the
  Catmull-Rom overshoot, at most 9/32 of their span either way — is the
  drawn geometry's and exact. A page not yet resident takes its nearest
  resident ancestor's, which is an estimate and not a bound: a parent's
  samples average its children's, and an average does not reach its
  extremes. Where nothing over it is held the ground under the eye stands
  in. A bound before arrival would need the bake to write each page's range
  of the leaves under it.
- **The chunk ladder is the same rule.** `pxl-render look/lod.rs`'s
  `Ladder` folds into it; the scheduler's pressure, if kept, can only move τ.
- **Places take the rule by their cell's side,** in place of `wanted_over`.
- **Objects' sub-cell tiers are levels −1 to −3** of the same rule.

## Where this stands

Designed and first built on 2026-09-25, for the pyramid's layers — terrain
and places:

- `world/pyramid/window.rs` chooses levels by the rule from the eye
  (`Domain::splits`, `split_distance`); the rings around the map center, the
  range anchor and the tilt-driven window are gone. `LEVEL_CHUNKS` is
  derived from the rule and sizes the chunk table and the mesh arena; the
  held ring stays 5 × 5, now as the bound on how far a level's chunks
  reach.
- A chunk's box is its page's relief (`layer/terrain/relief.rs`) with the
  placed objects' columns over it (`layer/object/store.rs`
  `column_boxes`).
- The window's base is the level the ground under the eye wants, from the
  highest point known there; a hill nearer the eye than that ground is
  drawn no finer than the base.
- Turning the camera moves the eye around the map center in this orbit
  model, so a spin changes what is near and what is held.

- Every ancestor of the window's top pages is held up to the pyramid's top,
  at most 2 × 2 a level (`CHAIN_PAGES`), and the cover stands the nearest
  resident one in for a root not drawn whole, in place of everything under
  it, so a zoom or a pan draws coarser ground rather than a hole while the
  window's own pages come.

- A chunk's tier is the rule's step (`wanted_step` in world/chunk/table.rs)
  in place of the table's ladder; the byte budget's pressure only moves the
  criterion.

- Ground the frustum does not see is not refined (`Domain::splits`), and
  pages none of whose ground is drawn at their level are not asked for, as
  Cesium and Mapbox select by the frustum. The top level's roots and the
  chain stay all around the eye, so a turn finds coarse ground there at
  once; what was held of the far side keeps its record until another page
  wants it. The frustum is widened by the planet's curvature, which draws
  far ground lower than it lies.

Not yet:

- Objects' sub-cell tiers are not levels of the rule. Built later the same
  day: the rule reaches below z16 and a z16 chunk folds its objects to the
  cube its distance wants.
- Decoration is scattered from the leaves alone, so it ends where the rule
  stops drawing them (level 1's split distance), nearer than a kind's own
  reach. Built later the same day: a chunk scatters from its own level's
  page, each level cell's candidate that of a z16 cell under it, so a
  zoom keeps the instances it had and thins them a quarter a level.
- A tier changes at its threshold with no crossfade; the machinery for one
  (a secondary tier and its weight) is kept but unused.
- The window is WINDOW_LEVELS deep, so from low over the ground the far
  ground ends at the coarsest level's reach rather than at the horizon.

τ, whether the work is equalised across devices, and hysteresis are left to
measurement.

## What the rule is and what a layer is

Settled later the same day, and it replaces "places take the rule by their
cell's side" above. The
rule is shared: it chooses a level — a zoom — for each piece of ground by its
distance from the eye, `r_L = (P / τ) · 2^L`, and nothing about a layer
enters it. What a layer draws at the level it is given, and in how much
detail, is the layer's own: a tuning inside the perspective, not a second
rule. At level L terrain draws its cells averaged 2 × 2 (detail +1), places
count in a lattice sixteen times the ground's cell (+4) and stand a lone place
on its z16 cell, and objects draw their own cells (0): an object's cube and
the ground's are one size wherever both have data, never fine objects on
coarse ground. The rule's levels go below z16 — z17 to z19 near the eye —
and where it asks finer than a layer's data the layer draws its finest:
terrain z16, objects their sub-cell tiers down to z19. z16 is terrain's atom
and nothing more. What this leaves open is in
[the backlog](./BACKLOG.md), under this rule's entry.

## The window goes

Settled and built later the same day. The window was the one piece of the
old selection left standing, and both of its ends cut the rule short: its
base was the level the ground under the eye wanted, so a hill beside an eye
floating over a valley — 300 units off, the valley 2,000 under — was held at
the valley's level 3 where its distance wanted z16, eight times the pixels a
cell is allowed; and its top was five levels over that, so from low over the
ground the far ground ended at the coarsest level's reach, short of the
horizon. Geometry clipmaps (Losasso and Hoppe) take the first as the price
of holding levels by the height under the viewer; CDLOD and Cesium have no
window at all and select every node by its own box.

The window's one job was the budget: five levels of 5 × 5 records, and five
groups of the mesh arena. What replaces it is the budget itself
(`world/pyramid/domain.rs`, which `window.rs` became):

- The roots are the pyramid's top, and the rule splits from there down to
  terrain's atom, so a hill is split by its own distance and the ground
  runs to the air's reach, or the top level's.
- A view wants the pages its drawn chunks lie in and every ancestor of
  those up to the top — the chain's job, now for all of it — and nothing
  else.
- Before anything is asked for, the chunks drawn and the pages wanted are
  counted. Where they pass `VIEW_CHUNKS` (1,024, a quarter of the chunk
  table, since a stand-in draws four for one) or `VIEW_PAGES` (165, the
  records the window had, so `.fixed` did not move), every split distance
  is shortened alike, a quarter of an octave a step, until they fit. A view
  over budget is coarser everywhere, never missing a piece; the fit is
  decided from the eye at once, not fed back from refusals.
- The arena's coarse groups are derived from the chunks a view draws and
  half again for what a change holds on to, which is the five it had.

Over flat ground the budget does not bind at any height: all around the
eye, with no frustum, a view draws at most 992 chunks from 41 pages; a view
looking one way, 250 to 450 chunks from under 30. The pages are a quarter of
what `.fixed` holds for them, which is room to be measured before it is
given back.

A page's height range before it arrives stays its nearest resident
ancestor's, an estimate: a bound would need the bake to write each page's
range of the leaves under it. Cesium, whose quantized-mesh carries a tile's
own range, knows a child's no better before the child comes. A peak the
estimate misses is split a fetch late and corrected when its page arrives,
so what is drawn once the pages are held is exact and the bake is left as
it is.

## Drawn in place, and ordered from the center

2026-09-26. With the window gone, a view fetched its pages coarsest first:
every ancestor of the page under the eye was a hole at distance nought, so
the first six connections went to z6 through z11, and the top page's
two-kilometer cubes stood as walls in front of the eye for seconds while the
eye's own page waited (measured on a cold origin: the eye's z16 page asked
for at 3.5 s, landed at 5.9 s). Fetched finest first instead, the fine pages
sat unused: a sibling group was drawn whole or not at all, so a resident top
page drew the whole view as one chunk until the last page under it landed —
new ledger lanes read one chunk drawn and none open for 4.3 s after a jump,
with some 260 wanted and twenty pages resident.

Two changes, decided with the user and reviewed by the other sessions:

- Every chunk the rule wants is drawn in its own place, from its page or the
  nearest resident page over it, upsampled: that page's cells for its
  color, its surface at the chunk's own cells for its height (Cesium draws
  a parent tile's data for a child not yet loaded). The whole-group rule,
  stand-ins from under, their depth limit and the chunk table's fourfold
  room for them went. A chunk borrowing a page scatters no decor, and its
  own page landing rebuilds it through the ordinary sources compare.
- One order for every layer's work, out from the view's center — the point
  the camera turns about, where the character stands: within a layer the
  page the ground nearest the center is drawn from, then the top pages,
  then every page drawn from nearest the center first, then the fallbacks;
  the layers terrain, then the decor's models, then the places; the chunk
  table builds in the same order.

After a jump to a cold coast the cover drew every wanted chunk from the
first frame and left nothing open, and the ground was its own from two
seconds on. A seam where a borrowed chunk meets one built from its own page
is left in [the backlog](./BACKLOG.md).
