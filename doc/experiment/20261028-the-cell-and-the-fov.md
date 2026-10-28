# The cell and the FOV

**2026-10-28.** E8: how the view looks with a wider vertical FOV and with a
coarser camera grid cell, seen side by side. Exports and ports only, at the
user's word through pixelet-81.

## Why

The vertical FOV is 50°, and the camera grid's cell is 1/96 rad at the view's
τ of 2, about 12 CSS px at the reference view (1080 CSS px high).

At 53.13°, tan(half) is 1/2, so one CSS px is exactly 1/1080 rad at the
reference view. A 16 px cell would be `SPLIT` 135 at the same τ.

The question was whether either reads better: block size and chunkiness at
the same place and pose, out to the horizon, and where decor turns to
sprites.

## How

Three exports of `0380ba7ba` (`4679ea4d4` and a record):

- **:5176 A:** as it is: 50°, `SPLIT` 192.
- **:5177 B:** 53.13010235415598° in pxl-kernel view/param.rs `FOV` and
  pxl-spec camera/grid.rs `REFERENCE_FOV_DEG`, and `SPLIT` = 67.5 ·
  `RATIO_MAX` = 135 in pxl-spec-core grid.rs. Its shaders were made again.
- **:5178 C:** the same FOV, `SPLIT` kept at 192.

What `SPLIT` sizes followed it in the export:

- `window::RADIUS` and `ACROSS`. `ACROSS` rounds up to a power of two by
  itself, so nothing assumes one of `SPLIT`.
- `cell_at`, `beam::spacing`, decor's sprite edge and the clouds' thinning.
- pxl-spec `split_distance` and `target_cell_px`.

pxl-spec-core's tests passed on B. The kernel decor test
`decor_s_edges_are_angles_the_reference_view_draws_as_set` failed on B (16 px
for the model edge) and would on C (11.25 px). It writes the 12 px cell as a
literal.

## What was seen

The camera grid's cells, from `beam::Grid::of`'s spans:

| | FOV | `SPLIT` | cell at the reference view | cells at 2056×1113 | cells at 1920×1080 |
|---|---|---|---|---|---|
| A | 50° | 192 | 12.06 CSS px | 166 × 90 = 14,940 | 14,400 |
| B | 53.13° | 135 | 16.0 CSS px | 125 × 68 = 8,500 (0.569×) | 8,160 |
| C | 53.13° | 192 | 11.25 CSS px | 178 × 96 = 17,088 (1.14×) | 16,416 |

B's 0.569× matches the predicted 0.57×.

The visits per lane are left out. They were not counted at a held place and
size: the browser's location failed now and then and fell back to a default
place, and the window was resized during the runs.

**The user's look:** B, at 16 CSS px a cell, is too coarse. A or C is
better.

## What it says

The cell stays near 12 CSS px. The FOV's value, 50° against 53.13° with
`SPLIT` at 192, is held until the knobs the view is made of are gathered in
one place. Today they sit apart:

- the cell size is hidden in `SPLIT`'s 96;
- the FOV is written twice (kernel `FOV` and pxl-spec `REFERENCE_FOV_DEG`);
- a test writes the 12 px cell as a literal.
