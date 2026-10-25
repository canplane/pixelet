# The view's cost, read against its invariant

**2026-09-28** — an audit by reading, not measured, and one decision. It holds
the tree as it stands against the invariant
[what a view costs is the eye's](./20260925-what-a-view-costs-is-the-eye-s.md)
set down, and finds that the rule keeps it and the budget does not.

**Superseded in part** the same day by
[what is drawn is what the screen covers](../20260928-what-is-drawn-is-what-the-screen-covers.md):
its audit stands, and its decision — a column's cubes bounded by k — is replaced
by bounding what is drawn by the screen.

## The invariant

That document derived what a view may cost from its rule alone. A node at z is
split while its nearest 3D distance from the eye is under
`r_z = (P / τ) · 2^(16 − z)`. From that:

1. **Nodes a zoom are a constant.** Every selected node at z lies in the ball of
   radius `r_{z−1}` plus a diagonal around the eye, so a zoom holds at most a
   fixed count — the same for every zoom, the way the flux through every shell
   around a charge is the same.
2. **Zooms are a fixed count**, the ball under the finest zoom the data has
   counted as one zoom at the same cost.
3. **The frustum takes a constant share** of each zoom, Ω / 4π.
4. **These constants are the budget.** The records, `.fixed`, the chunk table
   and the arena are derived from them, not set.
5. **Fitting is decided from the eye at once**, never fed back from a refusal; a
   pressure, if one is kept, moves only τ.

## What holds

The rule is the invariant's. `Domain::split_at` splits by `D_min` against
`split_distance(z)` from the eye alone, with no tilt and no floor at z16; one
`Order` ranks fetch, mount, build and the record given up; the pages wanted are
the drawn chunks' and every ancestor to the top.

A resident byte of the ground is bounded where it lies. A page is one record of
`.fixed.terrain`, the same size at every zoom. A chunk's mesh is one slot of its
pool, and a mesh past the slot is baked a step coarser (`BuildEvent::Coarser`),
so a slot's worst case does not depend on what the ground holds. The run sets,
the bake, the surface lanes and the decor's gathering are `.fixed`.

Over flat ground those two together give the invariant. A zoom's shell lies 3 to
6 of its own chunks from the eye at every zoom, so every shell holds the same
count of chunks, each one slot of the same size, and 12 to 16 pages.

## What does not

| Invariant                    | The tree                                                                                                                                                                                                                                               |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1. a bound a zoom            | Only z16 is held to one (its own chunk count); every other zoom only to the sum (`VIEW_CHUNKS`), so one shell can take another's share.                                                                                                                |
| 4. derived, not set          | `MAX_CHUNKS` (4,096) and `VIEW_PAGES` (48) are set. 48 is under what the rule's zooms can want — 12 to 16 pages over eleven zooms, by domain.rs's own comment — so the view is fitted to it by search every frame the eye moves.                       |
| 4. the chunk table           | `VIEW_CHUNKS` is a quarter of the table because a stand-in drew four for one; the stand-ins went on 2026-09-26, and the table needs the view and the one it replaces (`WIDEST_SELECTION`), twice.                                                      |
| 4. decor                     | Outside the budget: heap blocks under `RETAIN_BUDGET_BYTES`, half the heap. A block's bound (`DECOR_BYTES`) and the chunks' count are both there to derive it from.                                                                                    |
| 5. no feedback               | `pressure_step` moves the criterion on the heap's refusals. It exists because decor, the volumes and the models are in the heap.                                                                                                                       |
| 2. the ball at the same cost | The count is the same, but a z16 chunk holds its own tier and the cube above (`Scheduler::ask`), 96 + 71 KiB against a coarse chunk's 71 — a cache the invariant does not ask for, where two tiers are needed only while one dissolves into the other. |
| the search itself            | `domain_for` tries every step from the finest (`(0..MAX_STEP).find`), walks each to the end past the budget, and keeps every node it measured in `HashMap`s: unbounded in time and heap, and heaviest on the steps it throws away.                     |

Besides the budget, three things are not the eye's at all: the geocode fetch is
centered on the map center (layer/geocode/fetcher.rs), the raycast keeps its
composites by the tick they were last used, and the object store is resident
whole. The last is a stated exception, as the external assets' bodies are
(mem/fixed.rs).

## Where 3D breaks the bound

Invariant 1's constant for a 2.5D layer is a disc, one node a column. A chunk is
a cube, and ground steeper than a cube's floor or ceiling is two or more a
column — 1.5 near Gangnam and 1.7 on Bukhansan (2026-09-26). The 3D constant, a
ball, closes but is far past any memory. So the disc was kept as a target and
the view is held to it by loosening every split distance alike (domain.rs: "not
a bound the rule keeps by itself … the view is held to it instead"). That is the
search above, and the reason the budget is a runtime answer and not a
derivation.

Two ways were weighed:

- **Loosen by shell.** A shell past its share loosens its own criterion. The
  invariant then holds per shell, but the criterion is no longer one number for
  every zoom.
- **A column's cubes are bounded, and past the bound the ground is drawn a zoom
  coarser.** A zoom's column holds at most k cubes; where the ground under it
  would need more, its parent is not split there. It is the slot's rule one step
  up — a mesh past its slot is baked a step coarser, never cut and never
  searched for — and it keeps one criterion.

**Decided: the second.** With it every count is a derivation — a zoom's chunks
are k times its disc, the view's are the zooms' sum, and the pages follow from
the chunks — so the search, `VIEW_CHUNKS` and `VIEW_PAGES` as set numbers, and
z16's own chunk count as a special case go. What is paid is the ground past the
bound, drawn over τ where it is steepest, as a mesh past its slot already is. k
is a product number, to be said aloud.

What the derivation has to face: taken all around the eye, a zoom is about 330
chunks and 16 pages, so eleven zooms are some 3,600 chunks and 176 records
against the 1,024 and 48 set today. The frustum's share (invariant 3) is what
brings that down, and it depends on the screen's `P` and field of view, which
are the device's, known at boot and not at compile time — the backlog's "split
distance from the real screen". Whether `.fixed`'s terrain range is sized at the
widest screen the kernel accepts, or laid out at boot, is part of the same work.

## What goes with it

In order of dependence:

- **The domain.** The derived counts in place of the set ones; the column bound
  in the split; the search, its hysteresis and its `STEP` gone. Until then a
  walk that stops at the budget and a search that starts at the last step, and
  the column heights (`HEIGHTS`) forgotten a page at a time, not whole on every
  arrival.
- **The chunk table.** The builder's own table of chunks (build.rs
  `Builder::chunks`) is the scheduler's slots kept in step by id; its fields
  belong on the slot. A slot's tiers and decor bounded by construction rather
  than by `trim`, the index a table of fixed capacity, `MAX_CHUNKS` twice the
  view, and a z16 chunk one tier except in a dissolve.
- **Decor, and the pressure.** Decor in a pool derived from the view as the
  meshes are, which leaves the heap's refusals nothing to answer, and the
  pressure and `RETAIN_BUDGET_BYTES` go. Open: whether the kernel needs its copy
  of a block once it is on the device (`Decor::groups`, the hit test).
- **Two questions of representation**, not decided:
  - The domain's tree is not the hierarchy: a peak's chunk is taken under its
    column's nearest held chunk (`Domain::children_of`), while the chunk table
    finds what covers what by geometric ancestry (`Chunk::ancestors`).
  - "Seen" has two owners: the domain's `Frustum::sees` orders the work, the
    table's `measure`, with the horizon test, builds and draws it.
- **The small ones.** The geocode fetch from the eye; the raycast's composites
  given up in the view's order.

The places are left out of this record; they are off by default and were set
aside in the conversation.
