# The ground runs to the horizon

**Measured and built 09-23.** The distance fog is not fog. It hides where
the drawn ground ends, and on a planet that edge has a natural place: the
horizon, which is finite and set by the camera's height alone. Drawing that
far was tried and timed. It is affordable at street level and at altitude,
and it is not at the zoom in between, for a reason that is about the meshes
and not about the distance.

## What the band is for

Two things dim the distance, and only one of them is physics.

- **The air.** Aerial perspective, from the atmosphere's own tables: a
  segment's transmittance dims a color and the light scattered along it
  fills the color back in (`pxl-render frame/shader/atmos_view.wgsl`,
  `veil`). It is measured in kilometers of real air, so it changes with the
  zoom because a world unit is a different number of kilometers at each.
- **The band.** A smoothstep from `FOG_NEAR` to `FOG_FAR` on the distance
  from the eye, mixing the surface into what the sky pass draws past it
  along the same ray (`shade.wgsl`). `FOG_FAR` is `RENDER_DIST`, scaled up
  with the camera past 4,096 u (`pxl-kernel layer/environment/fog.rs`).

The air alone cannot hide the edge. Horizontally at sea level the optical
depth over 6 km — where the band ends at street level — leaves about 0.8 of
the blue and 0.94 of the red, and looking down from altitude the air is
thinner still. What lies past the edge is the sky pass's ground: a smooth
sphere in the land palette's color, with no relief and no buildings. Take
the band away and that difference is the seam. The band is a crossfade into
it, not weather.

## The horizon is a height, not a tilt

On a flat world a view laid along the ground reaches forever, which is why a
render distance had to end it. The map is drawn on Web Mercator's sphere
(`pxl-render look/globe.rs`), and there the ground curves away at

    d = √(2Rh + h²)

with `h` the camera's height and `R` the sphere's radius in the map center's
units — `MERCATOR_RADIUS_U / cos φ`, 3,365,684 u at Seoul. The tilt decides
whether the horizon is on screen, not how far it is. So the edge the band
hides can be put where the planet already ends:

| camera           | height  | horizon   | `fog_far` today |
| ---------------- | ------- | --------- | --------------- |
| dist 200, 75°    | 52 u    | 18,700 u  | 6,144 u         |
| dist 2,000, 75°  | 518 u   | 59,000 u  | 6,144 u         |
| dist 8,000, 80°  | 1,389 u | 96,700 u  | 12,000 u        |
| dist 32,768, 85° | 2,856 u | 138,700 u | 49,152 u        |

This is the shape virtual-globe engines take: the ground is drawn to the
horizon, the horizon is what culls, and the distance is dressed by
atmospheric scattering rather than by a band. Cozzi and Ring's _3D Engine
Design for Virtual Globes_ (2011) is the reference for horizon culling and
for the depth precision that a far plane this distant needs; the ring
structure we already draw with is Losasso and Hoppe's geometry clipmap
(2004), with Ulrich's chunked LOD (2002) beside it. Minecraft is the other
tradition — a flat world, a render distance, fog at its edge — and that is
the model the band came from.

## What it costs

Measured 2026-09-23 on the development machine: Chrome, a 9.2 Mpx canvas on
a 120 Hz display (8.3 ms a frame), tiles served from the local bake, the
camera placed exactly and held awake by a nudge under a pixel, one 6-second
window a point and one run each — not averaged. **A** is the band as it
ships, a window of 5 levels. **B** pushes `fog_far` to the horizon above and
widens the window to 8 levels, which is what the reach then asks for. GPU
time is the world pass's own, which the ledger now times apart from the
frame's other passes.

| camera           | A gpu p50/p95 | B gpu p50/p95 | A tris | B tris | A dt p95 | B dt p95 |
| ---------------- | ------------- | ------------- | ------ | ------ | -------- | -------- |
| dist 200, 75°    | 12.5 / 13.9   | 16.7 / 17.5   | 4.26 M | 5.38 M | 17.3     | 23.9     |
| dist 2,000, 75°  | 7.5 / 13.7    | 27.8 / 38.1   | 4.28 M | 8.20 M | 16.3     | 56.8     |
| dist 8,000, 80°  | 19.0 / 29.6   | 26.0 / 29.7   | 5.66 M | 8.10 M | 54.3     | 34.9     |
| dist 32,768, 85° | 29.7 / 40.8   | 32.0 / 34.1   | 10.6 M | 11.2 M | 65.3     | 38.9     |

It looks right. At altitude the ground runs to the limb with no cut, and at
street level the land fills out to a horizon of hills.

**C** is what was built from all this, measured the same way that evening:
the haze, the reach read off it and the planet, ground past the horizon
culled, and the outermost ring drawn as coarse as the ladder says. The
window is 5 levels, as it ships.

| camera           | A gpu p95 | C gpu p95 | A tris | C tris | A dt p95 | C dt p95 |
| ---------------- | --------- | --------- | ------ | ------ | -------- | -------- |
| dist 200, 75°    | 13.9      | 14.2      | 4.26 M | 2.88 M | 17.3     | 17.1     |
| dist 2,000, 75°  | 13.7      | 17.9      | 4.28 M | 4.46 M | 16.3     | 24.7     |
| dist 8,000, 80°  | 29.6      | 19.9      | 5.66 M | 6.08 M | 54.3     | 25.8     |
| dist 32,768, 85° | 40.8      | 21.6      | 10.6 M | 6.82 M | 65.3     | 33.7     |

The far views, which were the worst, are now the ones that changed most:
half the triangles and half the frame time at full zoom-out, while the
ground goes further than it did. The middle zoom costs 4 ms more, and that
is the reach being honest — from 980 m up the air leaves 77 km, where the
band used to end at 12.

Memory pays for the wider window: `.fixed.terrain` 125.3 → 200.5 MiB, and
the mesh arena on the device 206.9 → 287.9 MiB.

## What realism would say

A distance is the wrong thing to hold the haze at, but haze itself is not
wrong: on Earth the far ground goes long before the geometry does, and what
takes it is the aerosol near the surface. A world unit is 1.89 m where the
map is (`km_per_u`, the projection's meter scaled by the latitude's cosine),
so the horizons above are 35 km at street level and 112 km from 980 m up. A
clear day's visual range is 30 to 50 km — Koschmieder's `V = 3.912/β`, with
β about 0.1/km clear and 0.5/km hazy.

The air we draw is far cleaner than that. Bruneton's clear-sky values stand
unchanged (`pxl-render look/atmosphere.rs`): Mie extinction 4.44e-3/km, a
horizontal total near 0.022/km, a visual range of 179 km — better than the
clearest day anyone sees. That is why the ground stays crisp to the horizon
and why a band had to hide the edge at all.

So the realistic move is not to push the band out but to give the surface
its aerosol. Then the ground fades on its own where Earth's does, and from
altitude it does not, because a view from above passes through less of the
layer — the 1.2 km scale height is already in the model. It also turns the
reach into something derived: where transmittance along the ray falls under
a threshold, a ring contributes nothing and need not be drawn, which is
exactly the middle zoom that failed above. The one caution is that scaling
Bruneton's Mie twenty-fold whitens the whole sky; the haze wants a medium of
its own, read by the aerial perspective, as engines that do this keep it.

## What the numbers say

- **At altitude it is free, or better.** The far views cost the same GPU time
  or less and paced better, because what the band was cutting there was
  already coarse ground a few pixels tall.
- **The middle zoom is the case that fails.** 13.7 → 38.1 ms for 1.9× the
  triangles. At 518 u up the horizon is 59,000 u away, and reaching it means
  eight rings rather than three.
- **The cost is linear in levels, by construction.** A clipmap ring is a
  fixed 3×3 of pages whatever its level, so each level added draws about as
  many chunks as the last: at dist 2,000, B's per-level triangles ran 2.36 M,
  2.37 M, 0.76 M, 0.55 M, 1.10 M, 0.52 M, 0.47 M, 0.07 M. Distance does not
  make a ring cheaper.
- **So the meshes are what has to get coarser, not the rings.** A box is 36
  vertices and 12 triangles whether it covers one cell or a thousand, and a
  coarse level's boxes land on a few pixels at the horizon, where the cost is
  vertices and quad overshading rather than fill. A ring three levels out
  should be drawing far fewer, far larger boxes than it does.
- **What a vertex does is a tenth of the pass; how many there are is the
  rest.** The air a vertex takes — four table reads and a dozen
  exponentials, `air_at_vertices` in gpu/renderer.rs — was measured by
  drawing it and not: a median 6.9 ms against 5.0 at the same 966 k chunk
  triangles and the same fill, the samples overlapping (6.2–7.1 against
  4.6–7.1), and at street level 8.9 ns a thousand triangles against 8.7,
  which is nothing. The same pass runs 3.9 → 6.9 ms when 597 k triangles
  become 966 k. So an aerial-perspective froxel LUT, which would ride on
  those same vertices, is not the move; the vertices are.

## Where it stands

The band stays until the far rings are cheap, and the order is:

1. **A coarser tier per level**, so a ring's mesh shrinks with its distance.
   Half done: the outermost ring now follows the ladder as far as it goes,
   since no level over it holds that ground (`relative` in
   world/chunk/table.rs), which is where most of C's saving came from. The
   rings under it are still drawn one step off their own cells, because the
   ring above is what holds anything coarser —
   [a coarse cell is isotropic](./20260923-a-coarse-cell-is-isotropic.md) is
   the rule those tiers are built from.
2. **The surface's aerosol** — built 09-23 (`look/atmosphere.rs`'s
   `HAZE_VISUAL_RANGE_KM`, 40 km, and the layer it sits in; `atmos_view.wgsl`
   puts it in front of the medium rather than inside it, since the medium's
   own scattered term divides by what the whole ray extinguishes and
   saturates from space). What is left is reading the reach off it.
3. **Horizon culling** — built 09-23 (`measure` in world/chunk/table.rs):
   a chunk is drawn while `E·P >= r²`, the eye and the chunk taken from the
   planet's center, widened by the chunk's own radius. It is correctness as
   much as cost — nothing draws the planet, so ground past the horizon would
   come up under the horizon line with no occluder to hide it.
4. **The band** — built 09-23: demoted from a render distance to a crossfade
   at the nearer of the reach and what the rings draw (`band_at` in
   layer/environment/fog.rs). The window's top, the clip planes and the
   puck's visibility read the reach now, and `FOG_PER_DIST` and
   `RENDER_DIST` are gone from the spec; FAR_CLIP, FOG_NEAR and FOG_FAR are
   left as the ratios they always were.

What is left is the rings under the outermost one and the window's width:
B's eight levels are still what drawing to the whole horizon at a middle
zoom would take, and they are worth having only once a ring's mesh is
cheap. The B experiment itself was a throwaway — `fog_far` returned the
horizon, `fog_near` and `far_clip` followed it, `WINDOW_LEVELS` went to 8 —
and nothing of it is in the tree.

One thing the work turned up: asking for a coarse tier overran the two
slots a chunk's wanted tiers are held in, because until now a chunk only
ever wanted steps 1 and 2 (`ask`). The kernel's own test drives a view from
high up for that reason (apps/web/src/kernel/kernel.test.ts).
