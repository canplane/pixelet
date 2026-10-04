# The view is cubes about the eye

**2026-09-26** — a design conversation, built the same day on the
`eye-sphere` branch and measured in the browser against main. It supersedes
two points of
[one order, out from the center](./20260926-one-order-out-from-the-center.md)
— the center the order was taken from, and a node one band high — and
builds what that document listed as missing, except occlusion.

## What changed from the morning's document

That document ordered every piece of work by how near it came to the view's
center, "the point the camera turns about, where the character stands". That
is two things and neither is the one that sees. The character is a thing in
the world like any other, and the point the camera turns about is the handle
it is moved by. What decides how large a cell is on the screen is the eye,
and a view ordered from anywhere else asks first for ground the eye is not
closest to. So there is one center, the eye, and the map center orders
nothing.

It also gave a node as `(z, x, y, band)`, 64 cells across and a band
high. At z16 that is a cube, but a band is 64 units at every zoom, so a z10
node would be a slab 65,536 units across and 64 high: not a cell of the
hierarchy, with no parent or children `space/hierarchy.rs` could name, and
a view made of them would be a stack of plates rather than a tree. A node is
a cube of its own zoom instead, 64 cells a side on all three axes, with
eight children a zoom finer, and the band is where a z16 cube stands in
height.

## Addresses in 3D, storage in 2D

A chunk is its zoom and an x, y and z (world/pyramid/chunk.rs `Chunk`), and nothing
more: no table holds one per block, and the storage under it is not touched.
Terrain stays a 2D page and objects stay columns of bands, and a rebake was
never part of it. What a chunk is pointed at is its column (`Chunk::column`),
the node in two axes that the page and the object store are read by, and its
page and its place in the page are that column's.

The height of a chunk comes from its index and not from its corner as a
point. A point's z is a 16-bit lane, ±32,768 units, and a cube at the
pyramid's top is 65,536 units tall, so the corner of the second one up is
out of any point's reach; before this, the corner wrapped, and chunks turned
up at z index 511.

A block with nothing in it is no chunk. The lookup that says so is
`Domain::occupied` (world/pyramid/domain.rs): the ground's span over the
column cut to the cube's heights, or any object in a band the cube holds
(layer/object/store.rs `Store::holds`). The walk down the tree asks it of
every child it would make, so the air over a plain and the air between two
things stacked apart are never split, counted, fetched for or drawn.

A column's heights — the ground's and the objects' together, and each apart
where occupancy needs them apart — are read once per revision of the
terrain and the objects (`Domain::heights`, `spans`) rather than once per
chunk per step of the budget's search, which had been most of a zoom's turn
on a mountain. The ground's span is read from the footprint alone, off the
page the ground is drawn from (layer/terrain/relief.rs `ground_under`),
never a page's whole relief: a mountain in one corner of a page then makes
its own ground near and no other.

The surface drawn is a Catmull-Rom interpolation, which overshoots the
samples it reads. Its negative weights are at most an eighth an axis, so a
height lies within 9/32 of its sixteen samples' span past either end. Taken
over the whole footprint, that bound reached a quarter of the tallest relief
past ground that does not move, sea beside a hill read far under the sea;
taken window by window, it reaches only as far as each window moves.

## One order

Everything a view asks for is ranked by one key (`Order`): seen before
unseen, then nearer the eye first — the shells the rule draws its zooms in,
from the inside out. The same key ranks the fetch classes of every layer
(io/policy.rs), the chunks mounted and built (world/chunk/table.rs
`priority`), and the record given up when a wanted page needs the room, the
last in the order first. A chunk is near by its cube: the nearest point of
its box, the column's heights cut to its own, from the eye in three axes.

A fetch the view has left is stopped in the turn it leaves (io/sched.rs
`pump`), all of them at once, and its credit is free that turn; a late
answer for it is ignored. Before, one was stopped only when something waited
for its credit, one per host round trip, so a zoom out and straight back in
left all six credits on the zoom passed through.

## A budget of cubes in the frustum

What a view holds is counted in what it draws: the cubes the rule draws,
with every ancestor of them, against `VIEW_CHUNKS` (1,024); the pages they
need, against `VIEW_PAGES` (48); and
the z16 cubes it sees against the z16 chunks a view over
flat ground draws — about 330, a disc of the zoom's reach widened by a
chunk's diagonal. The last is new with the cube. Flat ground is one cube a
column, but a slope that crosses a cube's floor or ceiling is two, and a
mountain's view wants more than flat ground's. Where any of the three is
past its bound, every split distance is shortened alike a quarter of an
octave at a time until they fit, so the view gets coarser and nothing is
left out. It is loosened at once and tightened only when a step finer would
also fit, so a view on the edge of its budget does not flip as pages land.

A slope two cubes a column was measured at 1.5 cubes a column near Gangnam
and 1.7 on Bukhansan.

## The mountain holes, and upper bounds that close

Ground on steep slopes showed holes, from any distance, with its top missing
and not only its sides. It was on main too, and three causes were found.

- **A chunk not yet built was culled under a camera looking at it.** Its
  bounds before its first mesh were a sphere at sea level, so a mountain's
  chunk was off the screen until it was built, and it was built late because
  it was off the screen. Its bounds are now the domain's heights cut to its
  cube (world/chunk/table.rs `Slot::unmeasured`).
- **A mesh was cut to its slot.** The leaves' slot was sized on flat ground,
  and a slope ran past it; the rows past the slot were lost, which is the
  missing top. A mesh is never cut now.
- **A peak was cut at its parent's ceiling.** A coarser zoom averages a
  peak under its ceiling, and the finer zoom's ground past it had no chunk
  over it. A parent now takes the finer zoom's ground up to its column's
  nearest held chunk above or below it, and splits into the chunk that
  ground ends in.

What replaced the cut is a bound that closes. The leaves' own tier holds
`FINE_BOXES`, derived and not measured: a column of heightfield is one run
in a chunk that is a cube, and a box is at most a run, so 64² boxes (world/
chunk/mesh_slot.rs). Every other tier holds `COARSE_BOXES`, 4,551, said
aloud as a pool size; a coarse cell at step `s` holds at most its own box
and the floor under it, so a chunk bakes to at most 2·(64/s)³ boxes and at
the coarsest step always fits. A mesh past its pool is baked one step
coarser (`BuildEvent::Coarser`) until it fits, and the ledger's
`slot_coarser` counts how often. The one alternative weighed was capacity —
slots grown until the worst mountain fit — which is a number the next
mountain disproves.

## The balance pass removed

The terrain had one limit of its own (layer/terrain/present.rs): no two
chunks that met were more than a zoom apart, since a skirt hides a step of
one zoom and not two, and meeting it cost a walk up every chunk above and
below each side. It is gone with the layer hook it was the only use of. A
chunk at its own cells draws the sides of its rim down to its floor whatever
the apron beside it holds (pxl-render mesh/chunk_mesher.rs), and a coarser
tier summarizes no ring and so stands open on every side already; two chunks
that meet may then be any zooms apart and neither is open to the sky.

## Objects live in the cube their band is in

A band is a z16 cube's height, so an object in a band is in exactly one
chunk a zoom. A write to a band (world/chunk/source.rs `band_written`)
moves the tiers and decor revisions of the chunk it is in at each zoom —
and, above z16, the chunks beside it whose fold reads its apron — and of no
other chunk over its column. A chunk draws and carves only the bands it
holds (`draws_band`). Before, a write moved every chunk over its column, and
a column's objects were one height range, so the cubes between two things
stacked apart were held.

That puts placed objects on the ground's lifecycle: split, ordered, counted,
built, invalidated and drawn with the cube they are in. What is not shared
is storage, which stays what it was.

## A camera inside a chunk

A chunk the camera stands in is seen whatever the frustum says of its
center. That was tested on its footprint, which in 3D counts every chunk
over the camera's column as around it, the ones far above and below too; it
is tested on its sphere now.

## Measured

Response is the time from the zoom command until the ledger shows nothing
pending, no gap in the cover, no queued build and no stale chunk. A zoom
from 20,000 u in to street level, one tab, the rest of the machine idle:

| Place      | main        | eye-sphere  |
| ---------- | ----------- | ----------- |
| Bukhansan  | 1.49–1.62 s | 1.42–1.47 s |
| Gangnam    | 1.17–1.22 s | 1.03–1.14 s |
| Hoengseong | —           | 1.8–1.9 s   |

No build waited on a slot in any of them. The camera's own animation
settles in about 60 ms, so it is not what the seconds are.

Three things made earlier numbers wrong, and each is worth knowing before
measuring again. The build artifacts carried dates in the future after a
clock skew, so cargo rebuilt nothing and the browser ran an older kernel;
`cargo clean` in the worktree fixed it. Two tabs of the app shared the
machine and the network. And a Chrome window left behind others stops its
animation frames, so a view in it never finishes.

Found along the way: the debug gizmo drew the dashed line from the map
center to a floating origin in dashes of fixed length, millions of them a
frame after a zoom out, and a heap that refused the block trapped the
kernel. It draws 256 at most now, lengthening them past that
(layer/marker/frame_gizmo.rs).

## Where it stands

**Built** on `eye-sphere`, not on main, not deployed. Open:

- **A column's runs are built again for every cube over it.** A chunk
  composes its whole column and bakes the part in its heights (the column
  run set cut to the cube, pxl-render mesh/voxel_scan.rs
  `OwnedRunSet::clip_from`), so a column two cubes tall is scanned twice.
  The runs could be the column's once and each cube a cut of them.
- **Occlusion.** A chunk behind a ridge is split, fetched and built like one
  in view. The 3D occluder is a box, not a horizon.
- **A cut ceiling flashes on a cold load**, while a coarser page stands in.
- `surface_at` still answers from a heightfield, by walking the column's
  chunks from the top down. That is the question it is asked — where the
  ground is under a point — and it is kept as the one 2.5D path.
