# One order, out from the center

**2026-09-26** — a design conversation, partly built. It settles how every piece
of work a view asks for is ordered and dropped: pages fetched, chunks mounted
and built, decor, places.

**Superseded in part** the same day by
[the view is cubes about the eye](./20260926-the-view-is-cubes-about-the-eye.md):
the order is taken from the eye and not from the center the camera turns about,
and a node is a cube of its zoom, not a block one band high. The rest stands,
and what it lists as missing is built there but for occlusion.

## The rule

There is one key and one demand. The key is how near the view's center a piece
of ground comes, in three dimensions: the center is the point the camera turns
about, where the character stands, and the ground is a chunk's box — its cells'
footprint and the heights its terrain and the objects on it span
(world/pyramid/domain.rs `Domain::order_of`). A 2.5D page is measured as the 3D
boxes of the chunks drawn from it, so terrain, objects and places sit in one
sphere. The demand is the view's domain as it stands this turn.

Every scheduler takes work nearest the center first and drops at once what has
left the demand. What a view shows is then always a sphere of current ground
growing out from where the camera looks, overwriting whatever the last view drew
there — a stale chunk, or one borrowed from a page over it. Response time is the
measure: the middle answering the camera, not holes at the edge being filled.

Between layers the order is the one a view is built up in: terrain, then the
catalog models its decor is drawn with, then the places. Placed objects are
resident and built with the ground's chunks, and decor follows a chunk as it
commits, so both ride the ground's order.

## 3D is the model; 2.5D is storage

A page's heightfield is how ground is stored, a compression of it. Nothing that
decides what a view does is 2.5D: selection, order, budget, occlusion and
drawing are 3D, over boxes in a sphere, for terrain, places and objects alike —
the rule chose its zooms and derived its budget that way from the start (the
ball about the eye in
[what a view costs is the eye's](./20260925-what-a-view-costs-is-the-eye-s.md)).
A 2.5D path is an optimization, taken where it is known to behave exactly as the
3D one would — a height read off the raster rather than a ray cast through the
volume.

The code is still 2.5D in four places, and each grows worse as placed cubes
multiply:

- **A chunk is a column.** Its box is a footprint and one height range, never
  split in height, so a tower stretches it upward and its nearest point —
  measured over the whole box — makes the ground at its foot as near as the
  tower's top.
- **The budget counts a disc.** Chunks are counted by footprint; cubes stacked
  in height cost what a ball does, and the budget does not see them.
- **Objects ride the ground's chunk.** They are folded into the column they
  stand in, with no node of their own to be refined or coarsened apart from the
  ground.
- **Occlusion as proposed was an azimuth horizon**, which only ground can cast;
  a 3D occluder is a box.

## Addresses in 3D, storage where there is something

Making the nodes 3D does not mean making an array 3D: one allocated in three
axes over the view would be almost all air and would not fit. The view is given
a 3D address space instead, and a lookup that turns an address into where the
thing addressed is stored.

- **A node is an address.** A block of a zoom's cells in three axes —
  `(z, x, y, band)`, 64 across and a band high — exists only as that tuple. The
  domain's walk makes them as it splits and drops them after; no table holds one
  per block.
- **A lookup points into storage.** Given an address, it returns where its
  contents are: for ground, the page that draws it — its own, or the nearest
  resident page over it — the cell in that page and the elevation samples around
  it, which is what a chunk's build already resolves in two dimensions
  (world/chunk/build.rs `Chunk::read`); for objects, the band tree under the
  column (layer/object/store.rs). A band adds one axis to what is there.
- **Empty is an answer, not a slot.** A block the ground's surface does not
  cross and no object fills is found empty by the lookup — the footprint's
  height range against the band, the objects' reach against it — and is never
  split, counted, fetched for or drawn. The 2.5D ground is the case of a
  footprint whose one crossed band is all there is.
- **A 2.5D path lives inside the lookup.** Where the answer is known to be the
  3D one — the height of the ground at a point, read off the raster rather than
  found by a ray through the volume — the lookup takes the short way. The
  shortcut cannot change what the view does, because nothing outside the lookup
  can tell it was taken.

Two costs come with it. A lookup is an indirection, and the mesher reads a
chunk's cells tens of thousands of times a build: it resolves the chunk's
storage once and reads the region as it does now, rather than looking up cell by
cell — what OpenVDB's accessors do by caching the path to the last leaf. And the
footprint's height range has to be a chunk's, not a page's, for the empty test
to discard anything: the page's relief is too coarse, and a chunk's range is a
few dozen of its elevation samples.

## What stands on it now

- **Drawing in place.** Every chunk the rule wants is drawn in its own place,
  from its page or, while that has not come, from the nearest resident page over
  it, upsampled (`Pyramid::drawn_from`). No chunk waits on its siblings, so
  ground whose page lands is on the screen that turn. Built and measured on
  2026-09-26: after a jump to a cold coast every wanted chunk was drawn from the
  first frame, where a resident top page had drawn the whole view as one chunk
  for 4.3 s.
- **Fetch.** Pages of every layer are ranked by the key (io/policy.rs): the page
  the ground nearest the center is drawn from, then the pyramid's top pages,
  which every chunk can borrow from, then every drawn page nearest first, then
  pages held only as a fallback.
- **Mount and build.** The chunk table mounts and builds nearest the center
  first (world/chunk/table.rs `priority`), with only the slots' holders ahead
  and off-screen work behind. The classes that ranked a chunk showing nothing
  before one showing an older generation before one asking to be sharper, and a
  wait that halved a chunk's key every four seconds, are gone: they sent the
  builds to the edge of the view while the middle stood coarse.

## What is still missing

Found by reading, not yet measured or built.

- **Fetches outlive their demand.** A fetch the view no longer wants is stopped
  only when something waits for its credit, and one at a time (io/sched.rs
  `preempt`). A zoom out and straight back in leaves the six credits on pages of
  the zoom passed through, released one per host round trip while the zoom
  wanted now waits. Everything outside this turn's demand should be stopped in
  the turn it leaves it.
- **Steep ground starves.** In mountains a chunk's split is decided by its
  page's height range, which changes as each page lands — an ancestor's range,
  then its own. The decision can flip, the cover change, and chunks go stale and
  are mounted again before their builds finish, over and over. The ledger's
  cover lanes and mount counts would show it. The prior art keeps what it drew
  until what replaces it is ready (below).
- **Occlusion.** A chunk behind a ridge is split, fetched and built like one in
  view, and in the center's order it can come early. Refusing to split what is
  occluded, and keeping it at the zoom it has, is where the saving is in
  mountains.

## Prior art

The first four are how a sparse 3D space is addressed; the rest, how a view is
scheduled over one.

- OpenVDB holds a sparse volume as a shallow tree of fixed-size blocks, stores
  only blocks with something in them, and reads it through an accessor that
  caches the path to the last leaf, about three times faster for coherent access
  ([overview](https://www.openvdb.org/documentation/doxygen/overview.html),
  [ValueAccessor](https://www.openvdb.org/documentation/doxygen/ValueAccessor_8h_source.html)).
- 3D Tiles' implicit tiling names every tile by `(level, x, y, z)` in a quadtree
  or octree and ships no tree at all, only bitstreams saying which addresses
  have a tile and which have content
  ([spec](https://github.com/CesiumGS/3d-tiles/tree/main/specification/ImplicitTiling)).
- GigaVoxels keeps an N³-tree whose nodes point into a pool of bricks or mark
  themselves empty or constant, and streams bricks as rays ask for them by
  detail and occlusion
  ([paper](http://gigavoxels.inria.fr/Publications/2009/CNLE09/CNLE09.pdf)).
- Minecraft cuts a column into 16 × 16 × 16 sections and does not store an empty
  one ([chunk format](https://minecraft.wiki/w/Chunk_format)).
- Cesium's 3D Tiles streaming ranks every request by one number built from
  distance to the camera and to the screen's center, cancels requests that have
  left the view or will have by the time they land, and defers the screen's edge
  until the camera rests — up to ten times faster in flight, with a quarter to a
  half fewer tiles fetched
  ([blog](https://cesium.com/blog/2019/05/07/faster-3d-tiles/),
  [PR #7774](https://github.com/CesiumGS/cesium/pull/7774),
  [issue #5509](https://github.com/CesiumGS/cesium/issues/5509)).
- cesium-native's selection ranks within a group by
  `(1 − dot(tileDirection, cameraDirection)) × distance`, keeps drawing what was
  drawn last frame until its replacement is renderable ("ancestor meets SSE",
  "kicking"), and caps the descendants a view waits on so it shows an
  intermediate ancestor rather than nothing
  ([selection algorithm](https://cesium.com/learn/cesium-native/ref-doc/selection-algorithm-details.html)).
- Cesium for Unreal refuses to refine a tile its GPU occlusion query finds
  hidden, and does not traverse past one whose result has not come: 17 to 31 per
  cent fewer tiles loaded at street level, nothing from the air
  ([blog](https://cesium.com/blog/2022/08/18/occlusion-culling-cesium-for-unreal/)).
