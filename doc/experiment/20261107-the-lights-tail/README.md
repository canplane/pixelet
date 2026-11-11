# The light's tail

**2026-11-07, measured 2026-11-10.** Whether the S23's p95 frame still spends its time in
the light, where in the light, and why, at `ac321a51af`, the last squash. A Galaxy S23's
Chrome and Chrome on the Mac, release pages. One run of each unless a count is given.

## What was tried before

The light's tail began on 11-07, when the rays' budget went. These stand in their own records,
and none is tried again here.

| what | when | what came of it | record |
| --- | --- | --- | --- |
| A budget of the rays' visits, rays parked across frames | until 11-07 | Removed: terrain broke while moving, with ghosts. Every ray ends in its frame, so an epoch casts every tile it reads at once, and the S23's p95 became the light's | [a ray ends in its frame](../../minutes/20261107-a-ray-ends-in-its-frame.md) |
| Casting again only the tiles a world change reaches | `aa2ee5efef`, kept | `light_hit` p95 17 to 5.2–5.4 ms with a character walking | [the journal](../../gpu/experiment/20261103-the-frame-on-the-s23/README.md), "What makes the light cast again" |
| A brick's columns read by the bands that read it | 11-08, kept | The gain is not separated from run to run | the journal, the same section |
| Sliding the depth step | held, not merged | A stepped cast costs as much as the whole recast it replaces, so the tail stays | the journal, the same section |
| Keeping tiles across a depth step | weighed, dropped | A kept slot's ray would start a step from a fresh one's | the journal, the same section |
| A list of the light's cells, drawn by a raster | proposed 11-06, measured 11-08 | Set aside: 5 to 400 times the nodes the walks ask, the ground being solid | [cell raster](../../minutes/20261106-cell-raster.md), [ray, raster, ray](../../minutes/20261108-ray-raster-ray.md) |
| Denser samples, a ray per pixel | before 11-08 | Not taken: a strip may be as thin as any | ray, raster, ray |
| Testing a hole's ray against the raster's candidates | weighed 11-08 | Finds nothing: a hole is where the candidates missed | ray, raster, ray |
| A raster of the ground's heights for the light; geometry kept across frames | weighed 11-08 | Not taken: the world's one representation is the query | ray, raster, ray |

What those records measured of the light is the Mac's. At 4K on 11-08, the light's spans went
43% to the box's depth stepping, 37% to bricks and edits by their columns, 12% to a slide and 7%
to tiles read anew. A whole cast of the 1.9 M slots the receivers read took 8% through the holes'
walk (150,000 to 170,000), 42 to 45% off the samples' raster and 46 to 51% as sky. What the S23
does differently from that is what follows.

## Tried

Nothing was kept; each of these was undone after its run, in a worktree of `ac321a51af`.

- `ray_cnts` on in the drive's settings, to count the rays.
- The light's three slot counters repurposed to say why a slot was listed as a hole, and the
  counters of the light's samples repurposed to count `sample_hit_cs`'s stops.
- On top of those, one at a time: the drive's sun at noon (`s23-noon`); its zoom sweep five times
  slower (`s23-slow`, `mac-slow`); the light's `SAMPLE` at 2 (`s23-sample2`); the light drawing
  and taking every face of a box (`s23-faces`, `mac-faces`).
- Then, with the slot counters counting the slots with any box, the slots that took a voxel and
  the holes beside a stop, and the samples' counters counting the stops and the stops no entry
  holds (`s23-noentry`, `mac-noentry`), one at a time on top: the light's first instance drawn
  as a triangle over the whole map (`s23-whole`, `mac-whole`), kept for all the runs below;
  signed coordinates in the slot's read (`s23-signed`); the light's visibility cleared to 7
  (`s23-clear`); the draw made direct, of one instance (`s23-direct`) and of 100
  (`s23-direct100`); the four indirect words read back at the frame's end (`s23-args`,
  `mac-args`); the draw indirect from a copy of its words made after the pass that wrote them
  (`s23-copy`); the draw indirect from words the CPU wrote, 18 vertices and 100 instances
  (`s23-cpuargs`, `mac-cpuargs`). `s23-copy` and `s23-cpuargs` did not take a new buffer: they
  took `Raster::hits`, grown to 4 words, which is otherwise the holes' stops' dispatch.
- With the triangle and the counters, one at a time: the app, wgpu on Vulkan (`s23-app`, a
  Release built with `/git/checkouts/` added to `number-paths.py`'s roots for the run); an
  empty direct draw before the indirect one (`s23-t3-emptydraw`); the light's targets at half
  the map's side (`s23-t1-halfsize`); a new buffer of `INDIRECT | COPY_DST` alone at offset 0
  (`s23-t2-apart`); the eye's draw from a new buffer of words the CPU wrote, 30,000 instances
  (`s23-t4-eyecpu`); the eye's draw direct, so that the light's is the frame's only raster
  drawn indirect (`s23-t5-eyedirect`).
- In a worktree of `3de91d61b4` with the direct draw undone, the light drawing indirectly from
  its args as before, and the triangle and the slot counter on top: a probe pass that draws the
  triangle alone, with no timestamp, from words the CPU wrote, into the light's cleared targets,
  which the light's own pass then loads, at one place each run (`p4` before the light's beam
  rays, `p6` after its samples, `p7` after `light_sample_hit`, right before `light_draw`); and
  with no probe, the light's render pass with no timestamp (`p0-nostamp`), with a span of its
  own that it begins and ends (`p0-ownspan`), and with a fresh span whose end alone it writes
  (`p0-ownend`). The reports of the 11-11 runs are under `report/11-11-*`.

## How it was measured

- The page: `PXL_PERF_DRIVE=1 make preview` in a worktree of `ac321a51af`.
- The runs: `script/perf/android-chrome.py` and `script/perf/chrome.py` on
  `http://localhost:4173`, the 20 s drive. `report.txt` and `values.json` of each are
  under `report/`.
- Each frame's spans, counts and `light_changed` were read from its `trace.json`.
- The phone was not let cool, and its clock was not recorded.
- The later runs were read from their `trace.json`, frame by frame. A frame's counts are
  read in about one frame of two. Each run's AP is read before and after it; `s23-why`'s
  was not.
- The sweep's phase is taken from the frame's time and the drive's 960 ms leg; 0 is the
  farthest zoom and 0.5 the nearest.
- On 11-11, Chrome was 157.0.8103.50 on the S23 and 157.0.8103.39 on the Mac.

## What came of it

### Whether it reproduces

| run | where | AP, °C | frames | GPU p50 / p95 | `light_hit` p95 |
| --- | --- | --- | --- | --- | --- |
| base-1 | S23 | 46.4 to 50.8 | 211 | 76.1 / 207.7 | not read |
| rays-1, `ray_cnts` on | S23 | to 51.7 | 260 | 61.8 / 163.6 | not read |
| s23-1 | S23 | 41.0 to 48.5 | 184 | 92.5 / 216.9 | 53.0 |
| mac-1 | Mac | | 776 | 22.5 / 35.8 | 2.3 |

The tail reproduces. `light_hit` is the first pass of the holes' queue, the last ray of
ray, raster, ray (`ray/cast.rs`, the walk run over `light_hit` and `light_walk`), and it
is the largest part, not the only one. In s23-1 the costliest tenth of the frames:

| | that tenth | the rest |
| --- | --- | --- |
| GPU | 219.5 | 81.5 |
| the light's spans | 145.0 | 23.0 |
| `light_hit` | 52.6 | 2.4 |
| `light_sample` | 33.1 | 5.0 |
| `light_beam` | 19.3 | 1.8 |
| `light_walk` | 17.7 | 0.7 |

The Mac's tail has the same period and is small: `light_hit` at most 6.9 ms.

The tail comes in time. In s23-1 the 16 frames of 183 with `light_hit` of 20 ms or more
fall in groups about 1.9 s apart, the period of the drive's zoom sweep. `light_changed`
does not separate them: in base-1, 192 of 210 frames carry `content`, and the 49 of them
without `world` cost 2.6 ms of light at the median.

In rays-1 a tail frame has 570,000 to 745,000 holes of the map's 4,194,304 rays, and
1,150,000 to 1,330,000 sky. A middling frame has 42,464 holes and 1,855,520 sky. The
eye's holes in the tail frames are 4,000 to 50,000.

### Why they are holes

| run | where | AP, °C | frames | GPU p50 / p95 | `light_hit` p95 | changed |
| --- | --- | --- | --- | --- | --- | --- |
| s23-why | S23 | not read | 196 | 83.8 / 201.8 | 52.4 | reasons of the hole |
| s23-why2 | S23 | 39.8 to 44.9 | 154 | 114.6 / 194.6 | 33.4 | and the samples about it |
| s23-noon | S23 | 40.4 to 45.5 | 144 | 115.2 / 288.6 | 60.1 | sun at noon |
| s23-slow | S23 | 42.1 to 45.9 | 157 | 103.2 / 241.2 | 45.0 | sweep 5× slower |
| s23-sample2 | S23 | 40.0 to 50.3 | 192 | 73.1 / 179.0 | 25.0 | `SAMPLE` 2 |
| s23-find | S23 | 40.6 to 46.4 | 191 | 73.1 / 161.7 | 24.6 | stops counted |
| s23-faces | S23 | 40.8 to 45.8 | 158 | 112.9 / 186.6 | 25.6 | every face |
| mac-base | Mac | | 915 | 20.4 / 28.3 | 1.7 | counters on |
| mac-slow | Mac | | 988 | 18.7 / 26.0 | 1.1 | sweep 5× slower |
| mac-faces | Mac | | 876 | 20.6 / 29.0 | 1.6 | every face |

The runs differ by more than the changes: s23-why, s23-why2 and s23-find hold the same
drive and shaders but for the counters, and their p95 runs from 161.7 to 201.8. No p95
above is a change's effect.

Every slot of a tail frame's holes on the S23 is one the raster left no voxel at. In
s23-why, of 289,000 to 687,000 holes, a voxel drawn but stale or not entered accounts for
at most 564. In s23-why2 no sample about such a slot is parked: at least one stopped on a
node, but for 28,000 to 42,000 slots of the map's fringe, which every frame has.

The sun is not the cause: at noon a tail frame has 608,000 to 676,000 such holes.

The sweep's speed is not: five times slower, it still has 593,000 to 726,000. The tail
is the terrain coming into the map, at phase 0.27 to 0.41 going in and at 0.60 to 0.66
coming out. It falls over the casts after that: 669,000, 158,000, 124,000, 42,000.

The samples' spacing is not: at `SAMPLE` 2, a tail frame has 615,000 to 665,000.

Which faces a box draws is not: with every face, a tail frame has 228,000 to 367,000 holes,
at the same 15.4 holes a stop as with the faces alone.

What separates the S23 from the Mac is what the raster makes of a stop. Of the casts with
the most stops:

| run | stops | holes beside a stop | holes a stop | boxes |
| --- | --- | --- | --- | --- |
| s23-find, tail | 42,145 | 660,816 | 15.68 | 22,015 |
| s23-find, tail | 40,230 | 630,885 | 15.68 | 14,920 |
| s23-find, the exception | 17,986 | 15,543 | 0.86 | 33,104 |
| s23-faces, the exception | 22,039 | 16,522 | 0.75 | 9,561 |
| mac-faces | 48,054 | 4,291 | 0.09 | 47,983 |
| mac-faces | 46,433 | 4,292 | 0.09 | 49,629 |

A sample stands for 16 slots, so 15.4 to 15.7 holes a stop means next to no slot took a
voxel, with as many boxes a stop as the Mac's. The S23's ordinary tail frames and its two
exceptions are alike in `light_changed`, the frame kind, and the frame before.

### Where the raster's answer is lost

| run | where | AP, °C | what the draw was | casts by slots with any box |
| --- | --- | --- | --- | --- |
| s23-whole | S23 | 37.8 to 46.7 | indirect, as it stands | 105 none, 17 under 5,000 |
| mac-whole | Mac | | indirect, as it stands | every claimed slot |
| s23-signed | S23 | 38.6 to 45.5 | indirect, signed read | 106 none, 17 under 5,000 |
| s23-clear | S23 | 44.2 to 44.5 | indirect, cleared to 7 | 45 every slot, 35 none claimed |
| s23-direct | S23 | 38.9 to 45.3 | direct, one instance | 43 every slot, 40 none claimed |
| s23-direct100 | S23 | 39.4 to 44.0 | direct, 100 instances | 35 every slot, 16 none claimed |
| s23-copy | S23 | 38.2 to 47.1 | indirect from a copy | 99 none, 17 under 5,000 |
| s23-cpuargs | S23 | 39.1 to 45.0 | indirect from the CPU's words | 74 none, 10 under 5,000 |
| mac-cpuargs | Mac | | indirect from the CPU's words | 175 every slot, 149 none claimed, 45 partly claimed |
| s23-app | S23, app | 36.8 to 43.8 | indirect, as it stands | 35 every slot, 13 none claimed |
| s23-t3-emptydraw | S23 | 37.1 to 45.0 | an empty direct draw first | 104 none, 20 under 5,000 |
| s23-t1-halfsize | S23 | 38.4 to 45.1 | targets at half the side | 106 none, 1 partly |
| s23-t2-apart | S23 | 38.6 to 46.8 | a buffer of its own, offset 0 | 105 none, 19 under 5,000 |
| s23-t4-eyecpu | S23 | 38.4 to 46.1 | the eye from the CPU's words | 107 none, 20 under 5,000 |
| s23-t5-eyedirect | S23 | 39.4 to 46.0 | the eye direct | 98 none, 19 under 5,000 |

A count covers only the slots of the tiles a cast claimed, so a cast that claims none shows none.
The triangle is the first instance of each of these runs, so a draw that runs at all covers
every claimed slot.

The boxes are there. In s23-noentry the cast of 45,386 stops holds 24,307 boxes, and 16 of its
stops find no entry. The slots with any box are 471, and 375 take a voxel. The Mac's cast of
49,342 stops has 46,054 boxes and 757,862 such slots. The counts of stops do not separate the
two: the S23 is at 15 holes a stop from 2,520 stops to 45,386, and the Mac's first casts of the
terrain, of 24,619 and 41,751 stops, are at 0.72 and 0.29.

The slot's pass sees the raster's pass: cleared to 7, every claimed slot reads it (s23-clear).
So the loss lies in the draw. The indirect words are right at the frame's end: 18 vertices,
as many instances as boxes, and 0 and 0, in all 108 casts of s23-args and all 376 of mac-args.
Neither a copy of the words made after the pass that wrote them nor words the CPU wrote brings
the draw back. The same draw made direct covers every claimed slot.

The eye's draw takes the same pipeline through an indirect call and works on the S23: its median
cast has 1,329,968 voxels and 11,409 holes (s23-noentry).

The app answers on the same phone: with wgpu on Vulkan the triangle covers every claimed slot
through the same indirect call, and its casts of 12,037 to 17,089 stops take 148,971 to 246,432
voxels. The app's wgpu is a patched fork (`rust/Cargo.toml`, `[patch.crates-io]`,
[the patch](../../gpu/minutes/20261110-the-patch-that-fixed-one-phone.md)), so it is not the same stack
with another front. What it says is that the loss is not the phone's alone. It lies on the page's
path on this phone.

None of the differences tried between the eye's call and the light's brings the light's draw
back: not an empty draw first, not the targets' size, not a buffer of its own, and not the eye
going direct. The eye lives on words the CPU wrote, its median cast 1,315,712 voxels and 14,665
holes (s23-t4-eyecpu).

Three more, all on the page with the triangle: the eye's bind group at the light's place
(`s23-t6-eyegroup`, 120 casts none), the light's draw with no `light_rec_draw` pass before it
(`s23-t7-nocompute`, 125 none), and the light's pass kept out of a command buffer of its own
(`s23-t10-nosplit`, 98 none, 21 under 5,000). The one that brings it back moves the draw: right
after the eye's raster, from words the CPU wrote, before any of the light's compute passes
(`s23-t8-aftereye`), 52 casts cover every claimed slot, 40 claim none and 1 partly. Which of the
passes between that place and the draw's own takes it away was not found.

The page's console on the S23, read through a whole run (`s23-plain`), holds no WebGPU warning or
validation error.

### The light's draw made direct

The light draws every place of its draws' buffer, `record::LIGHT_DRAWS` instances, directly. The
buffer is cleared before each draw, and a place that holds 0 is outside the clip volume
(raster.wgsl `vs`). The args buffer now only counts the boxes and places them. Three runs of each
on the S23's page and in the app, five on the Mac, alternating. The Mac's first two runs of each
are left out of its row: another load on the machine shows in them.

| where | | frames | GPU p50 / p95 | tail casts | holes, mean / p50 / max | `light_hit` a run, p95 | light's spans a run | `light_raster` p50 / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S23 page | before | 797 | 57.7 / 161.3 | 67 | 507,831 / 558,343 / 777,870 | 1,715, 49.9 | 6,840 | 0.85 / 1.90 |
| S23 page | direct | 853 | 57.6 / 119.0 | 83 | 93,457 / 62,570 / 1,897,984 | 682, 9.5 | 5,938 | 1.44 / 2.95 |
| Mac | before | 2,724 | 20.4 / 28.6 | 172 | 60,993 / 47,618 / 1,291,440 | 488, 1.6 | 3,705 | 0.26 / 0.66 |
| Mac | direct | 2,754 | 20.4 / 28.4 | 172 | 58,366 / 48,652 / 1,133,811 | 494, 1.7 | 3,870 | 0.46 / 0.72 |
| app | before | 730 | 74.9 / 159.2 | 70 | 71,693 / 62,675 / 132,468 | 1,233, 24.2 | 7,255 | 1.58 / 3.73 |
| app | direct | 704 | 77.4 / 158.8 | 70 | 66,691 / 61,118 / 137,474 | 1,245, 24.2 | 7,516 | 2.53 / 4.54 |

A tail cast here has more than 100,000 holes and voxels together. AP on the S23's page ran 42.8 to
48.9 before and after alike, and in the app 39.0 to 46.8. One direct run on the S23 has a cast of
1,897,984 holes, every claimed slot, and the Mac has casts over a million in both builds; those are
not the raster's loss, and were not looked into.

The S23's page now answers as the app does: 62,570 holes at the median against the app's 61,118.

Pictures, at a held view looking down at about 0.62 of the zoom, 07:00 sun, shadows on, two runs of
each build on the S23's page and on the Mac: the mean per-pixel difference (of 255) between before
and direct is 0.775 and 4.186 on the S23 and 0.828 and 0.925 on the Mac; between two runs of one
build it is up to 4.117 on the S23 and 1.210 on the Mac. The S23's shadows are the same to the eye
in the two builds. `./script/diff-check.py` and Safari's probe (178 modules and pipelines ok) pass.

### What loses the indirect draw

One run each on the S23's page, by the slots the triangle covers:

| run | what | casts | none | under 5,000 | every slot | partly | AP, °C |
| --- | --- | --- | --- | --- | --- | --- | --- |
| p0-indirect | indirect, as before the direct draw | 142 | 118 | 23 | 1 | | 31.6 to 42.4 |
| p4 | probe before the light's beam rays | 100 | 45 | | 54 | 1 | 34.6 to 44.0 |
| p6 | probe after the light's samples | 107 | 51 | | 54 | 2 | 35.9 to 44.7 |
| p7 | probe right before `light_draw` | 105 | 48 | | 55 | 2 | 37.4 to 45.6 |
| p0-nostamp | indirect in place, no timestamp | 71 | 32 | | 39 | | 37.5 to 44.1 |
| p0-ownspan | indirect in place, its own begin and end | 54 | 23 | | 31 | | 42.4 to 43.8 |
| p0-ownend | indirect in place, a fresh span's end alone | 111 | 92 | 19 | | | 38.4 to 44.9 |

A cast that claims no slot counts under none.

No pass between the eye's raster and the light's draw takes it away: the probe draws up to the
place right before `light_draw`. What the light's own pass had that the probe had not was its
timestamps. `light_raster` is one span over three passes, `light_sample_hit`'s, `light_rec_draw`'s
and the render pass, and a span's first pass alone wrote its begin (`gpu/api.rs`
`TimestampWrites`), so the render pass wrote an end without a begin. Without the timestamp, or
with a begin and an end of its own, the indirect draw covers every claimed slot in place. With an
end alone in a slot no other pass writes, it is lost as before. On the S23's Chrome a render pass
that writes an end and no begin loses its indirect draws; its direct draws (`s23-direct`) and the
eye's pass, which begins and ends its own span, are not lost. No console error comes with it.

The hook on the WebGPU calls, mended (a render pass with no color attachment broke it), showed on
the Mac the eye's and the light's render passes alike: the same pipeline, the same args buffer's
size and usage, each at the start of a submit of its own.

### The light's draw indirect again, every pass with both timestamps

What was decided is in [ray, raster, ray](../../minutes/20261108-ray-raster-ray.md), its 11-11 section.
The light's raster is drawn from its args again, as the eye's is, and the direct draw is gone.
Every pass writes a begin and an end of its own; a span over several passes is their sum
(`gpu/api.rs` `Stamp`). Runs alternate main (`3de91d61b4`, the direct draw) and this, with
`ray_cnts` on: five rounds on the S23's page, three on the Mac and three in the app (one of the
app's main runs ended with no result in 240 s, and is left out).

| where | | runs | frames | GPU p50 / p95 | tail casts | holes, mean / p50 / max | `light_hit` a run, p95 | light's spans a run | `light_raster` p50 / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S23 page | main | 5 | 1,258 | 64.3 / 134.3 | 101 | 85,953 / 66,417 / 1,298,804 | 654, 10.2 | 5,988 | 1.64 / 3.21 |
| S23 page | indirect | 5 | 1,053 | 73.4 / 149.0 | 74 | 85,847 / 65,464 / 788,240 | 614, 10.9 | 5,797 | 1.18 / 2.88 |
| Mac | main | 3 | 1,916 | 27.5 / 45.3 | 226 | 86,973 / 70,371 / 1,897,984 | 610, 3.1 | 4,311 | 0.66 / 1.31 |
| Mac | indirect | 3 | 2,351 | 23.1 / 40.3 | 153 | 74,009 / 51,062 / 1,897,984 | 480, 2.1 | 4,334 | 0.20 / 0.66 |
| app | main | 2 | 426 | 78.8 / 184.0 | 38 | 67,357 / 56,427 / 129,212 | 1,220, 25.1 | 7,465 | 2.61 / 4.88 |
| app | indirect | 3 | 711 | 71.3 / 155.0 | 68 | 71,610 / 62,231 / 311,754 | 1,203, 23.3 | 6,672 | 1.07 / 3.10 |

AP ran 35.7 to 48.9 on the S23's page and 37.4 to 50.2 in the app. The light's indirect draw
answers on the S23's page as the direct one did: 65,464 holes at the median against 66,417.

Two of the five indirect runs on the S23 ran slow, 157 and 158 frames at a GPU p50 of 117 ms, and
every span in them took 1.7 to 2 times its time in the other runs; one main run took 1.3 times.
The S23's GPU p50 above holds them. The Mac's runs were taken while the machine had other load,
and differ from each other as much as from the other build.

`light_raster` is not the same measure in the two builds: in main the span ran from its first
pass's begin to its last pass's end, and here it is the sum of its three passes.

Then the frame's spans and passes were held in fixed arrays, and three more pairs ran on the S23's
page, the GPU's clock (`/sys/class/kgsl/kgsl-3d0/devfreq/cur_freq`) read each second. A frame's
passes timed were counted into the `debug` span for these runs, undone after them. The clock
column is over the 22 seconds of the drive:

| run | frames | GPU p50 / p95 | tail casts | holes p50 / max | `light_hit` a run | passes a frame, most | clock p50, MHz | AP, °C |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| indirect 1 | 297 | 55.2 / 113.4 | 27 | 69,870 / 133,067 | 660 | 48 | 719 | 33.8 to 44.9 |
| main 1 | 296 | 58.0 / 113.5 | 24 | 60,445 / 118,980 | 629 | | 680 | 37.8 to 45.7 |
| indirect 2 | 292 | 57.5 / 118.7 | 28 | 68,776 / 131,655 | 673 | 48 | 719 | 39.3 to 46.5 |
| main 2 | 264 | 59.0 / 131.1 | 22 | 69,040 / 120,988 | 666 | | 615 | 40.1 to 45.7 |
| indirect 3 | 199 | 87.7 / 166.8 | 13 | 73,955 / 1,036,848 | 667 | 48 | 475 | 40.4 to 44.5 |
| main 3 | 160 | 115.3 / 168.8 | 8 | 71,181 / 122,806 | 578 | | 348 | 40.6 to 43.4 |

The slow runs are the phone's clock: the third pair of both builds ran held at 475 and 348 to
401 MHz through the drive, while the others ran at 615 to 719, with AP no higher. A frame times at
most 48 passes, of the 256 the query set holds.

Mac Safari ran each build once, with its window in front (two earlier tries behind other windows
ended with no result in 240 s). Main: 452 frames, GPU p50 / p95 23.1 / 33.9, holes at the median
50,810. Indirect: 685 frames, 22.2 / 38.6, 47,510. Both read every span the drive runs, and the
same ones read nothing.

### What calls the tail, and whether a user meets it

A worktree of `ac321a51af`, with `tool/probe.patch`, undone after it, recorded in each frame the terrain reads
started, the orbit's distance from `controller::dist` and whether the frame began the
light's epoch. The drive is unchanged. Three paths ran beside it, from the same 20 s
window: `pan`, a left drag at 30 px/s at the default distance; `walk`, the character
walking; `zoomin`, one zoom in over 3 s from the farthest, then still. One page of the
release build on port 4174. A tail frame is one with `light_hit` of 20 ms or more.

| run | AP, °C | frames | GPU p50 / p95 | `light_hit` p50 / p95 / max | tail frames |
| --- | --- | --- | --- | --- | --- |
| core-drive-1 | 40.0 to 44.3 | 156 | 111 / 219 | 3.3 / 45.2 / 113.8 | 13 |
| core-drive-2 | 39.9 to 43.9 | 156 | 110 / 244 | 3.5 / 46.3 / 97.4 | 16 |
| core-pan-1 | 40.3 to 44.8 | 293 | 56 / 64 | 0.3 / 0.3 / 110.6 | 1 |
| core-pan-2 | 39.8 to 46.2 | 478 | 33 / 37 | 0.1 / 0.2 / 63.3 | 1 |
| core-walk-1 | 40.7 to 45.3 | 340 | 43 / 91 | 0.3 / 12.4 / 203.0 | 2 |
| core-zoomin-1 | 40.5 to 44.1 | 229 | 66 / 117 | 0.3 / 15.9 / 124.8 | 10 |
| core-zoomin-2 | 40.0 to 49.3 | 765 | 14 / 42 | 0.0 / 0.2 / 76.7 | 8 |

The two pan runs and the two zoom-in runs differ in their p50 by a factor of 1.7 and 4.8
on the same path, so these p50s compare nothing.

In the drive the tail goes by the eye's distance. By the frame's distance, drive-1 and
drive-2:

| distance, units | frames | `light_hit` median | tail frames | GPU median | epoch share | terrain reads a frame |
| --- | --- | --- | --- | --- | --- | --- |
| 100 to 170 | 2, 5 | 68.1, 76.9 | 2, 5 | 286, 287 | 1.00, 1.00 | 0, 0 |
| 170 to 300 | 15, 11 | 36.4, 39.5 | 11, 10 | 217, 206 | 0.73, 0.73 | 0.73, 0.45 |
| 300 to 700 | 22, 15 | 3.7, 4.3 | 0, 1 | 118, 122 | 0.82, 1.00 | 0.36, 0.47 |
| 700 to 3,000 | 22, 26 | 3.9, 3.8 | 0, 0 | 118, 116 | 1.00, 1.00 | 0.45, 0.54 |
| 3,000 to 12,000 | 22, 29 | 4.0, 3.5 | 0, 0 | 116, 115 | 1.00, 1.00 | 0.41, 0.52 |
| past 12,000 | 73, 70 | 0.3, 0.3 | 0, 0 | 67, 70 | 0.99, 1.00 | 0.95, 0.77 |

Below 300 units, 28 of 33 frames are tail frames; above, 1 of 279. The reads do not
separate them: within three frames of a tail frame 5.38 and 4.50 terrain reads started,
against 4.59 and 4.13 about any other. The epoch does not: 0.92 and 0.88 of the tail
frames began one, against 0.94 and 0.99 of the rest, since in the drive nearly every frame
does.

A user meets it, in three forms:

- A slow move near the ground has no tail but one frame: `pan` at distance 16 has one
  frame a run of 110.6 and 63.3 ms, `content+receivers`, an epoch, 13 s in. `walk` has one
  of 203 ms and 437 busy, 17 s in, an epoch, and `light_hit` of 12 to 16 ms in about 20
  frames around it, `slide+receivers`, none of them an epoch.
- One zoom in passes through the same distances: `zoomin-1` has 96 to 125 ms on five
  frames between distance 133 and 4, in 1.6 s; `zoomin-2`, whose p50 is a fifth of that,
  has 13 to 71.
- The view at rest at distance 2 keeps having it, with no input, no read and no load:
  `receivers+world` frames, not an epoch, of 123 to 125 ms in `zoomin-1` (6.8, 10.1, 12.5,
  17.1 s) and 69 to 77 ms in `zoomin-2` (2.7, 3.7, 4.4, 7.2, 8.0 s), among 163 and 110 such
  frames that cost little. What the costly ones have that the others do not was not read.

### Why the world changes at rest

`zoomin` again, three runs (`core-rest-1` to `3`, AP 38.7 to 50.4, 40.1 to 47.2 and 39.9
to 46.9), on a worktree of `ac321a51af` with `tool/probe-rest.patch` and the ray counters on, each frame recording what
set the light's `world`, how many columns it named, how many bricks went present or stopped
being so, and the finest clip level among them. Undone after it. A frame's counts are read
in about one frame of two.

The world does not change every few seconds. It changes on about one rest frame of five:
150 of 661, 153 of 667 and 129 of 718, for 7 to 9 s after the zoom in (to 11.3, 11.7 and
9.8 s of the window), and then stops. Every one of them is a clip level's brick going
present; none is a change of the service area, of what is placed or of a preview's cuts.
No brick stops being present. 312, 326 and 317 bricks go present, 30 to 55 a second, at
all the eye's clip levels (`cell_zoom` 15 to 22), with 3 to 7 terrain reads and 9 to 12
loads in the whole rest. It is the clip levels of the new zoom being filled, a share a turn
(`clipmap::fill_share`), not a change made with nothing to show for it.

Few of them cost. 13 of the 432 frames with `world` have `light_hit` of 20 ms or more.

| | frames | `light_hit` median | columns, median / most | holes where counted |
| --- | --- | --- | --- | --- |
| rest-1, costly | 5 | 70.1 | 1 / 3 | 789,491 (1 frame) |
| rest-1, cheap | 145 | 0.1 | 2 / 7 | 0 median, 75,744 at most |
| rest-2, costly | 5 | 69.5 | 3 / 4 | 789,816 median of 4 |
| rest-2, cheap | 148 | 0.1 | 2 / 9 | 0 median, 48,352 at most |
| rest-3, costly | 3 | 71.5 | 2 / 5 | 769,262 and 798,179 |
| rest-3, cheap | 126 | 0.1 | 2 / 10 | 0 median, 132,057 at most |

A costly frame's cost is the holes' walk: about 790,000 holes, 16,000 boxes and 1,100,000
sky, against at most 132,000 holes in the cheap ones, with as few columns. In each run the
costly frames rise through the clip levels with time: 18, 19, 20, 21, 21; 17, 18, 20, 21,
21; 20, 21, 21.

## Not measured

- Why the S23's Chrome loses an indirect draw in a render pass that writes an end alone, within
  Chrome, Dawn or the phone's driver.
- The GPU's clock in the five first S23 runs of each build, whose two slow indirect runs came
  before the clock was read.
- What holds the S23's GPU at 348 to 475 MHz in some runs with AP no higher than in the others.
- Whether the loss is a regression. The light's raster is in the tree before 11-07
  (`de4624772e`, 11-06), and no commit before `ac321a51af` was measured.
- `assembleMobileRelease` at `ac321a51af` fails in `xpute/ts/native/script/number-paths.py`,
  whose source roots do not take cargo's `/git/checkouts/`, while wgpu comes from a git fork.
- What the fall over the casts after the terrain comes in is due to.
- What the two exceptions on the S23 have that the other tail frames do not.
- The app; these paths ran in the page.
- Why the map's holes begin below 300 units, and what the map holds there that it does not
  above.
- Why a few bricks of a clip level leave the map with 790,000 holes and the others do not.
