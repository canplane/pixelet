# A receiver marks what it reads

**2026-10-27** — decided with the user. It records how an effect beside the
eye observes the world, and how the sun's map came to cost what its receivers
read. It is a decision record, not a rule each change is held to.

> One interface, one unit, each effect its own nature. The sun asks the world
> what the eye asks, at the eye's zooms, and passes by a cone of its own; the
> passes it takes are the eye's, written once. Its map is a field in space,
> so it stays a map, cast only where a receiver reads it, and every receiver
> says where that is by the one rule.

## Decided

- **The unit is the eye's.** The sun's rays ask each point at the zoom the
  eye asks it (`cast::Detail::eye`), with no cap of their own; their cone is
  the map's texel (`Sun::detail`). A coarser sun drew a block's column of
  mean height over the fine ground the eye draws inside it, as gray blobs
  edged by its faces (b3b3ee8c8). Pushing the caster's depth back by what it
  is coarser was a patch on two units, and was not taken.
- **One interface, the passes written once.** A cast's passes, the beam
  rays, the cells' starts, the samples and their boxes, a ray's pick or sky,
  the walk again, are one set over an observer (pxl-shader observer.rs
  `Observer`): its lattice, the ray at a point of it with the zooms it asks
  and its cone (`Aim`), whether its rays start apart, and where its boxes are
  laid. The eye (`Eye`) and the sun's map (sun.rs `Map`) are two; an effect
  more implements those and takes the rest (7895fb2ba).
- **Each effect its own nature.** What the eye records is a pixel's ray; what
  the sun records is a field a surface, decor, an impostor and an actor all
  read (pxl-render shadow.wgsl `sun_lit`), so it stays a map, not a ray at
  each of the eye's samples, which only the world's surface would read.
- **Cast only what a receiver reads.** Nothing past the disk a shadow fades
  out by (`Sun::rim`, `Sun::read`; f2e2c6ead). Within it, a receiver marks
  the tile of the box it stands on (`sun::stands_at`): the world where each
  drawn pixel's ray met it (`sun_mark_cs`), decor where each instance placed
  stands and the actors where each does, each the eye sees (`sun_mark_drawn_cs`,
  `eye::sees`, `Sun::ring`). One reach, a tile a thread, reads a tile within
  a tile and its taps of the way up the tallest decor from one stood on
  (`sun_reach_cs`, `Sun::up`, `Sun::up_at`), as nothing drawn is taller and
  everything stands on what the eye meets (22818bb97).
- **What a tile holds, and which tiles are read.** A tile's depth follows the
  eye's place, the light and the world; which tiles are read follows the
  look, the actors and decor placed again. Where only the latter moved, a
  tile cast since the former last did stands, and only tiles read anew are
  cast (`sun::MOVED_EPOCH`, `sun::fresh_at`; 2e513d00c).

## What follows

- **Units matched cost what is read, not the reach.** Over the whole disk at
  the eye's unit, the sun walked about twice the eye's nodes: `sun_walk` p95
  about 20–28 ms, 68–78% of slots taking a pick. The disk cut 56% of the
  slots, the marks most of the rest: 1.7–3.3% of slots cast on the start
  view, the walks 30–150k nodes a cast, the sun's passes about 6 ms at 1456 ×
  828 drawn at two, against about 25 with the disk alone.
- **The marks are by tile, the reach gathers.** A receiver writes one word,
  and the reach's tile is written by its own thread alone. Marking a column
  a tile wide from every pixel cost 20 ms at two; one tile a pixel, 4.

## Not taken

- **Frustum-fit cascades.** Matching the eye's unit in each cascade takes
  about 6144² texels a cascade, scale-free, several million a cascade even
  cut to the view: more than one map, for a unit already met.
- **Rings keeping the sun's own unit, with a receive rule.** A receiver
  within a recorded block counted lit is the depth push again.
- **Reuse keyed by the zoom a slot recorded.** The answer changes where any
  point on the ray's path changes zoom, not only its stop.
- **A sun ray at each of the eye's samples.** Exact for the world's surface,
  blind to decor and the actors.

## Open

- The sun's beam rays still cover the disk; they could follow the marks.
- An actor moving casts the whole chain each frame, though only its tiles are
  new.
- At a low sun the way up is long, and the reach reads more a tile.
