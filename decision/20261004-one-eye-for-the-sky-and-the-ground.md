# One eye for the sky and the ground

**2026-10-04** — decided and built the same day.

The world is matte and mostly untextured, blocks as plastic is, and its light
should read as daylight rather than a toy's. Until now the sky was lit and
shown physically ([the sky is the air](./20260922-the-sky-is-the-air.md)) while
the ground was lit on a scale of its own: a theme's ambient by the hour, and a
floor of 0.6 of every surface's albedo, by day and by night. The floor was
most of what a lit face showed, so a night read as day, and the planet's
ground past the terrain, floor-lit through a long air, turned the horizon
yellow.

## The light

The ground is lit by what lights the sky, and nothing else: the sun, the
Moon, the sky's irradiance, the second of the two lights, and what the ground
below bounces at the Earth's mean albedo. The themes, their clock, the
sunrise solver and the floor are gone; the location marker's night is the sun
under the horizon, as the stars' is.

At night the Moon and a moonless sky's glow are lights of their own, at
values the night eye reads rather than photometric ones (`look/atmosphere.rs`).
The glow lights the ground and is not drawn, so space stays black.

Lit by the sun's direction alone, a matte wall turned from it fell to a fifth
of the ground beside it. A quarter of the light keeps its direction; the
rest, what an up-facing face takes, reaches every face alike (`shade.wgsl`
`DIRECTED`). An up-facing face is lit as before, so the exposure, the sky and
the far ground are untouched. No face is painted by its direction.

## The eye

The sky and the ground are shown through one exposure (`EXPOSURE`, adapted up
to `MAX_ADAPTATION` once the sun is gone) and one tone curve: ACES as Stephen
Hill fit it, the filmic curve Unreal draws with, which takes the color out of
what is bright. The grade the client carried from three.js's postprocessing,
saturation and contrast raised, is gone, and with it the sky's own shoulder.

## Occlusion and shadow

Corner occlusion and the screen-space one scale the light a surface takes,
the sun's included: no shadow is drawn by default, and the corner is the cue
it leaves. Neither touches the air in front, and neither tints.

The sun's shadow map stays a development setting: it costs a geometry pass a
phone does not have to spare, and a matte world reads fine without it. Where
it is on, it is pushed back by its slope to the sun (`record.rs`
`depth_only`), which took the diagonal stripes off faces the sun grazes.

## What it cost

Nothing a phone pays for by default. The tone curve replaced a grade that
converted to sRGB and back; the exposure is read once a shader; the Moon is
not worked out by day.
