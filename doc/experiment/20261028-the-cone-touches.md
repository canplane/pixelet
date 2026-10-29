# The cone touches

**2026-10-28.** E3 of the grazing rows
([a ray is read against the light that arrives](../minutes/20261028-a-ray-is-read-against-the-light-that-arrives.md),
[the rows, seen](./20261028-the-rows-seen.md)): the law the sessions
converged on, built in an export and timed against HEAD.

The law, as it was put:

- touch instead of pass, `r = c/2`;
- a touched split descends into all its children, not only those the line
  crosses;
- the raster grows a box by `g ≥ r + ρ_s`, with a margin;
- the pixel takes the nearest grown box, with no axis refusal;
- the beam's prune term goes, and its nesting is asserted;
- the walk again stays as the safety net.

The bar was today's visits and today's p95.

**Dropped.** The visits held, and the time did not, by five to seven times
in the sample and raster passes at every view (below). The prune was then
taken off the eye instead, which the rows needed and the frame could carry
([the rows, seen](./20261028-the-rows-seen.md)).

## Why

E2 showed the grazing rows are the eye's prune, and that taking it off costs
about a millisecond of GPU p95 at the horizon. The law was the sessions' way
to keep a cut of the horizon's walk and still leave no row empty. It stops a
sample where its cone touches anything, rather than passing what the cone
cannot tell apart: conservative rasterizing turned to the ray's side.

## How

An export of `d8f712deb` on :5178, against :5176 (HEAD's code). The patch
(704 lines) is kept outside the tree, at the session's scratchpad
`e3.patch`.

**The walk** (pxl-spec-core cast.rs, a new `touched`). It takes the eye's
`Until::Opaque` walks, the samples and the walk again: `walk` hands over
where `detail.apart` is 1 and the cone is not zero. The sun keeps `walk`
and its prune, and control's cone is `EXACT`.

- **The touch.** At `t` the ray is widened by `TOUCH · (across + angle · t)`
  on every side, with `TOUCH` = ½. Each axis then bounds `t` linearly:
  `(d + k)·t ≥ lo − a` and `(d − k)·t ≤ hi + a`. A box is touched where
  those bounds meet within the ray's span (`Touch`). The test is exact, and
  a node's test holds for every node under it.
- **Why not `r` at the nearest point.** A first form took `r` at each node's
  nearest point, as E1's search did. A big split near the eye then had a
  small `r` and was passed whole, along with far cells under it that a wider
  `r` would have touched. The CPU check below caught it: stops came up to
  twice as late as the analytic touch.
- **The descent.**
  - A touched split tries all eight children, nearest first by where the
    touch meets each. Each pick is the nearest after the last one, so no
    frame holds which children are left.
  - A child is asked only where the touch meets it before the best stop so
    far.
  - A touched opaque cell is the stop where it is nearer.
  - The node the walk resumes in is climbed out of into each node over it.
    Every child there is tried but the one it came from.
  - Past a root, the walk goes into the next.
- **The beam.** The eye's beam prunes nothing.
- **The asserts** sit beside `beam::SAMPLE`: `GROW` = 0.75 ≥ `TOUCH` +
  1/(`SAMPLE`·√2), and the beam's 2√2 ≥ `TOUCH` + 1/√2.

**The visible sample** (pxl-shader raster.rs, raster.wgsl).

- The eye's sample boxes are grown by `GROW · s` times the distance to
  their nearest point. Every face turned to the eye is drawn.
- A pixel takes the nearest grown box its ray enters, across any axis. The
  sun is unchanged.
- A visible sample keeps its sample's face word (`face_cs`) only where it enters
  across the sample's axis.

**Checked on the CPU first** (a throwaway test in the export). Flat ground 15
under the eye, cone 0.010417, 399 rays from 0 to 0.4 rad down:

- No touch stops later than the exact walk.
- Every stop lands at the analytic first touch, `15 / (sin θ + k)`, to 1e-3.
- The same holds resumed from a node at zooms 4 to 26 holding the ray a
  tenth of the way.
- The visit counts there (touch 34.7, exact 30.6, today 28.8) do not carry
  over. That ground asks leaf cells at every distance.

**Two things naga and Tint refused**, kept here for the next build:

- A per-level array of children left, read and written across branches.
  naga made the shared element pointer a pointer-typed local and refused it
  (`LocalVariable InvalidType`). The walk now holds no such array.
- `f32::MAX`. naga prints it as a literal Tint reads as past f32.

**Timed** in Chrome at dpr 2, a 2056×1169 CSS viewport and a fixed render
scale of 1.00, at sky 3 p.m.

- Each run is the E2 motion: turn ±500 px, pan ±250 px down, pan ±300 px
  across.
- Each page was loaded fresh, given the E2 tilt for the view, and given one
  warm-up run.
- Horizon was timed on both ports with shadows off. Down was timed on E3
  only, against E2's HEAD numbers at a 2056×1113 viewport.
- The rest of E2's matrix was not run (see below).

## What was seen

**Horizon, shadows off** (ms, p95 unless marked):

| | :5176 HEAD | :5178 E3 |
|---|---|---|
| fps | 60.0 | 38–43 |
| interval p50/p95 | 17/18 | 22–24 / 37–45 |
| missed frames | 0% | 18–35% |
| gpu p50/p95 | 9.7/17.2 | 10.7–13.6 / 38–39 |
| beam | 2.9 | 4.4–5.5 |
| sample | 2.5 | 14.8–18.2 |
| sraster | 1.4 | 7–9 |
| pixel | 1.4 | 3.9 |
| miss | 1.1 | 0.1 |

**Horizon, ray_counts** (nodes a cast):

| lane | :5176 HEAD | :5178 E3 |
|---|---|---|
| eye beam | 897 575 | 869 652 |
| eye sample | 1 795 824 | 1 827 382 |
| eye miss | 708 459 | 1 807 854 |
| took / walked   | 58.57% / 0.20% (39.6 each) | 60.52% / 0.94% (21.7 each) |
| boxes | 2 798 | 1 550 |

The two ports' walkers did not stand at the same place. HEAD's view had the
water patch before the walker, and E3's did not. So the counts compare
lanes, not one view. A first E3 build, with `r` at the nearest point, had
counted beam 857 k, sample 1.75 M, no walks again and 1 336 boxes, at
sample p95 34–36 ms.

**Down, shadows off.** E3: fps 49, interval 17/31, gpu 5.5/29.0, sample
12.3, sraster 7.1, pixel 3.9, beam 1.7. HEAD in E2: interval 17/19, gpu
8.4/10.9, sample 1.5–2.2, sraster 0.6–1.0, pixel 1.6.

**The look**, before the user's own:

- The ground and what is placed draw alike, with no rows seen at the views
  above.
- A strip of water before the walker drew as bands with ground between them
  on one load.
- Outlines go where a visible sample enters across another axis than its
  sample's, since that face's word is dropped.

## What it says

The law meets the visit bar and fails the time bar, by far.

- **The visits hold.** Beam and sample nodes are within 3% of HEAD's. The
  first build, which touched less, walked no pixel again and drew half
  HEAD's boxes.
- **The cost is elsewhere.** The sample pass is about six times HEAD's at
  both views, looking down as much as at the horizon. So the cost is not the
  grazing walk.
  - It is what each visit now does. Up to eight children are tested a pick,
    and the start is climbed out of.
  - The sample entry now holds both walks, since the sun still takes
    `walk`. sample.gen.wgsl is 1.42 MB against 0.67.
- **The raster costs five to seven times HEAD's.**
  - Every eye box draws three faces, not one, and each is grown by
    three-quarters of a cone on every side.
  - Looking down, where a cell is one or two cones wide, that is several
    times the fragments.
- **What follows.** Visit counts are not the cost check this law was to be
  settled by. They held while the time went up sixfold. The law is not
  taken as built. Any next form needs, before any timing:
  - a descent with no per-child scan;
  - a visible sample that does not draw three grown faces a box.
