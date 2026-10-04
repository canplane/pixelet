# What is heavy, and what gives way

**Measured 09-23, not built.** The frame is one pass: the world pass is 85%
of it and every other span rounds to zero. The device's mesh memory is nine
tenths reserved and empty. And the one machine built to give way under load
watches neither of them — it watches the heap, which is at a twentieth of
its room.

This is the queue that follows from those three facts, in the order the
measurements put it. What each item is worth is recorded beside it, and what
is not worth doing is recorded at the end so it is not proposed again.

## The frame is one pass

Over 955 frames of a sweep with tilt mixed in, on an M1 Max:

|      | p50  | p95  | max  |
| ---- | ---- | ---- | ---- |
| dt   | 25.0 | 41.8 | 75.0 |
| task | 2.9  | 4.0  | 9.0  |
| gpu  | 21.3 | 37.1 | 40.9 |

All 21.3 ms of the GPU's time is the world pass. The air's tables read 0.2;
the shadow map, the depth prepass, the occlusion, the badges and the cases
read zero. The kernel's own work is a ninth of the frame.

So the frame is not shared out among many costs to be trimmed one by one.
Anything that cuts the world pass is worth its weight, and nothing else in
the frame is worth looking at until it does not dominate.

## The device holds ten times what it uses

The arena is a fixed grid of slots, one for every zoom and residue a chunk
can have (`world/chunk/mesh_slot.rs`):

|                                                      | reserved    |
| ---------------------------------------------------- | ----------- |
| five coarse groups, 576 slots of 4,551 boxes at 16 B | 200 MiB     |
| the leaves' group, 576 slots of 2,730 boxes at 24 B  | 36 MiB      |
|                                                      | **236 MiB** |

What lives in it is 2,304 to 2,964 chunks at about 500 boxes each — 20 to
30 MiB. The rest is reserved and empty, and it is reserved from the first
mesh written, whether or not the window ever reaches that zoom.

Two things cannot move it, and both were tried:

- **The ladder cannot.** Its thresholds choose which zoom covers a place,
  not how many steps a chunk is cut into: `ask` gives every zoom over the
  leaves the same step 2, so a chunk is always 64 cells at 32×32 whatever
  the zoom. Pushing `TARGET_CELL_PX` from 6 to 12 leaves the arena at 236
  MiB exactly.
- **The slots cannot be trimmed.** A sweep over Seoul and Gangnam, 150 u to
  38,400 u, asked a coarse tier for 3,842 boxes — over the 3,769 the slot
  was sized from. Trimming to a tighter measurement leaves under a tenth in
  hand, and a mesh past its slot is ground that goes missing.

## What gives way watches neither

`Ladder::set_pressure` pulls every threshold to `2^(-pressure/2)` and makes
the whole screen coarser, the way Cesium raises a tileset's screen-space
error when its cache is full. It is wired and it runs every frame.

It never fires. Across three sweeps and 6,367 frames the pressure stayed at
zero, because its signal is a refused heap block and the heap is at 47 MB of
863. A chunk's meshes left the heap for the device when they went into the
arena, and the arena is a fixed size, so nothing there can be refused.

So the governor's target is gone. What is left in the heap — decor — has too
much room to ever pull on it.

## The bind group is set once a draw

`draw_meshes` calls `set_bind_group` for every draw it issues, and the value
it sets is one of the arena's six groups. At the p95 of 1,199 draws that is
1,199 calls where six would do.

It is worth more than it sounds. The draws are where tilt costs what it
does: the same triangles come out split across more draws when the frustum
lies long and thin along the ground — 938 to 1,199 at the p95 — and the
frame's tail grows 18% with them while its median does not move. Every draw
is two calls out of the module, and one of them is this.

Skipping the call when the group has not changed, and ordering the draws by
group so it changes at most six times, halves the crossings. A draw carries
its own uniform index as its first instance, so the order is free to change.

## A mesh that cannot land is baked again every tick

Two chunks of one residue share a slot, and the one that gets there second
waits: `self_record` throws its mesh away rather than take the ground from
under a chunk that is still drawing from that slot. The waiter is still
visible and still pending, so the next tick puts it back in the builds and
it is baked from the start again. The ledger has read a visible chunk
waiting 20.7 seconds, and the bakes at a thousand a second.

The conflict is known before the bake: the slot is `slot_of` of the chunk's
own coordinate. Checking it where the chunk is asked for, rather than where
its mesh lands, ends the loop — the chunk is parked until the holder leaves
instead of being baked into the bin.

That alone does not end the wait. **Two zooms share a group when they lie a
window apart** (`z` modulo the window's width), which a jump that skips the
zooms between puts on screen together: from z6 to z16 the old zoom is the only
thing covering its ground, so it is drawn; the new one cannot land because
the old one is drawn; and the old one is not retired because nothing covers
it. The census counts it — `stale_on_waiter` — and a zoom taken in steps
never meets it, because the window slides a zoom at a time and each is
covered before the next is wanted.

Two ways out. A rule: when the waiter's zoom would cover the holder's, the
holder goes first. Or the grid itself, below.

## The arena is a grid where it could be a pool

A slot's address is a multiply — `slot_index × capacity` — so every slot is
the size of the worst chunk, and one exists for every zoom and residue
whether anything lives there or not. That is where the ten-fold reservation
comes from, and where the residue conflicts come from with it.

Nothing in the draw path asks for it. `first_box` is already an arbitrary
number the draw record carries, passed as the draw's first vertex, and the
shader reads `boxes[vertex / BOX_VERTICES]` with no base of its own. The
uniform grid buys one thing: there is no allocator to write.

What would take its place is a **pool**: blocks in a few size classes,
handed out from a free list and given back. Memory becomes what is held
rather than what could be held, classes bound the fragmentation a plain
best-fit would leave, and a mesh stays contiguous, which is the one thing
the draw path does need. The device side does not change — not the shader,
not the box, not the write. The kernel gains a table from chunk to block, a
few tens of kilobytes.

Then two things follow that cannot follow now. Residue conflicts end, and
with them the wait and the deadlock above. And the arena becomes a budget,
which the pressure that already exists can be paid from — a handheld's is
one number rather than a branch.

## A coarse cell's floor is the centering's, not the ground's

A coarse tier is about 3.4 runs to a cell: the full buckets under the
surface, merged into one; the floor that fills the gap under the top
bucket's box; and that box. Dropping boxes under a fill threshold does not
move the 3.4 — that was measured and reverted
([a coarse cell is isotropic](./20260923-a-coarse-cell-is-isotropic.md)).

The middle term is not the ground's. A box shrinks by `∛f` **about the
cell's center**, which leaves a gap to every neighbor, and the floor exists
to fill it. Give the box the occupied extent on each axis instead, scaled so
its volume is still `f`, and flat ground fills its cell laterally on its
own: no gap, so no floor, and the top bucket merges with its neighbors as
the full buckets already do. The rule stays symmetric — the same sentence
for x, y and z — so it does not fall to the evidence that withdrew the
height field, where thirty-two voxels lying flat and thirty-two standing up
came out drawn and vanished.

It is a change to a rule decided the same day, so it is a decision and not a
correction. What would settle it is the sweep that is already written:
Seoul and Gangnam, 150 u to 38,400 u, reading `work coarse_boxes`. The
prediction is 3,842 down to about 2,700. If it does not fall, the model is
wrong.

## What a target would have to be

Giving the pressure a live signal — the world pass's own milliseconds in
place of the heap's refusals — puts the governor back on something that
moves. The machinery is all there: the step and relief timers that make it
slow to coarsen and quick to relent, the margin that stops a chunk flipping
on a threshold, the dither over the first three handoffs. Only the input
changes.

The target it steers to is the decision, and two things have to be known
first.

- **The device is the phone, not the desk.** The target on record is an
  iPhone 13 mini on a 16 ms display
  ([mobile and perf targets](./20260810-mobile-and-perf-targets.md)). A
  desktop's 21.3 ms is the wrong ruler: steering to it leaves the phone
  where it was and coarsens the desktop for nothing. The target wants to be
  a share of the device's own frame, so one constant fits both.
- **A coarser cell does not divide the pass.** One zoom coarser is a
  quarter of the cells, so a quarter of the boxes and a quarter of the
  triangles — but the same pixels are covered, by larger triangles. The pass
  falls to `vertex / 4 + fill`, not to a quarter. The ledger already carries
  what separates them: readings at one `fill Mpx` with the triangles varying
  give the slope, readings at one triangle count with the viewport varying
  give the rest. Four readings decide whether 21.3 ms becomes 6 or 11, and
  that is what decides whether the ladder reaches 60 Hz at all.

One guard belongs with it. A step of pressure moves the window up, and its
new top is a zoom nothing is holding yet — a fetch and a bake away. The
near ground goes coarse at once, from zooms already held, but the far ring
is blank until it arrives, and a loop reading the frame while that happens
will read its own lag as load. Pressure wants to be clamped to what the
window holds.

## The order

1. **The bind group**, which is two lines and changes what everything below
   is measured against.
2. **The slot checked before the bake**, which ends the loop that bakes into
   the bin.
3. **The waiter's priority**, which ends the wait itself for the price of a
   rule.
4. **The pool**, which ends it by construction and takes the memory with it.
5. **The coarse cell's extent**, which lowers what a slot is asked for
   rather than what it is given.
6. **The pressure's signal**, once 1 has landed and the vertex-to-fill split
   is known.

## Closed

- **Work for a zoom that has passed is already canceled.** The builds are
  cleared and rebuilt every tick from the chunks that are visible and
  pending, so a chunk the window has left stops being stepped on the next
  tick. What is wasted is a tick's budget, not a bake.
- **The slots stay as they are.** The strongest sweep asks for more than
  they were sized from.
- **A fill threshold does not pay.** Measured and reverted; the notch it
  leaves is a quarter of the detail the ladder had already spent, and it
  buys noise.
