# The light's list, measured and set aside

**2026-11-08** — recorded with the user, after the S23's p95 was traced to the light's rays
and the layers the drawing keeps were checked against the code. The light's list was decided,
measured at its first step the same day, and set aside: "Measured", then "Closed", at the end.
The structure stays as it is.

## What the user decided

**The rays matter for the eye, not for the light.** The eye's rays earn their place twice: a
pixel's zoom follows its distance, and only what is seen costs. The sun and the moon look
along parallel rays, and each tile of the light's box asks one zoom, its band
(`Light::band`). The depth of the first thing each texel meets is what a raster's depth test
gives of the boxes of the band's cells.

**The light's tiles are listed and drawn by the raster.** Each tile's cells at its band, found
by a descent of the query over the volume its texels' rays sweep, are drawn from the light as
boxes; the light's beam, samples and holes' walk go. The list is made each cast from the query
and kept by no frame.

**10-26's "each frame's geometry is found by rays" is the eye's.** The rest of that record
holds for the light as for the eye: the world's one representation is the query, no geometry
is kept per square, chunk or column across frames, and a job goes to what does it better.

**The eye stays ray, raster, ray, and its holes' walk stays.** A hole is a node seen only as
a strip narrower than the samples' spacing, at the edge of what hides it; such a strip may be
as thin as any, so no density of samples finds them all short of a ray a pixel. Denser samples
cost every pixel of the screen, the square of the density, to find what lies along the edges;
the holes' walk costs only the pixels left, about 0.2% of them in a usual frame and 8–28 ms
of a p95 frame on the S23. What it finds the record keeps, and the raster draws it from the
next frame on, so it walks only where something is newly seen. Samples made dense, and a ray
a pixel, were tried before and failed. 11-06's third reading, the eye's samples made denser in
the grazing rows, goes.

## What was weighed

- On the S23, the light's holes' walk (`light_hit`, `light_walk`) is the largest part of a
  p95 frame, 90–200 ms. A list leaves no hole: a texel's ray enters a box exactly where the
  texel's middle falls on its projection, the light being orthographic.
- What a whole cast of the light reads on the Mac's drive: 30–38 k boxes at the median, 1.9 M
  texels cast, 146 k of them walked. A list was put near 100 k cells, the cells the sun does
  not see uncounted ([the journal](../experiment/20261105-the-frame-on-the-s23/journal.md),
  11-08).
- A list kept across frames would be the meshes of 10-12 again under another name; none is.
- The eye and the light share the walk's code and its tests today; the light's list needs its
  own, held to `cast::first` texel by texel.

## Measured: the list, set aside

`list::cells` on the CPU (pxl-engine, not kept: out/cmp/list-step1.patch): a tile's cells at
its band, found by a descent of the query over the parallelepiped its texels' rays sweep, each
split's children within its content box. On the real world's test data, 14 cases of the light,
1,024 tiles of 8 texels a side each, against `cast::first` texel by texel:

| case | cells a tile | nodes asked a tile, listed | nodes asked a tile, 64 walks |
|---|---|---|---|
| about the eye | 13,361 | 17,307 | 2,106 |
| wide, over the clip levels | 594,309 | 707,742 | 1,798 |
| on a bridge | 3,477 | 4,660 | 1,820 |
| on what is placed, cut and hid | 856 | 1,359 | 3,077 |
| straight down | 10,809 | 13,502 | 1,772 |

- **Exact:** in 13 cases every texel took the depth its walk leaves. Past the area's wall
  4,055 of 65,536 differed, the list's nearer; why is not found.
- **Not cheaper:** the list asked 5 to 400 times the nodes the walks did, in all but one case.
  A ray stops at the first thing it meets and walks nothing under the ground; the list takes
  every cell the sweep holds that is not transparent, and the ground is solid as deep as the
  sweep runs.
- **Without what is hidden, still not cheaper.** The user: the ground under the surface is
  not to be counted. Left out, on a quarter of the tiles, a cell at the band whose faces toward
  the light each meet a listed cell: about the eye 6,947 a tile left of 13,579, on a bridge
  2,647 of 3,503, straight down 3,159 of 11,775. What is left at the band is the surface the
  tile's rays sweep along, from a low sun hundreds of cells of it, every one with a face toward
  the light; only what hides it farther along the light leaves it out, which is the walk's own
  test. What is coarser than the band, 2,300 to 2.3 M a tile, is whole cells and lacks under
  the ground or past what is held, a neighbor's test at each one's own zoom not taken.
- **So the rays matter for the light too:** not for its zooms, which it has one of a tile,
  but because only what is seen costs, and the world is a solid volume. 11-06's estimate of a
  list near 100 k cells counted no cell under the surface.
- **What would make it cheaper:** an answer that a node is solid whole, under the ground,
  which 11-06's cell raster put first. Caves, bores and what is placed open the ground below
  its surface, so such an answer takes each of them, and what is left is a front-to-back test
  of what hides what, which is the walk's own work.

**The light keeps its rays.** "Each frame's geometry is found by rays" stands for the eye and
the light alike.

## Closed

**The eye and the light are one observer, by one rule.** The light sees every kind of thing
as the eye does, through the query: the ground, its caves, the ways, what is placed. Neither
is handled apart, and neither takes a path of its own for a kind, as a raster of the ground's
heights from the clip levels would for the light (weighed and not taken: the caves, the bores
and what is placed would each need their own caster, and the ground a reader beside the
query).

**So the structure stays.** The eye and the light alike are rays that find, a raster that
draws what they found, and a walk where a hole is left (ARCHITECTURE.md, Drawing). What the
S23's p95 has left is sought within it.

