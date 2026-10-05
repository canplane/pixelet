# The surface a way runs on

**Status: aborted** on 2026-10-06, before any of it was baked. The contract
below led to a rule set (a step of at most 1u first, clear space and cover
next, a straight line between a structure's ends adjusted where it
collides) and a solver over each page's 1-ring that took 30 to 50 s a page,
125 s for nine pages at its fastest, and still left seams apart. It was dropped for
[bridges and tunnels as Arnis solves them](../20261006-bridges-and-tunnels-as-arnis-solves-them.md):
the same tags stand as Arnis stands them, solved in tens of milliseconds a
page. Nothing below is what the bake does or aims at; the files it names
are gone.

**2026-10-05** — a draft, written to be argued with. Sections 1 to 3 state
what is to be guaranteed and how a failure is judged; section 4 is open and
lists the choices with the cases that tell them apart; section 5 names no
algorithm yet; section 7 says what can be surveyed before any of it is
chosen.

A way's drawn surface is a contract with the map: it is joined where the map
joins it, crosses where the map crosses it, stands off the ground where the
map says a bridge or a tunnel, and never breaks. Heights are not in the map;
they are inferred, and an inference may yield. The contract may not.

## 1. Definitions

**What the source says.** Overture's segments give, for each way: its
geometry; its connectors, which alone decide where two ways join — sharing a
place without a connector is no join, and a join may cross levels
([Overture](https://docs.overturemaps.org/guides/transportation/segments-and-connectors/));
its flags over ranges of its length (`is_bridge`, `is_tunnel`, `is_indoor`,
`is_under_construction`, `is_abandoned`, `is_covered`); its `level` over
ranges, an order and not a height; its class. Nothing in it is a height.

**What is estimated.** The ground's height, from the DEM: an estimate of the
terrain, which says nothing of a way. A way on the ground is near it; how
near is an inference.

**What is inferred.** Every height a way takes, every clearance, cover and
grade.

**Two ways' relation**, at a place where their plans meet:

| Relation  | The source                                     |
| --------- | ---------------------------------------------- |
| joined    | they share a connector there                   |
| crossing  | their geometries intersect there, no connector |
| unrelated | neither: their plans do not meet               |

**A drawn way** is every range not flagged indoor, under construction or
abandoned, and no ferry's route. A way is drawn at the finest resolution;
a coarser zoom folds it and is out of this contract.

**The surface** of a drawn way is the cells its width covers on the ground,
on a deck or on a bore's floor, after every height is quantized.

**The step Δ** is the most two neighbours' heights may differ: a vertical
length, 1u today, a knob of its own. It is not the cell's side, nor the
quantum heights are stored in (q), nor a grade: a grade is a height over a
horizontal distance measured on the ground, in metres.

**A way's form** is the structure the way itself builds around its
surface. It says nothing of what another way builds over or under it: a
road under a bridge, or a deck under the deck above it, keeps its own form.

| Form   | What the way builds                                                  |
| ------ | -------------------------------------------------------------------- |
| ground | solid from its surface down to the terrain; nothing of its own over  |
| deck   | a slab under its surface, air under that down to what it passes over |
| bore   | clear space over its surface, a shell over that, under the terrain   |

A way raised or lowered off the DEM on the ground — an embankment, a cut —
is still the ground form: what it builds is solid under it and nothing
over it.

## 2. What is guaranteed

The contract is stated in layers — the surface exists and joins as the
source joins it; each way builds its form; the heights hold — for the
reading's sake. The order is not the checks' independence: in the final
surface every guarantee holds together, and none is ranked against another.

### 2.1 What must join, and what only lies side by side

Two kinds of pair are told apart before any height.

**A required join** is one the source makes: two consecutive ranges of one
way, where a flag starts or stops (a bridge's range and the ground range
after it, a tunnel's portal); and two ways at a connector they share.

**A plan contact** is two surface cells, of any two ranges, that share a
side in plan — on the page's grid, whatever their heights. A plan contact
is only a candidate: whether the two cells are joined is decided by their
heights and forms, in 3D.

**A realized join** is a plan contact whose two cells are passable from one
to the other: neither's form walls it off, and their heights differ by at
most Δ.

- **H0 — each drawn range is passable.** Its surface cells hold, from one
  end of the range to the other, a path of cells each in plan contact with
  the next and realized. A gap, or two cells meeting only at a corner, is a
  break. _Checked by_ a walk over each range's cells.
- **H1 — joins are where the source makes them, and only there.** At every
  required join the two paths meet in a realized join. Where two ranges are
  not required to join, none of their plan contacts is realized: a crossing
  or two ways side by side are kept apart by height or by form. _Checked
  by_ every plan contact of two ranges, on the final surface, against
  whether the source requires that join.

**A junction** is how the join at a connector is made: a surface of
width, which every range ending at the connector reaches without a break.
Its cells are one physical surface, each with one height, which need not be
level. A height within Δ is how two surfaces join; it does not make them
one. Outside the junction each way keeps its own surface: overlapping in
plan makes no further join, and two ways joined at one connector are not
joined where else they meet. Which cells the junction holds is a
construction (section 3.3), not the contract, and two junctions that touch
are one surface only where the source joins their ranges (section 3.4).

**The join relation** says it once, for every required join. A surface of
range a at cell c and a surface of range b at cell c′, the two cells one
or sharing a side, are joined when

- a and b are one range;
- a and b are consecutive ranges of one way, and the cross-section where
  one ends and the other begins, as wide as the way, passes through the
  side c and c′ share, or through c where they are one cell; or
- a and b end at one connector, and its junction holds c and c′.

What joins is derived from it alone. At one cell, surfaces joined are one
surface, and so are surfaces joined through a third at that cell. Between
two cells, surfaces joined are **neighbours**, owing Δ (H3). A plan contact
not joined is asked apart (H1).

### 2.2 Each way builds its form

- **H2 — the form is the flags'.** A bridge range builds a deck, a tunnel
  range a bore, every other drawn range the ground form. A way not flagged
  a bridge builds no air under itself; a way not flagged a tunnel builds no
  cover over itself. What other ways build over it is theirs. _Checked by_
  each range's built structure against its flags.
- **H2′ — every surface can be passed.** On the structure as finally
  composed — the terrain with every way's build — each surface cell of a
  ground, deck or bore range has a solid floor directly under it, and no
  solid over it for its kind's clear height. Heights that join are not a
  way someone can walk if its clear space is shut. Another structure in
  that space is a clash for the heights to resolve, or reported; the
  terrain is what a way's build is composed with, not a structure. The
  clear heights are the code's for now (`pxl-spec bake/deck.rs`
  `headroom_m`), not a policy for the world. _Checked by_ each surface
  cell's column as composed.

### 2.3 The heights hold

- **H3 — no step past Δ.** Neighbours differ by at most Δ, on the surface
  as it is stored and drawn — along a path, across a way's width, from one
  range of a way to the next, at every join, across every page's edge.
  _Checked by_ every neighbour pair of the baked set; a page's edge against
  the page beside.
- **H4 — the source's order is kept.** At a crossing of two different
  levels, the higher level's surface stands over the lower's by at least a
  separation g_min (open: section 4). _Checked by_ every such crossing.

### 2.4 One answer

- **H5 — the cut does not matter.** The surface does not depend on how the
  coverage is cut into pages nor on the order anything is read. _Checked
  by_ baking a region whole and in pieces and comparing.

## 3. When they cannot all hold

Some guarantees are fixed before any height and some are not.

**Fixed by the source and the drawing**, checked rather than solved: that
each range's cells hold a path in plan (the plan half of H0), that the
paths of a required join touch in plan at it, and that each range builds
its flags' form (H2). A failure is a fault of the drawing or of the data,
reported where it is.

**Decided by the heights**: H0's and H1's realized joins, H3 and H4. With
the neighbours and the ordered crossings given, the required ones are
difference constraints — h_a − h_b ≤ Δ both ways for neighbours, h_low −
h_high ≤ −g_min at an ordered crossing, and a form's bound against the
terrain where an inference fixes one. Such a system has a solution exactly
when its constraint graph has no negative cycle, and a negative cycle
proves it has none.

H1's other half — no join where the source makes none — asks that two
plan-contacting cells of unjoined ranges differ by more than Δ, or that a
form walls them apart. That is a difference constraint only once it is
known which is over: at a crossing of two levels the source says, and
H4's g_min > Δ gives it; at a crossing of one level, or two unjoined ways
on the ground side by side, it does not (section 4). Until then it is
checked on the final surface, not solved for.

An infeasible region is a connected part of the constraint graph with no
solution. What is recorded is the regions and, for each, one negative
cycle with the places of its constraints — not a count of cycles, since one
contradiction appears as many. Every such judgement is conditional on the
values section 4 leaves open (g_min, and any least clearance or cover that
is made a bound). That the heights are feasible does not make the final
surface keep the contract: it is checked whole, H5 included.

What the bake does with such a region is open (section 4).

### 3.1 What the heights are a problem of

The heights of joined ways are one problem, not a structure's each: where a
way's form changes, or two ways share a connector, neither end is held by
its own form and the other made to meet it.

- **Several surfaces may stand at one place.** Two tunnels one over the
  other are two surfaces, and so are a deck and the way under it; a model
  of the heights has a surface, not a place, for its unit.
- **The source's consecutive ranges and its connectors owe a join**, and a
  change of form does not lift the debt.
- **A connector fixes no height.** It is where two ways must join; it does
  not flatten the place they join at, nor pin either to anything. What is
  owed is the join and Δ on the final surface.

The guarantees decide which arrangements of heights are allowed; they do
not pick one. Which of the allowed is made is the inference's objective
(section 4), still open; until it is chosen, what is defined is the set,
and the one surface made is not.

**The reference model is the final surface**: the contract is stated and
checked on it, cell by cell. The per-cell model is the measure of
correctness; a model of samples is a candidate for keeping its guarantee in
less, to be compared against it on the same guarantee — the bake's
computation, its working memory and what is stored, each apart. How the heights are computed, and what is
stored, need not be per cell. Two ways to compute them are on the table,
neither chosen:

| Variables                                  | For                                    | What it leaves                                                                                 |
| ------------------------------------------ | -------------------------------------- | ---------------------------------------------------------------------------------------------- |
| one height for each cell of each surface   | states Δ on the final surface directly | a variable for every cell across every way's width: the most for the bake to compute and hold  |
| heights at samples or sections along a way | possibly fewer variables               | a construction that keeps Δ across the width, on a curve, at a join and once quantized, proven |

Heights at a range's ends and at connectors alone are not a third: they
control neither a steep stretch between them nor a step across a width,
unless a rule of interpolation and of added samples comes with them, which
makes them the second.

What the bake needs to compute a surface that keeps Δ, and what the client
is given to draw it, are judged apart: the one need not be the other.

### 3.2 One construction from samples, examined

What is examined here is one construction, the nearest station; where it
fails, it fails as that construction, not as the limit of every model of
samples.

**The construction.** Each way has a profile h(s): heights at samples along
its centreline, linear between, s the distance along the way in u, counted
over the whole way so that no page cuts it. A cell c of the way's surface
takes H(c) = Q(h(σ(c))): σ(c) is the station of the point of the centreline
nearest c's centre, and Q the client's quantization to the stored quantum q
(`round`, half to even).

**Quantization.** For a positive integer k, |a − b| < kq gives
|Q(a) − Q(b)| ≤ kq, whatever the ties: Q moves each by at most q / 2, and
two multiples of q less than (k + 1)q apart are at most kq apart. It is
sufficient, not necessary: 0 and q stay q apart, while 0.5q and 1.5q,
exactly q apart, round to 0 and 2q.

**A pair of cells.** h is linear between samples, so with g the steepest
grade of the profile, |h(σ(a)) − h(σ(b))| ≤ g · |σ(a) − σ(b)|. A pair of
neighbours a, b of one way keeps Δ = kq when g · |σ(a) − σ(b)| < Δ, or when
their two quantized heights are compared directly. That is the whole of
what this construction is held to; what bounds |σ(a) − σ(b)| is geometry,
and none of it is assumed here:

- σ is a function only where the nearest point is unique, which holds
  within the centreline's reach (Federer), and the reach is the lesser of
  what its curvature and its bottlenecks allow: two parts of a way far
  apart along it but near in plan take the nearest point from each other,
  whatever its radius.
- The source's centreline is a polyline, and a polyline's reach is 0 at
  every bend: on the inside of one, the nearest point leaves one segment
  for the next across the bend's bisector, and σ jumps there.
- Where σ is unique and continuous, a bound on the projected points'
  distance in plan is not yet a bound on their stations, which run along
  the way. Neither is derived here.

So a radius of curvature alone bounds nothing, and no condition on the way's
width and radius is claimed for this construction.

**A counterexample, on a bend.** A way runs (−100, 0) → (0, 0) → (0, 100) in
u, 8u wide, h(s) = 0.2 s, cells of 1u and Δ = q = 1u. Along it, two cells a
station apart differ by 0.2. The neighbours centred at (−3.5, 3.0) and
(−3.5, 4.0) stand on either side of the bend's bisector: the first is
nearest the first segment, at s = 96.5, h = 19.3; the second the second
segment, at s = 104.0, h = 20.8. They draw at 19 and 21, two steps apart.
A point on the bisector t from the vertex takes s = 100 − t on one side and
100 + t on the other, so the jump grows to the way's whole width.

**Today's renderer, apart.** The client does not take one station: a deck's
column stands at the highest of its line over the stations its square spans,
a bore's floor at the lowest (`pxl-spec bake/deck.rs` `over_segment`,
`occupies`). A segment at angle θ to a cell's side spans
ρ = (L / 2)(|cos θ| + |sin θ|) either side of the centre's station, up to
L / √2, L the cell's side, clipped where a line ends square; several
segments over one column are merged, the least low and the greatest top.
For two neighbours over one straight segment of one form, away from its
ends, the two spans are the same length and shifted by at most L · max(|cos θ|, |sin θ|), so their
extremes differ by at most g · L. At a change of form the two take opposite
extremes: a deck's highest and the bore's lowest beside it differ by up to
g · (L |cos θ| + 2ρ) ≤ √5 · g · L, reached at tan θ = 1/2 on a way running
downhill from deck to bore. Where a column holds a vertex, no bound is
derived. An instance, as stored and drawn: two lines of width 2 cells, a
deck (0, 0) → (5, 2.5) with tops 13 → 10 and a bore (5, 2.5) → (10, 5) with
tops 10 → 7, points on the stored eighth of a cell and tops whole cells as
`Builder` keeps them. Through `over_grid` and `occupies`, with columns of
one cell, the deck's column (4, 2) spans tops 10.0 – 10.6 and its slab's top
is 11; the bore's column (5, 2) beside it spans 9.4 – 10.0 and its floor is
9. Every other pair of neighbours there differs by at most 1.

**What neither touches.** Each way is built from its own profile, so
neither construction says anything of the cells two joined ways share around
their connector; that is the junction's (section 3.3).

### 3.3 One junction, constructed

A construction for the junction at a connector k, examined on a Y: a way A
ending at k, B and C leaving it at an angle θ to each other, every way of
one half-width.

**Its cells**, in plan alone, which fails in section 3.5's experiment
where the ways meet at different heights. A cell of a range ending at k is _shared_ when another such
range's surface holds it too, or holds a cell beside it. The junction J(k)
is the connected part of the shared cells that holds k. Ranges that do not
end at k take no part, and the same ways meeting again away from k, apart
from J(k), are not in it.

- **Where a way enters.** A range's _collar_ is its cells outside J(k) that
  share a side with J(k); it enters there and nowhere else.
- **Where the ways part, on this Y.** No collar cell touches a cell of
  another range ending at k, since one that did would be shared and joined
  to J(k). That is all the construction gives: the same ways meeting again
  away from k make shared cells of their own, outside J(k) and owed nothing
  by it, and a contact running on from k can carry J(k) to a meeting that
  is not k's. On the Y below neither happens; that J(k) keeps the source's
  joins and no others in general is not claimed.

**Its heights, the boundary fixed.** Given J(k) as a fixed graph, the
collars' heights h(b) already quantized, multiples of q, and Δ = kq for an
integer k, J(k) is filled from them. With d(a, b) the fewest steps from a
to b through J(k),

- U(c) = min over collar cells b of h(b) + Δ · d(c, b), the highest a
  surface that keeps Δ can stand at c;
- W(c) = max over b of h(b) − Δ · d(c, b), the lowest;
- H(c) = q · ⌊(U(c) + W(c)) / 2q⌋, the mean rounded down in units of q.

U and W keep Δ between neighbours (McShane, Whitney), so does their mean,
and rounding down in units of q keeps it: ⌊y / q⌋ ≤ ⌊(x + kq) / q⌋ =
⌊x / q⌋ + k. Half to even does not, 0.5q and 1.5q rounding to 0 and 2q.
Where the collars are _consistent_, |h(b) − h(b′)| ≤ Δ · d(b, b′) for every
two of them, W ≤ H ≤ U and every collar is met exactly; where they are
not, no surface of J(k) keeps Δ with them, and the pair is the witness. So
consistency is necessary and sufficient for this extension, with Δ the only
constraint in it: a cover, a clearance or another structure in the way is
not part of it. H is one surface that shows a solution exists, not the one
the inference picks; the terrain and a gentle grade are the objective's
(section 4).

**On a grid.** Cells of 1u, half-width 2 cells, Δ = 1u, A level at 10, B
rising and C falling at grade g, each range outside J(k) by its nearest
station (section 3.2):

| θ   | g   | J(k), cells | collars    | worst step |
| --- | --- | ----------- | ---------- | ---------- |
| 90° | 0.2 | 22          | consistent | 1          |
| 60° | 0   | 28          | consistent | 0          |
| 30° | 0.1 | 42          | consistent | 1          |
| 30° | 0.3 | 42          | 8 pairs ✗  | 3          |
| 15° | 0.1 | 72          | 4 pairs ✗  | 2          |

At 30° and g = 0.3, B's collar cell (7, 1) stands at 12 and C's (7, −2) at
8, three steps apart through J(k). The narrower the angle, the longer J(k)
and the farther from k its collars: at 15° even a gentle grade fails.

**What follows.** Once a junction's collars are fixed by building its
ranges apart, filling J(k) cannot always meet them, and fails exactly
where they disagree. So the collars are constrained with the ranges'
heights (section 3.1), through three steps that are not alike:

1. **Samples to a collar's height.** For a fixed J(k) and fixed stations,
   h(σ(b)) is linear in the samples, a station lying between two.
2. **Quantization.** Q(h(σ(b))) is not linear in them: 0.5q and 1.5q meet a
   linear bound of q and are drawn 2q apart.
3. **The junction.** Given quantized collars, consistency is a set of
   difference constraints on them, and the fill above meets it.

Step 2 is where a model of samples must choose. Its quantized collar heights
can be variables of their own, multiples of q tied to h(σ(b)) within q / 2
with a rule for ties, which makes it an integer problem. A sufficient
condition can stand instead, |h(σ(b)) − h(σ(b′))| < Δ · d(b, b′), linear
and strict, keeping a margin and refusing some surfaces that would have
kept Δ. Or the samples are solved without it and the quantized collars
judged after, which guarantees nothing until that judgement. The per-cell
model has no step 2: its variables are the quantized heights themselves,
J(k)'s cells among them, and Δ between them is a difference constraint in
whole steps, whose system has a solution in multiples of q exactly when it
has one at all.

Not yet examined: a junction of ways of different widths; two connectors
whose junctions touch, which section 3.4 takes up; a way that leaves J(k)
and touches it again within one range.

### 3.4 Junctions that touch

Two connectors' junctions can touch or share cells. That they touch asks
two things, and one does not answer the other: whether their heights are
one problem, solved together, and whether any of their cells are one
surface. Solved together does not make one.

**What is solved together** is a connected part of the constraint graph:
two junctions are in one wherever a constraint, a join's or H1's, links
their cells. It follows from the constraints and needs no rule of its own.

**What is one surface** is decided cell by cell, by the join relation
(section 2.1): the ranges at a cell are one surface where they are joined
there, and separate surfaces, each with a height of its own, where they
are not. A model's unit is a cell's surface, not a cell (section 3.1).

**Which contacts join** is the same relation between two cells: joined,
owing Δ, or not, and asked apart. So every step of a passage follows a join
the source makes. From A to B, which the source does not join, a passage
goes by C, joined to each: into C at one place, along C, and out to B at
another, or at one cell where the three are one surface; never across a
contact of A's beside B's.

**Surfaces at one place.** A cell can hold two surfaces of the same form, a
deck over a deck or two bores, and that is not a fault. Whether they can
stand there is a question of their heights and of what each builds, solid
or open, not of their forms; two at grade within Δ of each other at one
cell is the open case of one level (section 4).

On a grid (cells of 1u, half-width 2 cells), the surfaces and contacts in
plan alone, before any height or what each builds; whether the surfaces
counted can stand together is not tried here:

| Shape                                        | Two surfaces | Unjoined contacts |
| -------------------------------------------- | ------------ | ----------------- |
| two side roads on a main road, 5u apart      | 0            | 0                 |
| the same, 4u apart                           | 0            | 14, of S1 and S2  |
| the same, 3u apart                           | 14           | 54, of S1 and S2  |
| a dual carriageway across a dual carriageway | 0            | 0                 |

Where the side roads meet the main road the junctions share the main
road's cells, and there each side road is the main road's surface; the
contacts left unjoined are the two side roads beside each other along their
length, the open case of ways side by side, which the junctions neither
make nor remove. Joined ways at different heights are section 3.5.

### 3.5 A ramp into a deck, a failed experiment

What this case shows is that two constructions of a junction fail with
these values held fixed; not that the source contradicts itself, nor that
any cells must be one surface.

| Given by the source              | Owed                                 | Chosen for this experiment                 |
| -------------------------------- | ------------------------------------ | ------------------------------------------ |
| the geometry, at 8.5°, 3u beside | the joins (section 2.1)              | the deck at 8u                             |
| the connector k, ending D₁ D₂ R  | Δ                                    | the ramp from 0 to 8 at a grade of 0.4     |
| D's bridge flag, R's none        | the forms, and an order where agreed | 4u wide; a slab of 1u, headroom of 2u      |
|                                  |                                      | the junction, by section 3.3 or by k alone |

Cells of 1u, Δ = 1u.

**By section 3.3.** R's surface touches D's for all its length, so J(k)
holds 99 cells from x = −22 to 2, 76 of them R's, down to where R meets
the ground. As one surface with the deck its collars cannot meet: a cell
of D's at 8 and one of R's at 0 are three steps apart through J(k), where
three steps allow 3u.

**By the connector's cross-section alone.** Converging on k, R's surface
and D's overlap in plan on 57 cells, two surfaces each, and on 32 of them,
from x = −8 to k, R stands within the slab and the headroom under D.

Each is the failure of the whole set of choices in the right-hand column,
held together. Which of them gives way — the two profiles solved together,
the width, the slab or the headroom inferred otherwise, or another
construction of the junction — the experiment does not say. Nor does a
clash of two surfaces make them one: one surface has one height, a height
within Δ is how two surfaces join, and neither a clash nor heights drawn
near changes what the source joins. What can be solved together while the
source's joins are kept is the next question (section 4).

## 4. Open

- **The terrain against gentleness.** Within the guarantees, a surface may
  stay near the DEM, or be gentle, or both within a tolerance. The orders
  give different maps:
  - _Three cells, DEM 0, 1, 2u._ The DEM itself keeps H3. Gentleness first
    flattens it to 1, 1, 1; the DEM first keeps it.
  - _A deck's end to the ground, a short stretch to the next join._ The DEM
    first lands the ramp as steeply as Δ allows at the deck's end;
    gentleness first spreads it to the join, which is no boundary of
    anything (a connector is a join, not a height).
  - _A tunnel between two portals on a hillside._ How deep its middle runs
    is the cover's, an inference.
    Nothing in the source decides this, and national design standards are not
    the world's. Not yet chosen.
- **g_min, and a deck's least clearance and a bore's least cover.** The
  source says over and under; how much is inferred.
- **A crossing at one level.** The source's `level` orders nothing there.
  Where the two forms differ — a bridge over a ground way, a tunnel under
  one, both at level 0 — the flags suggest an order; that is an inference
  from a range's structure, not the stacking order `level` states, and is
  recorded as a candidate, not made a rule. Where `level` and the forms
  would disagree, the disagreement is reported, never settled by which is
  compared first. Where the forms are the same too — two ground ways, two
  tunnels — nothing suggests one; the cause is not assumed (a missing
  junction, an error, or a real crossing) until the source is looked at.
  Not yet chosen.
- **What is solved together.** With the source's joins kept, which of the
  inferred values are variables of one problem and which are fixed before
  it: every range's heights, the cells of each junction, a way's width
  where the source gives none, a deck's slab, its clearance and headroom,
  a bore's cover. Section 3.5 fixed all of them and failed; which give way,
  and in what order where several could, is not chosen.
- **An infeasible region.** The bake fails; or it writes the region with its
  evidence recorded and the drawing short of the contract there. Not yet
  chosen.
- **Two unjoined ways side by side on the ground.** At their widths two
  ways the source does not join can touch in plan — a sidewalk beside its
  road with no connector, two roads close together — and on the ground at
  one height that is a realized join H1 forbids. Kept apart in plan (a gap
  of land between), or by height, or the contract read as allowing a
  ground contact at a road's edge. Not yet chosen; counted first.
- **A paved gap** between a sidewalk and its road (`corridor`), which
  paves two ways the source does not join into one: the same question.
- **What is drawn underground**, a bore's air: a cost to decide apart from
  this contract.

## 5. How it will be solved

Not chosen. The height layer's feasibility (section 3) is a negative-cycle
search (Bellman–Ford, or a faster one on this graph's structure). The
objective of section 4, once chosen, picks the solver; the guarantees are
checked on the baked set whatever solved it. A surface solved along a
centreline is not assumed to keep H3 across a way's width or once quantized:
those are checked on the cells.

## 6. Cases

Each a small fixture with its answer, an impossible one among them:

1. A bridge whose range runs straight into a tunnel's, with no ground
   between, as on a hillside: one surface, continuous across the change of
   form (H3). Its being infeasible would put the model of a portal or an
   abutment in doubt, not the data.
2. Three ways joined at one connector over a DEM that differs across it:
   one height at the join, H3 out along each.
3. Two ways crossing at levels 0 and 1, no connector: the higher over by
   g_min; never neighbours.
4. Two ways crossing at one level: section 4's open case.
5. A deck between two joins, too little length for its clearance over the
   way under it: infeasible, or the clearance yields if it is an inference.
6. A way across a page's edge: one surface from both pages (H5, H3).
7. Heights stored at q, Δ a multiple of q, a run that rounds two ways: H3
   on the stored values.
8. A ramp on a range the source does not flag: the ground form, solid
   under, never a deck (H2).

## 7. What can be surveyed now

Three surveys, kept apart:

- **The source**, needing nothing chosen: crossings at one level and
  without a connector; the flag combinations ranges carry and how they
  meet (a bridge's range ending at a tunnel's); joins that cross levels.
- **Today's bake**, once the definitions above are agreed: ranges whose
  path breaks, a range's boundary to the next included (H0); joins realized
  where the source makes none — a plan contact alone is not one — and
  required joins left unrealized (H1); ranges building a form their flags
  do not give, an approach drawn as a deck (H2); neighbours past Δ (H3).
- **The new model's feasibility**, only once section 4's values are set:
  its infeasible regions and their evidence.

## 8. What today's bake was seen to do

Looked at directly with `geo-bake check_surface` — a diagnostic, not the
contract's verifier, so a clean run of it proves nothing — which bakes a page
and its eight neighbours as the bake does, draws the page's chunks through the
client's own `ChunkSample`, and walks each way by the cells the bake gives it
(the code of 2026-10-05, on `carto-standard`). Each line is one way, read from
the source's ids; a page's other counts are provisional and not these.

- **A bridge straight into a tunnel** (Namsan, 127.00451 37.55363): at
  connector `7a2a1779…` the deck stands at 48 and the bore's floor at 41, a
  step of 7u; at `68feb3a4…`, 48 against 40. Each structure's end is held at
  its own form's bound, so a joined way breaks where its form changes (H1,
  H3; section 6, case 1).
- **A plain bridge's ends** (`e8ba50c4…`, Gangnam 127.02880 37.47868; the
  same on `06dbda2d…`, Yeouido): where each deck's range ends, the ground
  range beside it stands 2u off the deck and off its approach, and the
  approaches stand on air (H2). The ground range is matched to the deck's
  end by position, since a ground piece keeps no range: a strong sign, not
  a checked boundary.
- **A way on the ground** (Namsan): unflagged hillside roads step 2u between
  neighbours following the DEM alone — H3 is owed by ground ways too, not
  only by structures. How a ground way meets it is not chosen.
- **Tunnels that share a column** (Yeouido): the client keeps one bore a
  column, the widest, so of two tunnels through one column one is lost or
  drawn on the other's floor — a limit of today's representation, not a
  rule: separate tunnels may not be merged into one.

## What it retires

Of today's solve, if this holds: the approach's preferred grade
(`PREFERRED_SLOPE`, 8%); an approach drawn as a deck with air under it;
an order at one level by `gers_id`; a floor that silently wins over a
ceiling; a bore's portal held at its cover's depth.
