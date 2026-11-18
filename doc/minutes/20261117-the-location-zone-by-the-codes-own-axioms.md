# The location zone, and the observation quantized

**2026-11-17** — recorded with the user. The meeting began with the location
indicator's cost after the night's change (`cfc72c4ac5`, `ff07ca506e`, `6de67851df`;
[the deployed build's pace, measured](../experiment/20261116-the-deployed-builds-pace-measured/README.md),
"Then: the night's quick comparison") and ended with one law. Why the night's work
went wrong is
[an agent builds around the cost](../lesson/an-agent-builds-around-the-cost.md).

## The law

The user's, in one sentence: **the screen holds its last observation, and changes it
only when a new one, different enough to tell apart, is ready.**

Its two conditions carry every item of the meeting.

- **Different enough to tell apart.** Whatever moves, the camera, an asset's own
  clock, the world, moves in reals. What is seen of it is an observation, sampled by a
  grid, and a difference the grid cannot tell is no change (Nyquist). The same rule
  already decides what is drawn (the camera grid's cell, on which the level of detail
  is built) and how often an asset's own motion steps; it also decides when the screen
  is drawn again. It is the main axis's first rule, the eye sets the detail, carried
  from space into change.
- **Ready.** Until the new is there, the old stands; nothing vanishes meanwhile. It is
  the main axis's other rule, a coarser answer stands in until the finer one arrives,
  kept for everything shown.

## What follows from it

- **The real and the quantized.** Input, control and the camera are real: the
  camera's place is control's, and the camera observes the world, never the reverse.
  The view, what the camera sees, is the quantized side, made at one entry, and
  everything that sees reads it, the screen's rays included. The layers' order and the
  main axis stand as they are.
- **What draws the screen again.** The observation is the root: while it stands the
  whole stands, and when it changes the whole is drawn. An asset's own motion and a
  change of the world each open their own region, whatever the root did. Nothing else
  draws. The shape is React's (a memo that blocks, a state that draws its own subtree
  again) in the words convention already has; no tree is built as an object, since the
  memos and the renderer's marks of what changed are the tree.
- **The screen's rays behind the root.** Cesium draws on any change of the camera
  because it rasterizes and keeps nothing between frames. Pixelet casts a ray a pixel,
  its costliest step, and remembers what it found, so the barrier belongs before the
  cast. A convention holds as far as its premises do.
- **The quantum.** Half the spacing of whatever samples the stage: the camera grid's
  cell for what reads the world, the step in time for a motion, the viewer's eye (the
  CSS reference pixel, a visual angle) for the picture.
- **The tiles' ancestors** need not all be held. The demand asks each line up to its
  top and each tile is judged on its own, a miss drawn from the finest tile held over
  it; a parent required for a child would make the picture wait on the latest to land.
- **The location's zone** is the fix's cell, quantized, the cells about it at fixed
  offsets, each set on the support query under it. A zone at the same cell queries
  nothing again, and the indicator runs as its peer the ghost does.

## The code

Kept by the law before the meeting: the level of detail, and a coarser answer of the
ground standing in for a finer one not held, which the query gives (`Answer::Coarser`).

Brought under it at the meeting:
- **The location's zone** (`600037b61c`, `aff9dfde09`, `8cb606094e`): keyed by the
  fix's cell, run as the ghost runs; its center is grounded each step, the one query
  that tells where it is, and its cells are laid only in the frames that draw it.
  That a zone at the same cell asks nothing again held only where the ground's leaf
  was held: far from the eye any tile landing dropped the grounding, and on the S23
  the step cost a millisecond a frame for a zone not drawn.
- **What stands while it is read again** (`63f0ec4499`, `64e4fae492`, `948b7b3889`,
  `bbd6b5e5af`, `8b39a6b078`): a block the heap refused is written again from what is
  held, the zone's, the marker set's and the debug meshes' as the ghost's; a marker
  stands until the drawn brick over it is read, across a split or a merge, a brick's
  places read whole or not at all; a place is named by the ground cell it stands in,
  so its drawing is held whatever tile it is read from.
- **The camera** (`93a64f5181`, `8b39a6b078`, `9fd850c654`, `d753e720dc`): the view
  takes a turn only once it moves the screen by half a CSS pixel; the pick, the world
  gizmo's heading and the UI's compass read the view; the camera's ease ends where its
  pose and the one it eases toward are not distinct in the view, by the one test the
  view takes its pose by, and the two ends chosen by eye are gone.
- **The places' asks of the ground** (`e77e5f8a03`) run under the entry's budget, the
  decision of 2026-10-28: the loop's body for one place handed to the pass every other
  budgeted work uses, a place read again standing meanwhile on the surface the same
  place stood on.
- **What moves on its own** (`59f9975f42`): the characters, the beacon and the clouds
  read their clocks in one place, which wakes the frame at the next step and records
  it.
- **The triggers** (`6102908d4e`, `8e8f7f5f8c`): an ask for a frame says its reason
  (`InvalidationReason`: a method applied, the view, the scene, a completion, pending
  work, an animation), counted by the scheduler and read by the profiler. Who asks
  follows where the trigger comes from. A method from outside asks as a method, in
  the one place the interfaces are implemented (`link/export.rs` `with_frame`): every
  method that changes what the core holds, none that only reads, and the device's
  heading alone by a condition, only while the camera follows, since the device tells
  it far more often than a frame is worth. What changes inside the core asks where it
  changes. A rule tried in between (`b8c835dcc8`), that an event asks once, so a
  method whose state asks adds none, was taken back: it made who asks depend on
  whether a state happened to ask, and an ask is a flag, so two draw one frame.
- **A walker stands where a decor of its cell is drawn** (`68f5c3828e`, `e88d2fa7d3`,
  `ec8803500f`): the step-slide walk the pan and the companions share ended only at a
  leaf not held, which never comes where the world answers without leaves, past the
  service area and over a childless tile of sea, and from the orbit's largest distance
  a drag stopped single frames for 100 to 560 ms. The eye had closed that on
  2026-10-11 (far out the world was a map) and `5b054bb5fd` of 2026-11-09 opened it.
  The user's rule: a placed cube has the ground's level of detail, and a glTF mesh
  placed in the pyramid has a decor's. A walker claims one box, `BODY_BOX`, `[1, 1, 2]` leaf
  cells, so it stands where a decor of that box is drawn, out to 192 units from the
  eye. There alone its
  character is drawn, which an angle about a sphere decided before (out to 768 times
  the model's radius) and its height, the spec's, decides now; there alone the chat's line hangs over it, which a marker's
  zooms decided before (out to 3072 units); and a walk is walked only where the
  walker stands at both its ends, which the walk itself measures, so it is under 384
  leaf cells an axis for any input. A pan's walk and a companion's take no eye of
  their own: a walker is walked where the eye draws one. Any other move ends where
  it is sent with its height unknown, one walled staying rather than leave the area;
  a destination is put once a tile held gives its ground exactly, which is no read. Tried on the way and taken back: the walk crossing an empty shard in one step
  (`18598cc6e8`), which closed one of the two cases by a second path; the rule of
  2026-10-11 put back as it was, which tested the walk's start alone by the drawn eye
  while the pan's length read the distance asked; a walk over the cells of the zoom
  drawn, which the walker's floors do not hold where a cell is larger than the
  walker; the ground's leaf zoom alone (96 units), which stopped the walk where the
  character was still drawn; an angle of the walker's own size; the clip level's
  reach (384), where a decor is generated and not drawn; and the distance the orbit
  is asked to stand at, which the terrain and a zoom's ease hold apart from where
  the eye is.
- **The floor under a point in a coarser cell** (`c02a80d07b`): a ray down never stops
  in the cell it starts in, so from over the box a coarser cell is drawn as, within
  that cell's cube, the floor answered was the cell below's, and a character on
  coarser ground was drawn sunk a cell. The climb already read that top and dropped
  it; it hands it on.
- **The climb's rays read by their distance, as every ray does**: the engine's walk
  ends at a cap of steps, 4096, a guard for the GPU that the CPU's walk shares since
  2026-11-05, and out of steps it ends as gone to its reach. The climb out of what
  fills a walker's place read the finest detail at every distance, to the top of the
  world, and read a ray out of steps as solid all the way: from more than about 2040
  leaf cells of solid, 4.9 km, the floor answered was the world's top. The user's
  rule, held for the camera, the light and the shadows from the start: what reads the
  world is isotropic and by perspective, the farther the coarser, and what a ray did
  not find within the cap is not there. Control's rays now read so, the
  walker's, the orbit's way and the pointer's, where each read the finest zoom at
  every distance. They are the one walk every reader takes, and near and far are
  told from the eye and from nothing else, wherever a line starts: the eye sets the
  size a place is asked at, the eye's own rays ask each node at its own, a walker
  asks its cell at the one size the eye sets for it, and with no eye a read answers
  a lack
  ([the eye sets a size, and the world answers](./20261118-the-eye-sets-a-size-and-the-world-answers.md)).
  What a ray meets far off is a coarser cell, and that is its answer: nothing reads
  it again from nearer, which would be a second rule.
  From far a gap too thin to tell is passed, as an eye passes it. A
  ray reports the cells it read as far as it went, at the detail it asked there,
  where a ray by distance reported its coarsest zoom's cells whole, 2048 units a side
  for one column. A walk that ended at tiles not held answers a lack, where it
  answered as one that went its reach over coarser ones, and the climb goes over
  ground a coarser tile stands in for as over ground held. Tried on the way and
  dropped on the user's word: the finest detail kept and the ray's length held to a
  count of cells derived from the cap, with a loop over reaches, which was a second
  rule beside the one every other ray reads by.
- **A ray into ground not held.** West of a held leaf the coarser tiles of its line
  stand in, out to the edge of the coarsest, and past that edge every reader answers
  a lack; so a ray that leaves the held tile ends at a lack only there, and the
  climb's reading of `Coarser(None, _)` as filled stands.
- **A bound read through a stand-in.** A ray that passed whole a node wider than a
  brick answered exact over ground no leaf held, by a bound a coarser clip level
  stood in for. `clipmap::stood_in` says whether a node's bound is read so;
  `TileCache::relief` then answers it coarser, and a CPU ray asks the nodes under
  it. The bound itself, `bound_over` and the shaders stand as they were: a block
  with no bound over a brick not yet held costs ten times the nodes at the clip
  levels' edge, and a coarser clip level answering in a brick's place is the rule a
  cell follows.
- **One floor under the orbit's eye**: the orbit reads `World::floor_under` whether
  or not the sky is open over the pivot, so a climb's eye may come to rest in a cave
  under a hill.
- **One fact, one name** (`c95305007c`): a decor opens into its model at the grid's
  `SPANS`, as the ground's cell opens into its finer ones.

State the meeting added: the bricks whose place tile is still to come, the scheduler's
counts by reason, a view's pose that may differ from the camera's by less than can be
told apart, and a place showing by what it was handed. State it removed: the
ease's two ends, the smoothing's `eps` and its flag, the location's two-fold key, the effect that did real
work, the memo cleared on a refused block, the pick's own pose, a decor's constant, a
place's tile and index, and the asks for a frame written in each method's body.

Measured: the Mac's Chrome, by turns with the tree before, five times over the day,
read the same (tick p50 1.8 ms; the GPU within the runs' own spread; the runs late in a
series slower on both builds). The S23's Chrome against the deployed build #98 drew a
tenth fewer frames, as the day before, its GPU's p95 and longest frame the lower; and
with the location's zone laid only while drawn its tick fell from 5.9 to 7.1 ms at
p50 to 4.8 to 5.8, and from 13.1 to 14.1 at p95 to 10.1 to 11.4
([the experiment](../experiment/20261116-the-deployed-builds-pace-measured/README.md)).
The iPhone was not measured.

Control's rays as the one walk were seen in the app on the Mac's Chrome by the user
and found without fault, and measured against the tree before them
([the day's structure against the tree before](../experiment/20261118-the-days-structure-against-the-tree-before/README.md)):
the Mac's Chrome passes the gate. The S23's rounds parted nothing, the phone hot, and
the form that stands, its GPU's walk the base's, was not measured there.

Not pushed, not deployed.

What stands as it is, and why:
- **A place stands on the ground as drawn**, asked again when the detail drawn where
  it is changes. Cesium's clamped label reads the finest ground loaded, once, and is
  drawn over the ground whatever its depth; a marker here is tested against the
  world's depth at its anchor (pxl-render `pass/marker.rs`), so the premise differs.
  The loop this made, a height telling the cell that asked for the height, is cut
  (`685dbeff31`): where a place is and what it stands on are two facts, the first
  deciding the second.
- **A pins' count is asked where it stands at once, outside the budget**, where a
  place's surface is asked under it: the same contract, that nothing shown goes while
  it is asked, kept by two forms. A place is named by its cell and stands meanwhile on
  what it held; a count is named by the pins' cell, which changes with the zoom, so
  nothing shown before can stand for it, and its ray ends within that cell. Putting it
  under the budget was tried twice, the night's and at the meeting, and taken back:
  the count went and came back.
- **A motion's step is its own quantum** (`VISIBLE_STEP_CELLS`, a twelfth of a cell),
  not the view's turn. The clouds drift in the sky's own frame and the beacon bobs in
  the world's, and what steps them is time, the third sampler; the view's turn step is
  the picture's. Putting the one in the other's place was tried and taken back as
  forcing unlike roles alike: it changed no frame of a moving camera and would step a
  still screen 1.8 to 2.5 times as often.
- **`DECOR_IMPOSTOR_CELLS`** stays a quarter of a cell, chosen by eye, where the
  grid's quantum is half. Seen side by side on the Mac's Chrome at three zooms over
  the ground, a quarter and a half drew the same picture: a decor leaves by its clip
  level's zooms before this angle is reached. The value also sets how far the clouds
  are drawn, whether a character is seen, and the size of the fixed ranges
  (`manifest.json5` `fixed_sz`), so changing it moves those for no gain shown.
- The effects were read and do no real work (0.1 ms at p95).

## The way of working

The user's: what simplifies the structure, keeps a symmetry or clears the state space
is done, and never at the cost of a regression in pace. A task is divided, one problem
finished before the next. An agent's verdict is judged by the law and refused where it
departs. A fault met is first asked whether an ad hoc piece made it, and the piece
goes rather than the fault being patched. A minute holds conclusions, written whole.
These are carried into `AGENTS.md`, the code skill and `doc/README.md`.
