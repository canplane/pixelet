# What moved, drawn alone

**2026-10-29.** What a frame costs while the view stands and only what the
kernel animates moves (the actors' clips, the puck, the clouds), and what
drawing again only where they moved buys. Built in `4156f25eb` (an actor's
clip stepped a seen step at a time), `cd7f62b68` (a frame that draws a region
alone) and `07d4fa3d8` (the sun's map and the occlusion in such a frame).

## Why

With the resolve in ([the frames gathered](./20261029-the-frames-gathered.md)),
the idle page drew every frame as `px`: the actor's idle clip and the cars
kept the frames coming, each moved the jitter on, and so each cast every
pixel again and resolved the whole screen. The rule the user set: where only
what the kernel animates moved, draw again only what it touches, and work
out nothing else again.

## How

Each build on its own port (:5184, :5185, :5186), fresh or reloaded, Chrome
("Chrome Tahoe"), one tab in the foreground. The profiler's report over an
idle minute: frames by kind and wake, GPU p50, spans' p95. The view the page
opens at; settings switched on the port's own origin, never :5173's.

Where the frames kept coming at vsync, each source of wakes was switched off
in turn on a temporary build (none kept): the resolve's asking for frames,
the actor's wake, the puck's bob's.

## What was seen

**Idle at the display's rate.** Before the region, with the resolve's asking
off, the actor's wake off or the puck's bob's off, the page still drew about
60 frames a second, nearly every one woken by a timer. No one source held it
there; every step of anything animated drew the whole screen again.

**Idle, GPU p50** (3840×1986 canvas; the scale the profiler gave beside):

| shadows, SSAO | before | region frames |
|---|---|---|
| off | 16.0–18.6 ms, every frame `px` (scale 1.00) | 10.6–10.9 ms (`cd7f62b68`); 8.0 ms, 3594 of 3600 frames `region` (scale 0.84) |
| shadows | — | 10.9 ms, 3594 of 3600 `region` (scale 0.84) |
| both | 29.0 ms, no region taken (scale 1.00) | 16.2 ms with a region up to 0.9 of the screen; 13.4 ms, 3473 of 3479 `region`, the map's texels alone and the occlusion standing (scale 1.00) |

With SSAO on, the region first grew by the occlusion's reach and passed half
the screen every frame, so none was taken; the occlusion turned out to read
nothing the kernel animates (its prepass draws the rays and the grown decor
alone), and the region stopped growing for it.

Zoomed in about the actor and the puck after they moved and after the actor
walked: no trace left where they were. An actor half hidden at one point was
a block in front of it, as a frame drawn whole showed.

## What it leaves

- The clouds' drift draws the frame whole, at each of its steps.
- In a `px` region frame the sun's cast still runs (its receivers read the
  jitter): `sun_beam` 1.6–4.3 ms p95, and the sun's map then drawn whole.
  Its passes past the reach go no wider than the tiles it claims since
  `c636c1738` (below); the marks and the reach still run each frame.
- The finish encodes the whole canvas every frame: a canvas's texture is a
  new one each frame.
- The actor's region sphere is what its mesh reaches as skinned at its clip's
  first key, and as far as the clip moves it; no test holds it to a shipped
  model.

## Tried and taken out: the sun's cast left to the jitter's frames

A `px` region frame casts the sun again because its receivers read the
jitter (`sun_receivers`, `gpu/ray/cast.rs`). Read without it, on the reasoning
that a jitter under half a pixel moves the tiles a ray meets by no more than
the tile either side the receivers already read (`sun::READ_TILES`), a frame
that moves the jitter alone casts the sun no more.

On :5184 with shadows on, set to noon from the real time and switched on after
the load, once: inside the region the trees cast no shadow and the ground stood
lighter than outside it, a rectangle on the screen. Not seen again: on a fresh
load, with the shadows switched off and on, and with the hour set from the real
time to noon again. Without the change it was not seen either, though at
another view. Whether the change caused it was not settled, and what it saves
was not measured against the frames before it, so it was taken out; the sun's
cast in a jittered region frame is still open.

## The sun's passes as wide as the tiles claimed

The beam, the cells, the samples, their boxes and the slots were dispatched
over the whole map at every sun cast, and returned per tile where it was not
the cast's to cast (pxl-shader sun.rs `needs`). In a frame that moves only the
jitter, the receivers move within the tiles already cast, the reach claims
none, and every workgroup returned at once. Since `c636c1738` their workgroups
come off a buffer of their own (pxl-spec-core `sun::ARGS_*`): none as a cast
starts, each whole once `sun_reach_cs` claims a tile.

On :5184, the view the page opens at, 4112×2338, shadows on, an idle minute
after a minute's settling, the build before it and the build with it in turn
on the same origin:

| p95 (ms) | `sun_beam` | `sun_sample` | `sun_raster` | `sun_slot` | `sun_walk` | sum |
|---|---|---|---|---|---|---|
| before | 2.4 | 0.9 | 0.2 | 0.3 | 0.1 | 3.9 |
| gated | 2.0 | 0.1 | 0.1 | 0.1 | 0.1 | 2.4 |

GPU p50 31.8 before and 26.9 ms gated, but the spans the change does not touch
(`world`, `resolve`) ran 1.1–1.4× slower in the run before, and the display's
refresh read 16.7 ms there against 8.3; the p50 is not taken as the change's.
`sun_beam` is now the marks and the reach alone.

At this view nearly every frame was `px`, a region taken in 3 of 3507: the
region frames under What was seen were at other views.

## A low sun's shadows bounded too

Why it was not taken there: the sun. That view at the real hour there, about
16:30, has the sun about 10° up, and under a sine of 0.25 (about 14.5°)
`regions` drew the frame whole whenever an actor or the puck moved. With the
shadows off, 3595 of 3599 frames were `region` (GPU p50 6.5 ms); with them on
at noon, 3600 of 3600 (14.1 ms).

The cut taken out (`34611cf20`, only a sun on or under the horizon draws
whole), at the real hour: 3499 of 3565 `region`, GPU p50 18.0 ms against
26.9. At 7 o'clock, the frame drawn by regions alone and the same view drawn
whole after the shadows were switched off and on differed in 2 pixels at a
quarter of the resolution, against 23 between two frames of regions 3 s
apart.

A review found what the cut kept back. The shadow's bound (`SHADOW_RADII`)
takes the ground about level with the caster; ground falling away from the
sun more steeply takes the shadow's end past the region, where it stays until
a frame is drawn whole. How steep shrinks with the sun: about 10° at the cut,
7° at 10° up, 3.4° at 5°. It was so before the cut went, steeper at a higher
sun, and every drop broke it at any height; what the cut did was move the
slope. Only an actor and the puck report a shadow that moves
(`region::drawn`), so it takes one of them moving under a standing view, a low
sun and such a slope, and lasts until the next whole frame: 66 a minute in the
run above, about 6 at the views before. Bounding a slope instead (the bound
for ground falling up to an angle, drawn whole under a sun that low) was
weighed and not built: at 10° it draws whole about where the cut did. What
would bound it exactly is the pixels that read the map's texels redrawn,
found on the GPU, which the region, known on the CPU before the frame, is not.

## The shadow map's light, the moon's at night

The map was cast from the sun alone and darkened the sun's direct light
alone: at night, the sun's light none, it was cast for nothing, and a moving
actor or puck drew every frame whole. Since `d3e4d342e` it is cast from the
sun while it reaches the land (under the horizon by the dip at the eye's
height and its disc's radius, as `sun_transmittance` ends), else from the
moon while it does, else from neither, the shadows off for the frame; the
visibility darkens the light it is cast from (`surface_light_in`,
`SHADOW_MOON`). The light cast's buffers are made off the setting, not off a
light being up, so the frame a light first rises reads no unwritten map.

Seen on a build holding the sky at night: at 23:00 the moon's shadows under
the trees and the cars, a luma difference up to about 20 between shadows on
and off; at 18:30, neither up, no light span ran and 1732 of 1733 frames
were `region`. At 20:00, the moon low, every frame was `px`, drawn whole; not
looked into before the exact bound below.

The map's names say light, not sun (`pxl-spec-core light`, `Light`,
`eye::LIGHT`, `light_*_cs`, the spans `light_beam` to `light_walk`); the sky's
sun keeps its own.

## A jittered ray stands on a tile marked

What the veil was (under "Tried and taken out"): the mark stood each pixel on
the one tile its ray met, and `READ_TILES` covers a receiver's taps, not its
pixel's footprint. A jitter under half a pixel moves a far or grazing pixel's
ray across a tile's edge; in a region frame the region's pixels were cast
again at the new jitter and read tiles no cast had claimed, lit, a rectangle.
Now the mark stands a pixel on every tile between where its ray met the world
and where each pixel beside it did, across, down and either diagonal, each
pair of neighbors once, within `FOOTPRINT_TILES` (past it an edge, each its
own), and the jitter is not read as moving the receivers: a jittered frame
marks and casts the light no more (`light_world` only where the rays were
cast anew). Not run yet.

## A moved shadow bounded by where the world stands

The slope the bound left (above) is gone once the screen's world has landed
since the view last moved. Each drawn frame takes the least and the most of
the world's depth under each square of the screen 32 drawn pixels a side,
the squares it drew (`DepthBounds`, screen_depth_bounds.wgsl), and maps them
back; what landed holds while only the region's draws moved, a frame drawn
whole among them, as the view and the world under it stood as they were. A
moved caster then shades each square whose world, as a sphere about the box
it holds, lies behind the caster from the light and within the caster's
reach of the half-line from it away from the light, grown by a receiver's
taps (`regions`, `stood`): whatever the ground, slope or drop, ground, decor
or an actor alike, and however low the light; until it has landed, the frame
is drawn whole. The texels a caster covers in the map are all of it that
moves while only the casters do, so the light's side needs no more.

A first try kept, per tile of the light's map, the screen pixels its
receivers stood on, taken by atomic maxima in the mark and read back. The
drawn receivers' boxes in it were the tallest decor's sphere, which an
actor standing in its own shadow's tiles carried into every region; near the
eye that covered most of the screen and every frame was drawn whole (3113
of 3411 frames at noon, the region's share past 0.9). It was taken out.

## The history gathered square by square

What lay outside a region never gathered: a frame drawn whole that was not
quiet blends at no less than `BLEND_LEAST`, and while the kernel stepped
every turn every frame after it was a region frame, so the resolve's quiet
count started over each time and only the region was written back. Where a
turn stepped nothing, `moved` was empty, no region was taken, and the
jitter's frame was drawn whole: every third frame dearer with shadows off.
Both are one want, a pixel's age. The resolve now keeps each 32-pixel
square's frames since it last changed (`Resolve::age`): a frame where only
the region's draws moved sets the squares it changed to 0 and ages the rest,
a jitter's frame ages them all, anything else sets them all to 0. A region
is what changed and the squares still short of the jitter's sequence
(`gathering_rect`), and the resolve takes each square's share from its age
(screen_resolve.wgsl `ages`).

Run 2026-10-30, the opening view at noon, shadows and clouds on, 60 s: the
bounds first held only until a frame was drawn whole, and a frame drawn
whole while the squares gathered dropped them again, so 3056 of 3191 frames
went whole for want of them (GPU p50 11.1 ms). Held while only the region's
draws moved, every frame is a region frame: 3601 of 3601, GPU p50 5.9 ms,
p95 8.2 ms (12.1 and 17.7 ms on the line before). No leftover seen.

## The spheres the kernel hands, as drawn

An actor's sphere was as far as its clips reach and move, the one it fades
from too: the companion's person, 1 u tall, 2.07 u idle and 6.38 u
sprinting. It is now as far as its mesh reaches in the pose its palette
holds as drawn (`Actor::pose_reach`, `Model::reach_of`). The puck's was the
zone and the beacon together, its state the beacon's bob, so every bob
moved the zone's footprint; the zone and the beacon are now two, the zone's
state its place and the night alone. Not run yet.

## The canvas finished only where a region moved

The finish encoded the whole canvas every frame, timed with the markers
(`marker`, p95 5.8–7 ms with them at 4112×2338). It has its own span now
(`finish`). The world's is finished into a picture the surface keeps at the
canvas's size (`Surface::shown`), a region frame's only within its region
taken to the canvas's pixels, a pixel more each way (`canvas_rect`), the
screen's depth with it; then shown whole on the canvas by a pass that reads a
texel a pixel (`screen_show.wgsl`), as the canvas's texture is a new one each
frame and WebGPU keeps nothing of the last. After a frame the debug pass drew
into the screen's depth, the next finishes whole. Fusing the resolve with the
finish (two targets) was weighed and not built: the canvas is whole every
frame, so a fused pass is whole too and a region frame would write the whole
history. A copy onto the canvas was weighed and not built: xpute holds the
canvas as a view, not a texture a copy writes. Not run yet.

## The clouds drawn again where they moved

The cloud sheets are planes over the world, not shadows on it; a step of the
wind moved their words, which the frame read, and drew it whole. Now their
words are read apart: where they moved, the region takes where the two
squares the sheets are drawn on fall on the screen, cut at the eye's plane
(`sheets_region`, `plane_clip`); none where none is seen. The cumulus, drawn
as decor is and so a draw no region stood for (`BESIDE`), report each patch
as a region does an actor, a casting sphere in a state of where the wind has
it and what it lists, and the renderer takes the turn's reported draws
(`FrameInput::bounded`) out of what draws a frame whole. Not run yet.

## What follows

The region above is not where this ends. The screen's rectangle, its
squares' ages and the light-plane test against the depth's bounds are
replaced by the screen keeping a word a tile as the light's cast does,
claimed until it has gathered and drawn through a stencil, and the light's
changed tiles reached back to the pixels that read them: [the screen keeps
its tiles as the light does](../minutes/20261029-the-screen-keeps-its-tiles-as-the-light-does.md),
its order as decided on 2026-10-30.

