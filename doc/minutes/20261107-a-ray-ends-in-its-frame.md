# A ray ends in its frame

**2026-11-07** — recorded with the user, in the session that compares the frame with
the deployed build (`e317a3644b`). It changes
[the frame is what it must draw](./20261106-the-frame-is-what-it-must-draw.md).

## What the user decided

**Every ray ends in its frame.** The budget of visits and the parking of rays across
frames go. The stops keep a list's room and are no budget. Rays still park between a
run's walks and its asks, within the frame. This replaces "rays descend as far as each
frame lets them" of 11-06.

**The asks stay, as a budget of detail.** What a frame defers is detail, never the
frame's answer. A ray that cannot go on, its frame's asks spent or the pool out of room,
ends where it stands as a lack, at the deepest node held.

**`auto` bounds the frame from outside.** It gets a third knob, the rate turns go in at,
a whole number of the display's refreshes. Down, the rate moves first, then the scale,
then the lattice; up, the reverse. This takes up the backlog's third knob.

**The papers under `out/`.** The two the 11-06 minutes point at moved here:
[cell raster](./20261106-cell-raster.md) and
[the plan](../experiment/20261103-the-frame-on-the-s23/README.md). The rest are not kept.

## What was weighed

The deployed build has no budget of the queue's: none of `VISITS_A_FRAME`,
`ASKS_A_FRAME`, `STOPS_A_FRAME` or `PARK_ROUNDS` is in its tree, and every ray ended
within the frame. The user reported, on builds with the budget, terrain breaking while
moving, ghosts, and decor and terrain out of step; none of them on the deployed build.
A ray past the budget left its pixel to the record's stale voxel.

On the hot S23 at 348 MHz, on the user's drive:

| budget | casts a minute | GPU busy | picture |
|---|---|---|---|
| 2^23 | 348 | 157 ms | |
| 2^22 | 456 | 120 ms | the ground flatter |
| not reached | 381 | 142.5 ms | |

Whether these three builds differ by the budget alone was not checked.

The code did not do what the decision first assumed. `ASKS_A_FRAME` deferred the ray,
not its detail: a walk out of asks was left (`KIND_LEFT`) and parked, and rays past the
pool's room kept their owed bits, so both crossed frames.

What other systems do, read from their documents and forums:

- Nanite keeps root pages resident, so a mesh always has something to draw.
- A virtual texture falls back to a coarser mip while the finer page is not in.
- Cesium's 3D Tiles draw a parent with its children until the visible children load.
- Lumen limits the surface cache's updates a frame; fewer are cheaper and slower.
- RayCaching spreads a voxel cast over frames by tile, and leaves undersampling open.

Each defers detail and answers the frame from something coarser.

`auto` alone could not hold the frame: on 11-03 the S23 held 6–8 fps with both knobs at
their floor ([the walk on a phone](../experiment/20261103-the-frame-on-the-s23/README.md)),
and the light's and the cells' passes do not shrink with the pixels. The rate shrinks
every pass's work a second. Android's frame pacing guidance favours a steady divisor of
the refresh over uneven frames. Casting at a lower rate than the drawing, as Application
SpaceWarp does, was set aside: it nearly doubles latency and synthesizes frames, and it
is the budget's stale picture again.

## What was not a decision

The paper `lattice-queue` said "The lattice goes under the budget" under "the user's
standing decisions, via the coordinator", and this session passed that on as the user's.
No message of the user's in the session that wrote it says so. The nearest line is a
pasted outside review: rays go as far as the budget and unfinished work is kept. The
11-06 minutes' "as far as each frame lets them" is the only attributed line, and this
record replaces it.

## Built, 11-07

- No visit budget, no ray parked across frames, no owed work and no readback that wakes a
  frame for it. Rays park only between a run's walks and its asks.
- A ray the pool has no room for, or that may ask no more, ends as a lack where it stands
  (walk.wgsl `lacks`). `ASKS_A_FRAME` is one count for the frame, every run's together.
- The rolled lattice and the light's claim budget went with them
  ([the deployed build is the bar](./20261107-the-deployed-build-is-the-bar.md)).
- The rate is `auto`'s third knob.

## Open

- The rate's floor, 30 Hz, is the slow end of 11-06's target, not a measured figure.
- The on-screen name "auto resolution" no longer says what it moves.
- A frame has no bound of its own: a spike lasts until `auto` acts. Unreal's dynamic
  resolution has a panic switch for that; none is built here.
- The rate does not make a frame faster. At 142.5 ms the hot S23 stays under 11-06's
  30–40 Hz.

## Sources

- [Nanite root pages](https://forums.unrealengine.com/t/preventing-nanite-pop-in-issues-in-2d-rendering-use-cases/2512906)
- [Software Virtual Textures](https://www.quaddicted.com/webarchive/mrelusive.com_20170131/publications/papers/Software-Virtual-Textures.pdf)
- [CesiumJS PR 4287](https://github.com/CesiumGS/cesium/pull/4287/files/dd35d3566e83c35903a2b488e229fe7c3969caa9)
- [Lumen surface cache](https://forums.unrealengine.com/t/surface-cache-statistics/2675304)
- [RayCaching](https://uu.diva-portal.org/smash/get/diva2:1375732/FULLTEXT01.pdf)
- [Unreal dynamic resolution](https://dev.epicgames.com/documentation/unreal-engine/dynamic-resolution-in-unreal-engine)
- [Android frame pacing](https://developer.android.com/games/sdk/frame-pacing)
- [Application SpaceWarp](https://developers.meta.com/horizon/documentation/native/android/os-app-spacewarp)
