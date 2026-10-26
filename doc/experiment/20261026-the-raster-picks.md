# The raster picks

**2026-10-26.** The cast's `pixel` pass hands each pixel the nearest of the
blocks the 4 × 4 samples about it stopped at that its ray enters, and it was
the largest part of the cast ([where the cast spends](./20261026-where-the-cast-spends.md)).
Here the rasterizer makes that choice: the beams and samples run as built,
finding the blocks and writing the observed pyramid, and the blocks they
stopped at are drawn as boxes, the depth test keeping each pixel's nearest.

## How

Dev setting 20 `ray_sample_raster`, span `sraster` (branch
`experiment/20261026-cast-split`). A compute pass lays a box a stopped sample,
one where its left or upper neighbour stopped on the same box; a render pass
draws, per box, the face on each axis turned to the eye, only on the axis the
sample entered by, and the fragment keeps a pixel only where that pixel's own
ray enters the box across that face (the cast's own entry test), so the depth
test keeps the nearest block the ray truly enters. Lacking samples' boxes are
drawn untested, and a pixel whose nearest is one walks, as today. The pixel
pass reads the id. Fixed path, dpr 2, 2056 × 1169.

The first cut grew each face by a drawn pixel and tested only in the pixel
pass: the grown edges won the depth test where the ray does not enter, 20% of
pixels walked, and `pixel` doubled. Testing in the fragment ended that.

## Numbers

| Span     | As built (p95) | Raster picks (p95, two loads) |
| -------- | -------------- | ----------------------------- |
| `sraster`|                | 2.9 / 2.9 ms                  |
| `pixel`  | 34.8 ms        | 11.4 / 11.5 ms                |

The other spans as built. 14–46k boxes a frame of about 0.92M stopped samples.

Against the eye's own walk, in the same frames, with steps on:

| Load              | Pixels picked, raster / 4 × 4 | Unlike the eye, raster / 4 × 4 |
| ----------------- | ----------------------------- | ------------------------------ |
| loading, 25% lack | 75.08% / 75.03%               | 0.04% / 0.03% of stops         |
| warm, no lack     | 99.68% / 99.29%               | 0.90% / 0.76% of stops         |

Every difference from the eye is the eye stopping nearer: a block no sample
stopped at stands in front of the one picked. Of the raster's in the warm
load, 1.18M are the 4 × 4 rule's own, 489k are pixels the 4 × 4 rule walked
(no neighbour's block entered) where the raster took a block from a sample
further off, and 61k another block.

## What it showed

- **The choice costs a third of what it did**: 34.8 ms to 14.3 ms with the
  raster's own, the frame from 20 to 24 fps.
- **A wider choice is a little less right.** The raster's candidates are every
  stopped block on screen, not the 16 about the pixel; where the pixel had
  none of its own and walked, finding a thin block the samples missed, the
  raster now finds a block behind it. About a fifth more pixels differ from the
  eye than under the 4 × 4 rule. Holding the raster to the pixel's own 4 × 4
  samples would give the rule back exactly, at a test a fragment.

## Next: the raster held to the 4 × 4

Taken next, over keeping the wider choice: a fragment is kept only where its
box is one of the blocks the pixel's own 4 × 4 samples stopped at, so the
raster makes the 4 × 4 rule's choice exactly and only its cost changes. The
question is what that test costs a fragment, given that a box stands for a run
of neighbouring samples that stopped on it.

## Held to the 4 × 4, by reading the samples

Each fragment reads the pixel's 16 samples' stops (11 words each) and keeps
only a box one of them stopped on, writing the depth of its entry itself
(a variant the branch does not keep). Exact: picks differ on 191 pixels of 173M (0.00%), and the
raster's and the 4 × 4 rule's words differ from the eye's walk on the same
1.04% of stops. But not cheaper: `sraster` 18.1 ms + `pixel` 16.6 ms against
`pixel` 34.8 ms as built. The reads a fragment and the depth written by the
fragment, which turns early depth testing off, cost what the choice saved.
The test has to be cheaper than reading the samples.

## Held to the 4 × 4, by a mask

No reads a fragment (a variant the branch does not keep): samples merge into a box only within an
aligned 8 × 8 group, each box carrying a mask of the group's samples that
stopped on it, and the fragment keeps it where that mask meets the pixel's
4 × 4; the depth is the rasterized one, so early depth testing stays. Exact
again (picks differ on 116 of 192M, the same 1.00% unlike the eye as the
4 × 4 rule), and dearer still: 75.7k boxes, `sraster` 23.3 ms + `pixel`
17.1 ms. A block spanning several groups is drawn once a group it spans.

## Held to the 4 × 4, where it differs

The wide raster as before, and the pixel pass accepting its pick at once when
the box's sample is in the pixel's 4 × 4, else when one of its 16 samples
stopped the same way, else running the 4 × 4 loop (setting 20 = 2 at 71782ee9d). Exact (picks
differ on 227 of 146M; 0.79% unlike the eye for both). But 87.6% of pixels
take the second test, 16 samples read, and only 11.5% the first: a box's
sample is the top-left of its run, rarely in the pixel's own 4 × 4. At
2056 × 1113, `sraster` 2.5 + `pixel` 27.6 ms against `pixel` 31.8 ms as
built. The loop's cost was reading the 16 samples more than testing them.

## Where it stands

| Choice               | `sraster` + `pixel` | Unlike the eye     |
| -------------------- | ------------------- | ------------------ |
| As built (4 × 4)     | 34.8 ms             | the rule's         |
| Raster, any sample   | 2.9 + 11.4 ms       | about a fifth more |
| Raster, 4 × 4, reads | 18.1 + 16.6 ms      | the rule's         |
| Raster, 4 × 4, mask  | 23.3 + 17.1 ms      | the rule's         |
| Raster, else 4 × 4   | 2.5 + 27.6 ms (31.8)| the rule's         |

The saving comes from the wider choice itself: held to the rule exactly, the
raster costs what the loop did. In the exact cuts `pixel` also walks more
(0.9% of pixels with no pick, against 0.3%), and a walk costs its warp.

## Taken: the wide choice

The raster, any sample, is the cast now (cast-raster-pick): the 4 × 4 loop is
gone from `cast_cs`, which reads the raster's pick, and the setting, the hybrid
and the check with it. It is a design decision on these measurements, not a
rule for every pass. It was chosen because the parts divide where the costs
are: the shared discovery, the beams and samples writing the observed pyramid,
is cheap when a cell's pixels share it; the per-pixel 4 × 4 lookup was the
costly part, and the raster's depth test replaces it; and the boxes are derived
from the samples, with no chunk, mesh or frontier structure beside them. The
World Query still owns what a block is: the raster only orders the samples'
answers.

Side by side the two show no visible difference, and at 2056 × 1113 the wide
raster's `sraster` + `pixel` cost 12.8 ms against `pixel` 31.8 ms as built. The
price is the pixels' words unlike the eye's walk on 0.90% of stops against the
4 × 4 rule's 0.76% ("Numbers" above).

The shadows and the occlusion are each measured on their own; reusing the
raster's results there is a candidate, not a requirement. Its depth is where a
pixel's ray enters a sampled box, which is not the final shading depth on a
pixel that walks.
