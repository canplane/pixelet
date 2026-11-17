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

Kept by the law: the level of detail; the location's zone (`600037b61c`,
`aff9dfde09`); a block the heap refused, written again from what is held, and a
brick's markers standing until the brick is read again (`63f0ec4499`, `64e4fae492`).

Brought under it at the meeting:
- **A brick that splits or merges** (`948b7b3889`): a marker stands until the drawn
  brick over it is read, and a brick's places are read whole or not at all, what
  stood staying while a tile of theirs is still to come.
- **The camera** (`93a64f5181`): the view takes a turn only once it moves the screen
  by half a CSS pixel, and the pick is cast from the view. The eye's place was
  already taken by the lattice the rays start from.
- **What moves on its own** (`59f9975f42`): the characters, the beacon and the clouds
  read their clocks in one place, which wakes the frame at the next step and records
  it.
- **One fact, one name**: a decor opens into its model where the ground's cell opens
  into its finer ones, so the decor's own constant went for the grid's.

The Mac's Chrome, by turns with the tree before the camera and the markers changed,
read the same: tick p50 1.8 ms, GPU p50 20.6 to 21.0 ms, 839 to 850 frames. The
phones were not measured.

Where the code still departs from the law: the camera's controllers ask a frame every
frame they ease, where an asset wakes at its next step; the eye's place and the turn
are taken by two quanta, the lattice's and the eye's; the steps and the ease's ends
chosen by eye (`VISIBLE_STEP_CELLS`, `EASE_EPS_RAD`, `EASE_EPS_U`,
`DECOR_IMPOSTOR_CELLS`) are not yet the rule's values; the asks for a frame do not
name their kind; a place's surface and a pin crowd's stand are asked outside the
budget; and the debug gizmo and the compass read the camera, not the view.

## The way of working

The user's: what simplifies the structure, keeps a symmetry or clears the state space
is done, and never at the cost of a regression in pace. A task is divided, one problem
finished before the next. An agent's verdict is judged by the law and refused where it
departs. A minute holds conclusions, written whole. These are carried into `AGENTS.md`,
the code skill and `doc/README.md`.
