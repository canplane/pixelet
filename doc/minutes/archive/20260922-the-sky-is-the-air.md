# The sky is the air

**2026-09-22** — a design conversation and the building of it, the same day.
What is still open is at the end, under "Where this stands".

The sky, the haze over the ground, the color of the sunlight, the night and the
clouds are drawn from one model of the air, over one sphere: Web Mercator's own,
which the flat map is now laid back on about its center. Before this, each was a
picture of its own. The sky was the three.js dome shader's three-color gradient,
keyed to an hourly theme; the haze was one flat fog color mixed in by depth; the
sunlight was the theme's directional color; the stars were grid cells lit whole,
which drew as grey rectangles; the clouds were value noise painted on the dome;
and the world was flat to the edge of the fog. None of them knew about the
others, so a sunset reddened the sky and not the light, and a view from high up
saw the flat ground overshoot a horizon it had no horizon of its own to meet.

## The air

The model is Bruneton's and Hillaire's ("A Scalable and Production Ready Sky and
Atmosphere Rendering Technique", EGSR 2020), with the parameters Unreal's
SkyAtmosphere takes as the Earth: Rayleigh scattering over 8 km, Mie over 1.2
km, an ozone tent peaking at 25 km. The numbers live in one place,
`crates/pxl/web/render/src/look/atmosphere.rs`, and reach the GPU through a
uniform, so no shader carries a copy.

The GPU draws the air into small tables (`gpu/atmosphere.rs`), each a
full-screen pass into four half floats:

| table         | size      | drawn                                                  |
| ------------- | --------- | ------------------------------------------------------ |
| transmittance | 256 × 64  | once for a device                                      |
| multiscatter  | 32 × 32   | once, after it                                         |
| sky view      | 192 × 108 | when the sun's height or the camera's altitude moves   |
| moonlit sky   | 192 × 108 | the same, for the moon, and only while it lights a sky |
| exposure      | 1 × 1     | after the sky view                                     |

The sky pass then reads a table where it used to march or hash, and costs less
than the fbm it replaced. Every lit fragment reads the same tables for the air
between it and the eye (`atmos_view.wgsl`'s `aerial`): the segment's
transmittance by the closed form of an exponential layer along a straight line —
a ratio of two table reads is noise at the lengths a street view has — and the
sky's own light along that ray, in the share the segment is of the whole ray's
extinction.

The sunlight on the terrain is the CPU's: the same model integrated along the
sun's ray, once a frame, to the camera's altitude. It is balanced so a sun
overhead at sea level is white, and scaled so noon lights the ground as the day
theme did (`SUN_LIGHT` 0.5). A low sun reddens it and a set sun takes it away,
which is what the theme's directional color had been standing in for.

The band that hides the terrain's drawn edge is kept, but it fades into the
sky's light along the same ray rather than into a color. Where that ray meets
the planet before the top of the air, the sky pass draws the ground there too —
a flat face lit by the frame's own lights, so the edge is not a seam. What the
terrain does not cover is that ground, and from high up it is what lies between
the drawn terrain and the horizon.

## The exposure

The tables hold radiance for a sun of illuminance 1, and something has to decide
what that is shown at. The terrain is lit on the themes' scale, which is far
brighter than physical light — a horizontal face under the day theme takes its
albedo times about 1.24 (the sun at 0.5 and the fill, ambient 0.68 and
hemisphere 0.82, over π, plus the floor's 0.6), where the air lights the same
face at about 0.28. Their ratio, about four, is `EXPOSURE`: the haze and the
ground seen through it on the terrain's scale. The first value tried, 12,
drowned the ground past the terrain in white.

A fixed exposure turns a setting sun's sky black, because the sky's light really
does fall by fifty times while the terrain, theme-lit, does not. The eye adapts,
and so does the frame now: the exposure pass integrates the horizontal
illuminance — the sky view over the upper hemisphere and the sun through the air
— against a sun overhead at sea level, which the white balance makes exactly 1,
and raises the exposure by the square root of the fall, up to six stops.
Following the fall fully washed a 3° sun's sky out white, since the illuminance
falls with the direct sun long before the sky dims; half, as a camera's metering
and an eye both leave a dim scene dimmer, kept its color.

## The night

Past the six stops the sun's sky is black, and the moon lights it instead, from
a table of its own kept relative to its azimuth. How bright is a choice rather
than a measurement: the moon's light is some four hundred thousandth of the
sun's and an eye adapts to it by rods, which no screen can show, so a full
moon's sky is drawn at a share of the day's (`MOONLIT_SKY` 0.15, where 0.05 left
a gibbous moon's sky black). Its phase follows the magnitude law in Allen's
Astrophysical Quantities, which gives a half moon a tenth of the full one's
light. With no moon up the night is dark, and the themes lost the sky and fog
colors that painted it navy — a theme is now the fill the terrain is lit by and
whether it is night, and nothing about the sky.

The stars are the Yale Bright Star Catalog (Hoffleit and Warren, 1991, CDS
V/50): 9,096 stars to about sixth magnitude, eight bytes each, 72,768 bytes
baked by `script/bake-stars.ts` and drawn as one instanced strip. They are
turned by the sidereal time and the latitude through the same construction that
places the sun and the moon (`celestial_position.rs`'s `equatorial_to_local`,
tested against it at two cities). Brightness is the magnitude, color the
temperature from B−V through Planck's law, dimmed by the airmass. Their size is
in CSS pixels — in device pixels a dense screen halved them — and a star
brighter than first magnitude carries a faint halo, which is how an eight-bit
screen says one white star outshines another.

## The globe

The fog was 1,536 u, and at that the ground a street view saw ended in a grey
band. The window can reach further: with its finest at the leaves for decoration
and five zooms, its coarsest is z12, which draws step 2⁵, wanted out to 192 · 2⁵
= 6,144 u. That is `FOG_FAR` now. It costs no memory — the window's 125 records
are placed at boot — only a ring of pages and a constant number of chunks for
each octave added.

At that reach the flat world shows. Laying it on a sphere by lowering each point
by its sagitta was the first step, and was not enough: zoomed out, the terrain
slid against the horizon, first under it and then over. Two things were wrong.
Web Mercator is stretched north by the secant of the latitude, so distances on
the flat map are not the planet's; and the air held the camera's altitude inside
the atmosphere, so above 100 km the sky's horizon stayed where it is from 100 km
while the ground went on curving away.

Now every point is taken back to its latitude and longitude and set down on the
sphere in the map center's east-north-up frame (`look/globe.rs`, and
`frame.wgsl`'s `on_globe` for the GPU) — Mapbox's globe, which reprojects the
same Mercator tiles in its vertex stage. The sphere is Web Mercator's own,
`EARTH_RADIUS` from pxl-spec's EPSG:4326 adapter, which is what makes the map
center's neighborhood the identity and leaves picking where it was; the air is
drawn over the same sphere, from space when the camera is above it. The chunks'
culling and the camera's altitude over the planet use the same function on the
CPU.

The first build of it tore a character in two along the line through the map
center. WGSL holds `sin` and `atan` to an absolute error of 2⁻¹¹, so a GPU may
answer `sin(1e-7)` with anything within half a thousandth, and times the
planet's radius in units that is meters. The CPU's f64 tests could not see it;
switching the placement off in the page did. Near the center the angles are
taken as their series now.

## The clouds

A layer over the planet at 1.5 km, curved with it: where a pixel's ray meets the
layer's sphere, the cloud is value noise in kilometers anchored to the ground
and drifting with the wind, lit by the sun and the moon as they reach that
height through the air and by the sky above it, and seen through the air
between. It writes the depth the layer lies at, so a mountain stands in front of
the clouds behind it and, from above, the clouds hide the ground under them;
from orbit the reach grows with the height over the layer.

## Roads not taken

- **Volumetric clouds.** Nubis and its descendants march each pixel 64 to 128
  steps through a 3D noise, and rely on temporal reprojection to afford it.
  Reprojection converges only when every frame is drawn, and this client draws
  at rest every two seconds. A slab a kilometer or two thick, sampled a handful
  of times, would give the side view some volume at several times today's cost,
  and is recorded rather than built.
- **Twinkling stars.** A twinkle is motion; at a frame every two seconds it
  reads as a blink, and drawing every frame for it is the device's cost.
- **An aerial-perspective volume.** Hillaire's 32³ froxels want a compute pass,
  which the renderer has none of. The per-fragment approximation above needs
  none, and is exact where a view is near the ground.
- **The HYG star catalog**, for its CC BY-SA license, when the Bright Star
  Catalog is public.
- **Curvature from a zoom threshold.** Near the camera it is under a millimeter,
  so a switch would only add a pop where it turned on.

## The rest of the day

Three things were done beside the sky, each small enough not to need a document
of its own:

- **A deploy is a branch.** `check-and-deploy.yml` builds and publishes only
  what is pushed to `deploy`, so main can carry work between machines, compiling
  or not (7561b095).
- **A frame allocated some 530 JS objects.** `gpu.ts`'s `setBindGroup` built an
  array a call, every call; it reads its offsets from one view over the memory
  now (c10a01fb), measured in
  [experiment/20260922-what-a-frame-asks-of-webgpu](./experiment/20260922-what-a-frame-asks-of-webgpu.md).
- **zstd 19 against 3 for the bucket**, measured and added to
  [compression by traffic](./20260913-compression-by-traffic.md): 19 stays.

## Where this stands

Built, on main and not yet pushed: 2964b127 (the air), 85fd5349 (the exposure),
31f778a1 (the fog to the window), df12c155 (the first curvature), 562639ac (the
stars), 043dd7cb (the moonlit night), fe3e7786 (the clouds), dc89db17 (the
globe), and the ground past the terrain in the land's color rather than a grey.

Not yet seen on a screen: the clouds at sunset, and the moonlit sky toward the
moon itself — the page reloaded or the camera would not turn each time.

Closed since:

- The terrain stood at its elevation and the planet's ground at sea level, so
  from high up a far piece of terrain floated a little over it, its chunk walls
  showing. The ground the sky draws now stands at the elevation under the map
  center (`layer/environment/fog.rs`'s `centre_ground`, into
  `atmos_medium.wgsl`'s `land_altitude_km`).
- `look/lod.rs`'s note on which tier handoffs are dithered read against a
  `RENDER_DIST` of 1,536 u, a constant that then went; it reads against
  `NEAREST_REACH` now, which is the floor under the reach rather than a render
  distance
  ([the ground runs to the horizon](./closed/20260923-the-ground-runs-to-the-horizon.md)).

Open, and queued in [BACKLOG.md](../BACKLOG.md):

- Clouds with a thickness, the slab above, as a quality setting.
- The Milky Way, which a catalog of six thousand naked-eye stars does not make,
  and the bright planets, which the sun's and the moon's formulas extend to.
- The themes' remaining fill — ambient, hemisphere and the floor — could come
  from the sky's irradiance the way the sunlight now comes from the air; the
  floor exists for a phone's night and is the one number that would resist it.
