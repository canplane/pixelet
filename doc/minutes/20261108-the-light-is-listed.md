# The light is listed, the eye walks

**2026-11-08** — recorded with the user, after the S23's p95 was traced to the light's rays
and the layers the drawing keeps were checked against the code.

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

**The eye stays ray, raster, ray.** Its holes' walk goes only where its samples are made
dense enough that no visible node falls between them, which is not built.

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

## Open

- The list's size and time on the S23 at a low sun. First `list::light` on the CPU, held to
  `cast::first`, which measures the real list.
