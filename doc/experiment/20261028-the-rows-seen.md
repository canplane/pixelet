# The rows, seen with the eye's prune off

**2026-10-28.** E2 of
[what the grid cannot tell apart](../minutes/20261028-what-the-grid-cannot-tell-apart-stands-coarse.md)
and [a ray is read against the light that arrives](../minutes/20261028-a-ray-is-read-against-the-light-that-arrives.md):
whether the rows that go empty at grazing angles are the prune's, and whether
they line up with the node grid at their zoom (the face/inner-side reading:
`meets_box` shrinks only a side inside a node, so ground on a node's face is
not lowered and ground inside the next one is). Then, at the user's word,
what turning the prune off costs.

## Why

Since at least 10-27 14:00, rows of coarse cells went empty at grazing
angles. Looking straight down, none did. The user saw them early and let them
pass for speed. When they were raised (the screenshot of 10-27 22:30), the
sessions first read them as centered kept boxes and built a touch rule that
only the ground set. It broke the symmetry between the ground and what is
placed, and was reverted whole (`8ee81810d`).

The reading then narrowed to the eye's prune (`cf769abc1`, 10-27). It passes
a split whose content box the ray misses once each inner side is shrunk by
half a cone, so it could pass a row as air. 10-14 had proved that stripes
follow whenever what is drawn is only what rays stopped at
([the eye decides](../minutes/20261014-the-eye-decides.md)), and the CPU probe
(E1, [grazing on the CPU](./20261028-grazing-on-the-cpu.md)) found the
prune's face/inner asymmetry. What was left was to see it, and to know what
the prune still buys.

## How

Two exports of `458155604` side by side, each its own port and target:

- **:5176**, HEAD as it is.
- **:5177**, HEAD with the eye's prune off: in pxl-spec-core cast.rs `walk`,
  `meets_box`'s `prune` is zero where `detail.apart` is 1 (the eye's; the
  sun's is `sun::BEAM`, so its map keeps the prune; control casts with
  `Cone::EXACT` and prunes nothing either way). Its generated shaders were made
  from that tree.

Each page is opened fresh, so both start at the same place and view (the
camera is not kept). The user looked at both at the grazing views: the
terraces and the flat sand of the 10-27 22:30 screenshot, tilted toward the
horizon.

**The timing.** One per cell, asked for by the user. Chrome ("Chrome Tahoe"
in Claude in Chrome), one tab at a time in the foreground, dpr 2, a
2056×1113 CSS viewport, render scale 1.00; sky time fixed at 3 p.m. Two
views, each from a fresh load:

- **Horizon:** the start view tilted up by a scripted right-button drag of
  250 px, low over flat sand with the terraces behind.
- **Down:** the start view tilted down by 350 px, straight onto the ground
  around the walker. The two ports' frames land a few cells apart.

In each, the profiler records one fixed motion of scripted pointer events:
turn right 500 px and back (90 frames each way), pan down 250 px and back,
pan right 300 px and back (60 frames each). That returns the camera to where
it began. Each view runs with shadows on and off, two recordings each, then a
third with ray_counts on (shadows on; counting slows the casts, so its
timings are left out). Spans are the profiler's p95 over the frames that ran
them; ray_counts gives nodes a cast per lane over the counted frames.

The walker-height view was not taken: it needs its own setup on both ports.

## What was seen

The user's look: on :5177 the rows are gone entirely, and it is not
noticeably slow. Near the horizon some shimmer remains, which the user parks
as post-processing.

**Horizon, interval and GPU** (ms, p50/p95; missed is the share of moving
frames past their interval; — where it was not kept):

| shadows | port | interval | missed | gpu |
|---|---|---|---|---|
| off | :5176 HEAD | 17/18, 17/18 | — | 10.5/16.0, 10.0/16.3 |
| off | :5177 off | 17/21, 17/21 | 5%, 5% | 9.6/17.3, 9.2/17.3 |
| on | :5176 HEAD | 18/23, 18/24 | 13%, — | 10.0/21.6, 8.7/23.3 |
| on | :5177 off | 17/24, 18/23 | 19%, 13% | 9.3/23.5, 10.0/23.7 |

**Horizon, spans** (p95 ms, the two recordings' range):

| shadows | port | beam | sample | sraster | pixel | miss | sun beam · sample · walk |
|---|---|---|---|---|---|---|---|
| off | HEAD | 2.8–2.9 | 2.5 | 1.4–1.5 | 1.6 | 1.2 | |
| off | off | 3.3–3.4 | 2.5–2.6 | 1.1–1.2 | 1.6 | 2.0 | |
| on | HEAD | 2.6–2.8 | 2.2–2.7 | 1.4 | 1.6 | 1.0 | 2.6–3.0 · 1.4–1.5 · 0.7 |
| on | off | 3.3 | 2.6 | 1.1 | 1.6 | 2.0–2.3 | 2.6–2.7 · 1.2–1.4 · 0.7 |

**Horizon, ray_counts** (nodes a cast):

| lane | :5176 HEAD | :5177 off |
|---|---|---|
| eye beam | 875 998 | 901 460 (+3%) |
| eye sample | 1 807 993 | 1 847 604 (+2%) |
| eye miss | 721 159 | 1 941 742 (×2.7) |
| eye took / walked   | 58.65% / 0.21% (38.0 each) | 58.45% / 0.53% (40.4 each) |
| eye boxes | 2 708.5 | 2 147.4 |
| sun beam / sample / miss | 322 797 / 256 447 / 371 684 | 340 062 / 266 581 / 384 527 |

**Down**:

| shadows | port | interval | gpu | beam | sample | sraster | pixel | miss |
|---|---|---|---|---|---|---|---|---|
| off | HEAD | 17/18, 17/19 | 8.4/10.2, 8.3/10.9 | 0.7–0.9 | 1.8–2.2 | 0.9–1.0 | 1.6 | 0.2 |
| off | off | 17/19, 17/19 | 8.7/10.0, 8.7/9.8 | 0.6–0.7 | 1.5–1.6 | 0.6 | 1.6 | 0.3–0.5 |
| on | HEAD | 17/18, 17/18 | 7.9/15.3, 8.0/15.0 | 0.5–0.6 | 1.3–1.8 | 0.7–0.9 | 1.6–1.7 | 0.1–0.2 |
| on | off | 17/18, 17/18 | 8.4/14.9, 8.3/14.9 | 0.6 | 1.4 | 0.6 | 1.6 | 0.2 |

With shadows on, the sun's spans are the same on both (beam 2.5–2.6, sample
1.1, walk 0.4–0.5). ray_counts looking down: eye beam 573 933 against
574 810, sample 1 342 178 against 1 299 616, miss 7 against 29, every ray
took a sample; the few percent between them are the frames landing apart.

## What it says

The rows are the eye's prune: with it off they go, in the views the user
looked at. Whether they fall on the node grid of their zoom was not checked
with the gizmo; with the cause found, it no longer decides anything.

Off is not free where the rows were. At the horizon the eye's miss lane
visits 2.7 times the nodes, and the rays walked again go from 0.21% to
0.53%. Its p95 goes from 1.2 to 2.0 ms and the beam's from 2.8 to 3.3, less
0.3 in the raster. With shadows off, HEAD holds the interval at 18 ms p95 and
prune-off misses 5% of its frames (21 ms p95); with shadows on both already
miss a frame in seven or so. Looking down nothing moves, since nothing there
grazes. The beam and sample lanes visit within 3% of HEAD's, so the cost is
in the rays the beam leaves unresolved, not in the beam's own walk.

**Not as slow as feared.** The prune went in for the horizon. On 10-27 it took
the beam's p95 there from 10.9–12.4 ms to 3.7 ms
([a node is asked in its own units](../minutes/20261027-a-node-is-asked-in-its-own-units.md)),
and that record read as if the horizon would fall back that far without it.
It does not:

- Here, with every change since, the whole cost of turning it off is about
  1 ms of GPU p95 at the horizon with shadows off. With shadows on there is
  nothing to tell, and looking down nothing either.
- To the user's eye, too, the prune-off port is not noticeably slower.
- Much of what the prune once saved was saved again by what came after it.
  One τ for every view (`0c84382cd`) cast at a zoom coarser, and its own
  record shows the beam 8.4 to 3.0–4.7 ms and the beam's nodes 3.7M to 0.9M.
- This is a desktop, in Chrome at dpr 2. A device was not measured, and a
  phone's GPU may weigh that millisecond otherwise.

So the prune off, as it stands, still misses the hard constraint ("not worse
than now") at the horizon, if only by a little, and is not taken. The law the minutes converged on (touch, grow every band, all children
of a touched split, samples only) is what E3 prototypes, held to today's
visits lane by lane.
