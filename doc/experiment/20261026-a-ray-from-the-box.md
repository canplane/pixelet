# A ray from the box

**2026-10-26.** Whether a pixel's ray started at the front-most box a raster
pass hands it beats the cast as built, measured end to end
([a proxy only starts the ray](../minutes/20261024-a-proxy-only-starts-the-ray.md)).
The boxes, their rule and their pass are
[a box before the ray](./20261026-a-box-before-the-ray/README.md); this is the
ray's side and the whole. It does not: the walk from a box is exact and cheap
when the observed pyramid is warm, and the pyramid is warm only because the
beams and samples write it a cell at a time.

## How

The fixed path of
[the cast on a fixed path](./20261026-the-cast-on-a-fixed-path/README.md) (dpr 2,
`resolution` fixed, the dev build), one load a row unless said, the cast as
built at 76–83 ms p95 in every build measured. Settings, all dev-only: 20
`ray_oracle`, 21 `ray_proxy` (5 is the raster at 2 coarser), 22 `ray_proxied`.
With steps on, a check walks every pixel again from the eye through the
world's query and counts where the stop differs.

## Numbers

**The oracle**: the cast as built, then each stopped pixel walked again (span
`oracle`), so the start is perfect and the pyramid this frame's own.

| Start                                      | oracle p95 | Visits a stop | Differs from |
| ------------------------------------------ | ---------- | ------------- | ------------ |
| The root, just short of the hit (358dee10d) | 36.7 ms    | 25.0          | 0.0018%, the cast |
| The box at 1 coarser (394bdfa90)           | 21.5 ms    | 2.2 + 23 to find the box's entry | 0.69%, the cast |
| Finding the box alone (f4f0b5301)          | 16.1 ms    |               |              |
| The box chosen off the eye's own stop (15f5f3c3a) |     | 4.5           | **0**, the eye's walk |

The box walk is exact once the box is the right one; its differences were the
cast's samples' own (0.44% of all stops differ from the eye's walk) and boxes
chosen off a wrong sample's stop. A raster's front-most box cannot be the
wrong one, by the box's proof.

**The proxied cast**: the samples gone, each pixel walking from its raster box.

| Build      | Mode                                   | cast p95                | Steps                                          |
| ---------- | -------------------------------------- | ----------------------- | ---------------------------------------------- |
| dd8945d9c  | walk from the box, misses from the eye | 1169.8 ms               | 0 differ, 0 started past, 60% held over the box |
| dff8037e8  | misses from the box, its own span      | 72.5 + 214.8 ms         | 60% walked again                                |
| 1665b378b  | `read`: the box's line held            | 62.7 + 244.6 ms         | 53% walked again, 0 differ, 22,792 holes        |
| 1665b378b  | `write`: one Writing walk a pixel      | 254.1 ms                |                                                 |
| 16644a89b  | `beam`: beams and cells as built, no samples, each pixel resumed from its cell's line at its box | 177.9 ms | |

The producer by then cost 1–10 ms for its cut and 0.5–2 ms for the raster,
3–7k boxes a frame; it was never the larger part.

## What it showed

- **The walk was never the cost; the pyramid's writer was.** A pixel reads the
  observed pyramid, whose entry answers only at the zooms the frame asks of
  that node (`observed.rs` `holds`, `cast.rs` `asked`), and a moving eye asks
  others every frame. The cast as built pays that rewrite once a beam or
  sample, shared by a cell's pixels; any per-pixel writer, a miss walked again
  or a Writing walk, paid 200–250 ms.
- **Keeping the beams to warm it does not help**: the cells' beams and every
  pixel's own walk together, `beam`, cost more than the samples they replace.
- **The holes are open.** In `read`, 22,792 pixels over 11 frames show sky
  where the eye's walk stops: no box covered them. Not traced.
- **A frame failed whole three times without a word on the page**: a WGSL
  float past f32's range, the stream overflowed by shader sources made in one
  turn, and a bind group naming bindings naga had dropped. The kernel's own
  errors reach `ts/app/web/.dev-console.log`, not the page's console.

## Where the code is

Not on main, whose code is as before the experiment. Every commit named here,
and the tree with every mode built behind the dev settings above, is on the
branch `experiment/20261026-proxy`.
