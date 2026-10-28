# The shadow map

**2026-10-26.** With shadows on, the sun's shadow map was the frame's largest
cost while moving: on main at 21f72517b, fixed path
([the cast on a fixed path](./20261026-the-cast-on-a-fixed-path/README.md)), dpr
2, 2056 × 1113, `shadow` p95 60.2 ms and the frame interval p95 216 / max 367
ms, against 77 / 213 with shadows off. The map is cast incrementally: a texel is
walked again where the box moved onto it, where the eye's zoom at its stop
changed, or where its ray passes over a node the world answers otherwise, and
the whole map is cast where the light's frame steps.

This records what was changed, in order, on branch
`experiment/20261026-sun-map`. The numbers are rough: most are a single load,
the path ran 4 extra quarter turns (8 in all) from the beams on, and the counts
and check are summed over the 8–56 frames whose counts were read back.

## How

Dev settings, all off by default: `sun_counts` (20) counts what each cast walks
again and why into the profiler's sun lanes; `sun_check` (21) walks every texel
from the root each frame, untimed, and counts those whose stop, zoom or depth
differ from the map; `sun_beams` (22) is the beams below. The `shadow` span is
split: `sun_mark` (the tiles listed), `sun_beam`, `sun_walk` (the texels
walked), and `shadow` keeps the map's raster of the casters.

## Policy: fewer whole casts

Counted first: the map was cast whole in 8 of 48 frames (more than 32 nodes
moved ×6, a depth step ×4, the sun ×2), 31.5% of texels a frame were walked
again, 36.8 visits a texel, `sun_walk` p95 183 ms.

- The 32-node cap was a uniform's array size. The nodes moved are now a storage
  buffer of 512 (`sun::MOVED`). The mark tests each 8 × 8 tile's rays together
  against every node, and a slot is tested node by node only where 32 or fewer
  moved (`sun::SLOT_NODES`), else walked with its tile.
- A depth step was handled by moving each kept word's depth. That was dropped
  again with the start fix below, which makes a step move every start; a depth
  step casts the map whole, as on main.

After: whole casts 0 of 56 frames, 25.9% walked again, `sun_walk` p95 125.2 ms,
interval 184 / 322. The check found depth differing on 0.09% of texels.

## Depth: where a ray starts

A texel's ray start was worked out each frame from the eye's f64 position, so
its rounding to a Q3 step moved with the eye's fraction, and a kept word's depth
could sit a step off a fresh walk's. Now a texel's ray starts at the same point
in every frame of one light frame (`sun::Starts`): anchors 2048 texels apart on
the world's texel grid, worked out once on the host in f64, each start its
anchor plus f32 steps.

After: depth differs 0.058% (of which a start's step 1), stop 411, zoom 149 of
29.4M texels. `sun_walk` p95 183.8 ms, as a depth step casts whole again.

The walk's 28.0 visits a texel were mostly descent: 85% splits, 43% at zoom
under 12. The observed pyramid held 46% of them; a sample on a 4-texel lattice
met 94.1% of the stops.

## Beams

The eye's beams, for parallel rays (`sun::BEAM`, proof there): a beam ray at
every eighth texel and five past it, walked from the root to a block or a split
narrower than 2√2 spacings (`cast::Detail::beam`), leaving its line; each
8-texel cell starts at the least of them within five spacings, less 2√6
spacings, and its texels resume off that line, the node moved to their own
origin (`cast::Node::moved`).

| `sun_walk` p95     | beams off  | beams on                               |
| ------------------ | ---------- | -------------------------------------- |
| first load         | 187.8 ms   | 30.3 ms + `sun_beam` 7.3               |
| later, 3 + 4 loads | 168–181 ms | about 65 ms (63.5–71.0, one load 33.2) |

The later loads are on the widened keep test below. About 2.7× either way.
Visits fell from 24.8 a texel to 9.9 own and 0.7 shared; about 63k beam rays a
frame, 25 visits each. "Off the beam unlike the root walk" was 0 in every check:
the resumed walk stops where the root walk does.

A first check with beams on showed stop differing on 840k texels; longer runs on
and off showed it was sampling. Every texel that differed was a kept one, none
walked that cast, and depth differed on 0.42–0.64% over the longer rotation: the
keep test let stale words through, in both modes.

## Keep test: widened

Two causes of a stale kept word:

1. **A coarse stop block over a moved node.** A ray stops at a block that can be
   coarser than a moved node and contain it, and that block answers otherwise
   once the node does, though the ray passes beside the node. The test now asks
   whether the ray passes over the node's ancestor at the zoom stored in the
   texel's word, the zoom the eye asks at the stop (`sun::widened`); the mark
   uses the coarsest stored zoom in the tile.
2. **The zooms asked moving with the eye.** `Sun::stands` checks only the zoom
   asked at the stop; whether the walk stops at a block also depends on the
   zooms asked over its nearest and farthest points. Fixing it exactly needs the
   stop block's zoom and range stored per texel, a second map layer.

With the widened test, beams on: depth differs 0.27% (101,818 of 37.7M), stop
1,204, zoom 588. Of the kept texels stopping both ways, "same block asked alike"
14,890 and "another or asked otherwise" 87,577 (85%): cause 2 dominates. "Same
block" counts a block of the same zoom and range at another place too, so it
overstates cause 1. Over a node about 21% of texels.

`sun_walk` with it was 73.4 ms in one load against the 30.3 before it. The later
loads put beams on near 65 ms and showed a load at 33.2 while the world was
still loading, so the 30.3 may be such a load and the widened test's real cost
is not settled.

### Tried and not taken: a test at the stop

Keep the path test at the node's own zoom, and add a test that the stop (its
span along the ray from the stored depth, `stop_span`) lies within the node's
ancestor at the stored zoom; in the mark, the box of the tile's stop points.
`sun_walk` about 66 ms against 73.4 (four loads, 33.2 to 71.0), over a node
18–26%; "same block asked alike" 0.084% of texels in both check loads against
0.039% widened. Neither clearly faster nor exact for cause 1; the branch keeps
the widened test.

## The residual

Cause 2. The user judged the shadows on screen with the widened test and
accepted what remains; the second map layer is not added. Depth differences
swing with it from load to load (0.35% and 1.53% in two check loads of the stop
test), so a rate from one load is not a rate.

## What it showed

- **Whole casts were policy, not need.** Most went with the node cap.
- **The descent is the cost, and beams take most of it**, about 2.7× on
  `sun_walk`, exact against the root walk.
- **The keep test is where the map goes stale.** Cause 1 is exact with the
  widened test; cause 2 is accepted.
- **Still open:** what the widened test costs against the path test alone,
  measured with the world loaded before the path starts.

## The eye's split, every frame

On main (e04a3a855, then 1c5455121 and 403ddd70d). The keep policy is
gone and the map is cast as the eye's rays are, every slot each frame it is
cast:

- The beam as above, always. Then a sample every 4 slots (`sun::SAMPLE`),
  walked off it into the eye's sample words (`sun_sample_cs`,
  `beam::SAMPLE_*`).
- Their boxes are written by the eye's writer (pxl-shader raster.rs `boxed`,
  shared with `boxes_cs`) and drawn by the eye's raster pipeline in an
  orthographic mode off `Sun`'s words. The draw goes in the box's order
  (`Sun::texel`), laid from box texel (0, 0)'s start.
- Every slot takes its pick where its own ray enters it (`sun_cs`, the eye's
  `picked`). The rest are listed by the eye's `listed` into the eye's
  dispatch layout (`ray::MISS_ARGS_*`) and walked apart (`sun_miss_cs`), as
  the eye's `cast_cs` and `miss_list_cs` split.
- The sun's beam rays, samples and missed slots walk the observed pyramid
  as the eye's walks do, in passes of their own, through the eye's lines
  and resume (1c5455121): a node the eye's rays or an earlier frame asked
  at the same zooms is read, not asked again.
- A frame casts the map only where what it reads moved: the light's frame
  or box, the eye's place, quality or zooms, or the world. The eye's look
  alone casts nothing, nor does a standing frame. In the orbit camera a
  turn moves the eye's place, which the zooms its rays ask follow, so a turn
  there casts. Nothing is cast before all its pipelines are made.
- Gone: `sun_mark_cs`, the keep tests (`Sun::kept`, `stands`, `tile_*`,
  `Held`, `widened`, `over`), the moved-node feed, the kept corner, the
  stamps; the `sun_beams` and `sun_check` settings, the check and the cover
  (`sun_check_cs`, `sun_cover_cs`) and their lanes.

What the first cuts showed (rough, single loads):

- **Blocks smaller than the samples.** Asked at the eye's zooms near the
  eye, a block the sun's ray stops at is a texel or less. Each box covered
  about one texel, and 3–5% of texels took a pick. The sun's rays now ask the
  world no finer than `Sun::finest`, the zoom whose block spans
  `BLOCK_SAMPLES` (8) samples. With the world loaded, 88% of texels took a
  pick. The cost is shadow detail under that block size near the eye, which
  the check does not see (it walks at the same zooms).
  **2026-10-27: the cap is gone.** A coarse block cast its shadow on the fine
  ground the eye draws inside it, as gray blobs edged by its faces, the map's
  unit no longer the eye's. Counted with `ray_counts`, panning: on the start
  view 68.6% of texels took a pick (99.2% capped), the boxes 156k a cast
  (24k), the walks' nodes 11.2M (0.34M), `sun_walk` p95 about 28 ms (0.7);
  at mid altitude 78.4% (17.7% capped, its texels meeting nothing walked
  too), 10.4M (3.55M). The 3–5% above was not seen again. What is left
  walking is near the eye, where a block the eye asks is a sample's spacing
  or less.
- **A frame mismatch.** The boxes had been laid from the cast's Q3 eye and
  the uniform's `first` from the renderer's light frame.
- **Correctness.** Against a walk under the same rule, measured with the
  check before it was taken out: stop differs on 65 of 33.5M texels, depth
  on 0.38%.
- **Speed.** Before the miss split, not a gain: the 12% left walking sat in
  nearly every 8 × 8 workgroup, `sun_walk` p95 28–48 ms, `sun_sample` 3–7,
  `sun_raster` 1–3, and every moving frame cast the whole map where the
  kept map cast about a quarter of it. With the misses walked apart and the
  sun's walks on the pyramid, on the start view 96.9% of slots took the
  pick and each of `sun_beam`, `sun_sample`, `sun_raster`, `sun_pick` and
  `sun_walk` was under 1.5 ms at p95 (1c5455121's single load). The user
  judges it smooth while moving; it was not measured on the fixed path.
- **Against 9/26 (e78eeec55), read off the code.** That shadow pass drew the
  chunk meshes' boxes, which the mesher had built once per chunk, again from
  the light. It paid no ray. Here the boxes are found each frame by the
  sun's own rays, since meshers, runs and frontiers are not used. The
  raster draw costs what 9/26's did; the finding is the cost.
