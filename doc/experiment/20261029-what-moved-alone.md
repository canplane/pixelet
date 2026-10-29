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
region frames under What was seen were at other views. Why the region was not
taken here was not looked into.
