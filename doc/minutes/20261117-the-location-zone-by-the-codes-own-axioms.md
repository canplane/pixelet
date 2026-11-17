# The location zone, by the code's own axioms

**2026-11-17** — recorded with the user, in the session that moved the location
indicator's and the markers' asks of the ground under the entry's budget
(`cfc72c4ac5`, `ff07ca506e`) and then measured them
([the deployed build's pace, measured](../experiment/20261116-the-deployed-builds-pace-measured/README.md),
"Then: the night's quick comparison").

## What was asked

The measurement found the tick slower with the location's cells: about 1.2 ms at p50
on the S23 and 0.3 on the Mac, each frame, the scene step's. The session offered three
ways out by the numbers: put the location step back in the effects with its cells kept;
put the location back as it was before the change, one memo for the zone in an effect;
or keep it in the visible phase and lay the zone only once a pass ended.

The user turned the question round:

- Putting the location back as it was is ad hoc: an effect that does real work is the
  exception the rule names.
- The profile is not what the user cares about here. What matters is whether the
  state is clean: whether the code is consistent, and whether understanding a part
  takes the fewest axioms.
- No ad hoc piece is to be added, and a claim about the structure is to be reviewed
  adversarially before it is acted on.
- What the session says and decides is to be kept in the record, not left in the
  conversation.

## What was weighed

- **The location step in the effects, its cells kept.** It measured as the base did on
  the Mac (two runs), since the budget is fresh at a frame's start and a recompute ends
  in one frame. Dropped: it is chosen by the figures, and it would make the location the
  one part whose asks run through the budget's pass from an effect.
- **The location as it was.** Dropped by the user: ad hoc.
- **A pass held as state.** A list of the cells due, taken when a pass starts, asked
  across frames under the budget, and the zone laid once when the list is done; a change
  during a pass leaves the cell for the next. It ends in bounded frames. Dropped by the
  session: a list and a cursor held across frames is a concept no other part of the code
  has, one more axiom to read the location by.
- **The claim now under review.** The core's parts that ask the world for many answers
  follow four axioms: an answer from the world is kept until a change touches what it
  read (`Memo`); what is to do is drawn again each frame from state, what has no kept
  answer; what is to do goes through the budget's pass (`sched::run`), what is left drawn
  again the next frame; a result changes once every answer it needs is held, the last
  result standing until then. The brick reads, the places, the pins and the clouds would
  each be read by these, a place needing one answer and the location's zone all its
  cells'. The location's code as committed breaks the fourth: it lays the zone again in
  every frame that asked a cell, a zone of some new and some old tops, which is what the
  measurement found costing in the visible phase. Its order, the cells never asked first,
  is a fifth axiom, added so that the zone is laid while changes drop cells every frame.
  The fix it implies: lay the zone only when no cell is due, drop the order, and add no
  state beyond the table of cells; the cost, that while the ground keeps changing
  without end the zone stays at its last complete state.

## What the review found

An adversarial review read the claim against the code. The parts do not share four
axioms: only the location keeps a `Memo`; the bricks keep theirs per brick, and a
place's top goes with its brick; the clouds read nothing from the world. The markers
share one more rule, nearest first, and the location's own order is apart from it. The
arrangement drops a touched brick's pins and places before the brick is read again, so
under the budget they vanish rather than stand. One memo for the zone takes the fewest
axioms, but under `sched::run` it is one item, always run, so the budget never bounds it.

## What the user said then

- The beacon's place is the fix itself, and its height is quantized; there is nothing
  to ask for it.
- A zone at the same place has nothing to ask either.
- The night's diff is the first thing under review: the state it made for nothing goes,
  and the code goes back to what it was.

## Resolved

- The night's three commits of code (`cfc72c4ac5`, `ff07ca506e`, `6de67851df`) are taken
  back: the location indicator is again one memo for the zone in an effect, and the
  markers ask the ground as before. The tree's code under `rust/`, `ts/` and `spec/`
  is as at `bec3061dc4`.
- The two edits that were uncommitted (the location's pass over the cells due alone,
  the zone's span tested first) are dropped with it, and the cells' rewrite begun after
  the review with them.

## Open

- The zone's memo is keyed by the fix's exact place (`Key`, the device's coordinate and
  the world position made from it), so a move within a cell asks the whole zone again,
  though the zone is the same; and a zone whose block the heap refused clears the memo
  by a branch of its own.
- The arrangement's drop of a touched brick's markers before the brick is read again.

## Then: the zone keyed by its cell

The user said the zone needs no machinery: it is the fix's cell, quantized, with the
cells about it at fixed offsets, each set on the support query under it. A zone at
the same cell has nothing to query again. The location is an indicator, as the ghost
is, and the ghost is a tick that takes a quantized input (the hovered cell), calls
the query directly and writes only what changed. A query here is a read of what is
resident, made within the call.

The session took it so:
- The zone's memo is keyed by the fix's ground leaf cell, by its corner. The `Key` of
  the fix's coordinate and its world position goes: one fact held twice, and finer
  than what the zone depends on. `surface_support` reads the cell and not the point
  in it, so the center's query is the cell's. The beacon still follows the exact fix
  outside the memo.
- The location runs as a tick after the ghost, not as an effect, since an effect must
  not do real work (`frame.rs`). The step's name moves from the effects to the steps
  in `spec/client/core/perf/step.json5`.
- A test checks that a move within the cell grounds nothing again and that a move to
  the next cell grounds the zone there.

Left open: a zone whose block the heap refused still clears the memo by a branch of
its own.

## Then: the pose is real, the observation quantized

Asked whether the code minds the quantization of angles (polar, azimuth, distance),
which set off the rays, the session read what the code holds:
- The eye is snapped to Q3 (`pose.rs` `view_compose`). Motions in the world (the
  clouds, the beacon's bob and spin) step a visible step at a time, a twelfth of a
  camera cell (`StepClock`, `VISIBLE_STEP_CELLS`). The camera grid is an angle,
  1/96 rad (`lens::CELL`).
- The rotation is not quantized. A change of any size moves the view, and the view
  is drawn again, the pointer is picked again, and an observation is made again,
  since `Observation` holds the frustum in floats and is compared by value
  (`observation.rs`). From that the demand orders its wants again (`reorder`), and
  the markers lay themselves out again (`arrangement.rs`, whose input holds the
  observation).
- The ease stops by constants of its own, `EASE_EPS_RAD` and `EASE_EPS_U`, not from
  the camera grid. The angle's is about a ninth of a visible step, so the ease runs
  about 0.2 s past what can be seen, and the distance's is in units though what it
  moves on the screen grows with the distance.
- The pyramid quantizes what the observation yields (the bricks of the clip levels,
  the tiles asked for, both by the eye and the world alone), not what makes it
  computed again.

The user stated the principle, and the session agreed: the camera's place and angles
are real, and the observation is quantized. What is drawn (the rays of the screen,
the beacon, the pointer's pick) reads the real pose, so the picture stays continuous.
What reads the world (the demand, the bricks, the markers) reads an observation
quantized to the steps it can tell apart, and is computed again only when that
observation changes.

> Since 2026-11-17: corrected with the user. The rays of the screen are an
> observation too, the camera grid laid over the screen (`lens.rs`), and a pick is
> made against what is shown, so neither reads the real pose. The camera (the
> controllers, the ease, the input) is real, and it observes the world: what it sees,
> which cells at which zoom in which cell of the camera grid, is the observation, and
> the observation is quantized. It is made at one entry, where the camera observes,
> and everything that sees reads it: the screen's rays, the pick, the demand, the
> bricks, the markers. The screen is drawn again only when the observation changes.
> An asset that moves on its own is the same shape: its clock is real, and what is
> shown of it steps by a visible step (`StepClock`).

Open: the observation quantized so, the eye at the step of the pyramid it is read at
and the frustum at a camera cell, with the margin a frustum test then needs at the
screen's edge; and the ease's ends taken from the camera grid. Not decided.

## Then: what redraws the screen

The user named the two things that redraw the screen, the quantized camera's change
and an asset that moves on its own, and asked that their boundary be drawn and made
common, and that the quantization's entry be checked to stop what it should.

What the session read:
- **The camera.** Three gates, none at a visible step. The controller asks for a
  frame every frame it eases (`controller.rs` `camera_step`), until the ease's own
  constants end it. The view snaps the eye to Q3 and takes the rotation as it is
  (`pose.rs` `view_compose`). The renderer compares the frame's words byte for byte
  and draws the whole frame on any change, then asks for frames until the jitter's
  sequence has run (`renderer.rs`). The observation is made again on any turn, so
  the demand orders its wants again and the markers lay themselves out again.
- **An asset that moves on its own.** The characters, the clouds and the sky, and the
  location's beacon each step a `StepClock` by a visible step, wake the frame at the
  next step (`invalidate_at`) and tell the renderer it stepped (`note_animation`),
  which then asks for no frame after. The clock is shared; the wake and the telling
  are written out in each.
- **The asymmetry.** An asset moves by visible steps, wakes only at them and asks
  for no jitter frames. The camera moves by any amount, wakes every frame and always
  runs the jitter's sequence.

Proposed, not decided: one clock for a motion stepped by the visible step, owning
the step, the wake at the next one and the telling. The camera would be one of its
users: the observation it makes quantized at one entry (`view_compose`), the turn by the
angle of a visible step, the controller waking at the time its ease reaches the next
step instead of every frame, and the ease ending by the same step in place of
`EASE_EPS_RAD` and `EASE_EPS_U`. The observation would be quantized coarser above it.
Open: whether the observation may be quantized to a visible step, about 1/1152 rad,
which was chosen as a step not seen as a jump; by what distance a visible step of
the eye's place is measured; and whether the rotation goes first.

## Then: what stands while it is read again

The user had the two open items taken by external convention. MapLibre renders a
tile while it reloads (`reloading`), Cesium keeps an ancestor drawn until its
children load, and a streaming engine retries an allocation instead of making the
content again (Unreal's texture streaming).
- **A block the heap refused.** The location's zone writes its instances in place
  when they change, and a refused write is tried again from the instances held,
  a frame asked each time, as the ghost does. The branch that cleared the zone's memo,
  so every query ran again, is gone.
- **A brick read again.** What a brick read stays shown while the brick is drawn, and
  is replaced when the brick is read again; a brick that leaves drops it, and places
  go when the setting is off. Before, a touched brick's markers went before its read,
  which runs under the budget, so they could vanish for frames.

Left open: a brick that splits into its four or merges into its parent leaves the
drawn set, and its markers go until the bricks that replace it are read.

## Then: whether an ancestor is held

The user asked whether a tile held at zoom n means its parent at zoom n-1 is held,
recalling a history. An agent read the code and the record:
- **What holds now.** The tiles asked for are closed upward: the demand asks every
  demanded tile's line up to the layer's top (`demand.rs`, `Priority::Ancestor`, as
  Cesium's `preloadAncestors`), held while asked, so at rest the line comes to be
  held. The tiles held are not: each is judged on its own, so a missing ancestor never
  hides a resident descendant, and a miss is drawn from the finest held tile over it
  (`tile_cache.rs`, `finest_resident`). No test holds the ancestor asks since
  `183c4624ef`.
- **The history.** On 2026-09-25 (`b12bc77056`) every ancestor of the window's top
  pages was held to stand in for a page not come. On 2026-09-28 a closure was
  designed, a parent held whenever a child is, and dropped the same day
  (`e6e4670038`): a page read only once its parent was left a server with no coarse
  zooms drawing nothing, and one late ancestor everything under it; the agents found
  it in a run, and no word of the user's is recorded. On 2026-10-04 (`89a353b467`, no
  minute) the asked line came back as `preloadAncestors`. On 2026-10-14
  (`519cdf9f93`) a tile became readable only while both held and asked.
- **The markers.** A brick is not a tile: 64 cells a side against a ground tile's 512,
  in the same quadtree, split and merged by the eye's distance alone. Places are read
  from exact tiles only, and the tiles a new brick needs are mostly held. A brick that
  splits or merges loses its markers because the arrangement drops a brick no longer
  drawn before the bricks that cover it are read, not for want of data. Cesium's
  ancestor is the tiles' answer and is already the query's; for the markers the
  matching rule is the one taken above, widened: a leaving brick's markers stand until
  the bricks covering it are read, then are replaced at once, so the coarser and the
  finer are never shown together.

Weighed, with the user: every ancestor need not be held. The tiles keep what holds
now: the asked set closed upward, the held set judged a tile at a time, and a miss
drawn from the finest held tile over it. A held set closed upward would tie what is
shown to the latest ancestor to land, which is why it went on 2026-09-28; asking the
line keeps the ancestors there as a rule without that tie, as Cesium asks ancestors
ahead (`preloadAncestors`) and draws the one it holds without making a child wait for
it. For the markers the fix is the arrangement's, the widened hold above, awaiting the
user's word.

## Then: a tree of barriers

Asked how the triggers of a redraw become an explicit interface, the session counted
about fifty places that ask for a frame and sorted them: the observation (the
camera's pose, the screen's size), the world (tiles landing and bricks filled,
objects loaded and written, models come, edits and their preview, settings applied),
time (the characters, the clouds and the sky, the beacon, a bubble's end, the
camera's ease), and work left (input and output, a pass the budget cut, the GPU's
completions), which redraws nothing and only runs a tick, the renderer drawing
nothing where what it reads is the same. Each place calls `invalidate()` from any
layer, so no kind is written in the code and none has an owner.

The user put it as a tree of barriers, like portals: a barrier that blocks, as the
camera's quantization does, and a barrier that allows again, which opens the
drawing whether or not what is above it blocked.

The session agreed, with two corrections:
- A barrier that allows again opens what is under it, its own region on the screen,
  not the whole drawing; the whole is drawn again only when the root's barrier, the
  observation, passes. The renderer already works so: a stage that reads the same
  input keeps what it made (`Read`), and a draw that changed marks its tiles, which
  alone are drawn again (the masked frame). React has the same two (`React.memo`
  blocks equal props from above, a component's own state draws it again whatever
  its parent did), and so has Flutter (`RepaintBoundary`, `markNeedsPaint`).
- It is a tree of memoized nodes more than a matter of objects: every node has one
  rule, block what comes from above under an equal quantized key, and pass up its
  own change to open what is under it. By the code skill a node is data, its key and
  its last result, not a class.

Mapped: the blocking barriers are the observation's quantization at the root and
each reader's memo key (the location's cell, the markers' `Input`); the barriers
that allow again are the world's changes (`change.rs`, scoped to what they touch)
and time's steps (scoped to their asset); work left is no barrier. Not decided: the
tree drawn from the code, its nodes, keys and the path each change goes up.

## Then: where the real ends and the quantized begins

A design of the redraw's triggers said that an XR head pose must not be quantized.
The user answered that the camera's place is control's, not the observation's, and
asked how the camera, the view, control and input divide, and where along the main
axis the real ends and the quantized begins.

What the session read, against the layers' order (`rule.rs` `CORE_ORDER`: control
and the scene the two heads at 6, the view at 4, the query at 3, the layers at 2):
- **Input**: the events from the UI, the pointer, the keys, a pinch, the device's
  sensors, an XR head's pose (`control/input.rs`, the link). Real.
- **Control**: what turns input into intent and motion, navigation, the controllers
  (orbit, first person, tracked) and their ease (`control/navigation`,
  `control/camera`). Real.
- **The camera**: what control moves, its pose, the eye and its turn
  (`ObserverPose`). Real.
- **The view**: what the camera sees of the world, the observation, and what follows
  from it, the frustum, the lens, the demand, the bricks (`view/frame/pose.rs`,
  `observation.rs`, `lens.rs`, `demand.rs`). Quantized.

The quanta are the main axis's own two lattices: the world's (every size a zoom,
every position an integer: Q3, the tiles, the bricks, the cells) and the eye's (the
camera grid, a cell of a fixed angle, `lens::CELL`, and the visible step a twelfth
of it). The eye setting the detail is the real camera seeing the world through them.
So the boundary is the one place the camera observes, `view_compose`: above it,
what moves, is real; below it, what looks (the observation, the query, the demand,
the bricks, what the scene reads, the renderer's rays) reads the quantized. An asset
that moves on its own steps its real clock by the same eye's lattice (`StepClock`).
The head pose is input; whether a picture stepped by a visible step shows in XR is a
question of the observation's quantum, not of control.

Where the code departs from it: the view snaps the eye to Q3 alone and the turn not
at all; control asks a frame every frame it eases, deciding what the view should;
a pick reads the pose the controller holds (`pose.rs` `ray_of`), where a pick of what
is shown would read the view; and the renderer and the observation compare the pose
and the frustum in floats.

## Then: React's way, taken

The user asked that React's developer experience be taken, its words and the scene
graph's and zustand's, with no word coined. An agent drew a design from the code and
another reviewed it adversarially; the review held the analysis and refused the list
of asks to remove (four were unsafe: `track`'s own ask, and the marker's text, commit
and cancel, which end in no `applied`) and parts of the design. Taken, after it:
- **The root.** One entry, `view_compose`, quantizes the observation: the turn by a
  visible step, the eye's place by a visible step at the ground under it. The camera
  and its controllers stay real; an XR head's pose is input, no exception.
- **Moving.** The frame's motion, which paces the frames and the resolution, is
  whether the observation changed or a stepped motion stepped, in place of whether
  any frame was asked (`frame.rs`, `sched::is_invalid`).

  > Since 2026-11-17: taken back after review, uncommitted. The pacing's motion is
  > whether a frame asked for the next at once, frames drawn back to back, whose gaps
  > are lateness (`rate::judge`, `resolution::load`, perf's `settled`). An
  > animation's step wakes at its own time, so its gap is not lateness; counted as
  > motion, it walked the rate to its floor and the resolution down while the camera
  > stood still. Whether the picture moved and whether frames ran back to back are
  > two facts, and the pacing reads the second; `sched::is_invalid` stays. The asks
  > to be removed later must keep a frame that continues asking for the next.
- **The asks, by their conventional kinds.** A change of state asks a frame
  (`invalidate`, as Android's and as React's `setState`), from its owners: the link's
  `applied`, the objects' `note_damage`, a task's completion. A stepped motion (the
  characters, the clouds and the sky, the beacon) wakes at its next step and marks
  its frame animated (Android's `postInvalidateOnAnimation`). A timeout (a bubble's
  end, the companion's settle, the follow's grace) wakes once and marks nothing, so
  the jitter's sequence runs after it (`setTimeout`). Pending work (input and output,
  tasks) asks where it is now.
- **Animated by draw.** A draw that changed carries whether its change was an
  animation's step, so a step draws its own region alone, as a component's state
  draws its own subtree; the whole frame's one flag goes.
- **What goes.** Only the asks the review found safe. `track` ends in `applied`, the
  marker's edits end in `applied` as the other edits do, and the ghost tells a refused
  write apart from a change. A marker set whose write the heap refused writes again,
  as the zone and the ghost do.
- **Left out.** The observation's cache moved onto `change::Memo` (it would change
  which memos drop), and a quantization in each controller (two entries).
- **The ease.** It wakes at its next visible step, on a wall clock of its own as the
  characters keep, so the frame's step cap does not hold it back.

State: added, a bit a draw and a clock for the ease; removed, `EASE_EPS_RAD` and
`EASE_EPS_U`, about twenty asks, and `animation.rs` (its flag moving to `sched`).
In order, each built, reviewed and committed: the refused writes and the
marker's edits; the safe asks; the steps and timeouts; animated by draw; the root's
quantization and the ease.

## Then: the triggers, as they stand in the code

Asked whether the plan was the conclusion, the session dropped one piece and wrote
the conclusion down. Dropped: animated carried by each draw, which the review
suggested for a frame where a step and another change fall together; no such fault
was seen, and it would add state to every draw.

The triggers, each under its conventional name:
- **`React.memo`, which blocks**: the observation quantized at `view_compose`, and
  each reader's key; an equal key computes nothing again.
- **`setState`, Android's `invalidate`**: `invalidate()`, called by the owner of the
  state that changed: the link's `applied`, the objects' `note_damage`, a task's
  completion.
- **`postInvalidateOnAnimation`**: one function for a stepped motion, waking at its
  next step and marking the frame animated.
- **`setTimeout`**: `invalidate_at`, for a time that passes once.
- **Pending work**: `invalidate()` in sys, the link and the tasks.

No tree is built as an object: the memos and the renderer's damage marks, which draw
a changed draw's region alone, are the tree of barriers. The work gathers the fifty
asks to these owners, and quantizes the root.

The user asked that the problems be divided and done one at a time, each built,
reviewed and committed before the next, since doing them together had brought side
effects. The division: this minute; a note on the experiment's record; the marker
set's refused write; the ghost's refused write told apart from its change; the
marker's edits ending in `applied`; then the editing's own asks removed; `track`
ending in `applied`; the asks inside a frame removed, a place at a time; one function
for a stepped motion, on the beacon alone; then on the characters, the clouds and the
sky, and `animation.rs` gone; a leaving brick's markers held; the root's
quantization, itself divided (the turn, the eye's place, the ease, the pick).
