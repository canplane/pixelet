# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in the
discussion the line points to. A finished item is removed, not ticked — its
record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so it
is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **The S23's drive, hot, after the batch of 11-06.** Main 034ddad063 ran 10.7 fps at
  a 550 MHz cap, worse than before the batch, the picture broken and never at rest.
  The causes are found and their fixes in flight: stale record entries taken as
  answers (mid-distance slivers, faces against solid near the ground), the light's
  stand and propagation unbounded (~70 ms a light cast), the cumulus's drifting
  stand recasting the light and ungating the frame at rest, "not read back" taken as
  "owed". Open: the bright line at the far horizon (the walk's step bound or the
  budget). Then one table for the node cache and the records (out/one-table.md) —
  [the journal](./experiment/20261105-the-frame-on-the-s23/journal.md),
  [the minutes](./minutes/20261106-the-frame-is-what-it-must-draw.md).
- **Safari pins its GPU with shadows on.** Since 9e476eca74 every pipeline compiles
  on Safari 27; with shadows on the GPU sits at 100% and the run stops, and every
  phase logs "executePreDispatchCommands - could not track resource". Seen before
  the indirect-scope fix (034ddad063); recheck on main once the S23's fixes land.
- **A gate that every pass's access is declared and its hazards checked at build**
  (decided 11-06): each binding's role and access in its record, the build refusing an
  indirect read of args bound writable, two writers of a word, a read before its write;
  the frame kinds and owed/rest as a pure state machine explored whole; a whole-frame
  run under validation.

## P1

- **The render scale, pixel-bound or ray-bound.** The S23 at 0.75 and 0.5 tells
  which passes scale with pixels; then whether a lower scale is the way to 60 fps
  with shadows off. A quality decision with the user.
- **`auto` over the frame's interval too.** It is congestion control already (the
  user, [20261102-auto-judges-the-pixels-and-the-lattice.md](./minutes/20261102-auto-judges-the-pixels-and-the-lattice.md)),
  over the resolution and the lattice. The third knob is the rate frames go in at, a
  divisor of the refresh (`frame_pacing`), against the queue of casts in flight as a
  signal beside the GPU's busy time. Its name on the screen, "auto resolution", no
  longer says what it moves. **The lattice knob goes if it earns nothing**: first
  lattice 1 against 2 at the same scale at a device pixel ratio of 2; then remove it,
  with `LATTICE_MAX`, `gpu_lattice` and the judge's G.
- **The character preview does not show.** It did about three days before 11-05; a
  comparison with that build finds what lost it.

## P2

- **The vocabulary, cleaned.** Agents have left words with several meanings
  (`level`, `chunk`, `frame`), several words for one thing, and names of what
  happened. In order: every domain noun listed with what it names (read-only, by
  area), a glossary decided with the user, conventional names first and cited, put
  in the naming skill; then the renames by area, one names commit each.
- **Lighting run under decor.** A depth-only screen draw with a coverage bit would
  skip it, but the stencil's 8 bits are the gate's; first measure the share of
  screen pixels decor covers.
- **The character stands half inside a step at load.** At 38.1193, 128.4656 its
  right half is cut at the screen's middle column, by the wall of the higher cell
  next to it (`p_cut` in the journal's record). Why it stands there is open.
- **How strong a shadow is at dawn on flat ground.** A full shadow takes about 21%
  off at 5° of sun (`SHADOW_BY_UP_TAKEN`, `LIGHT_BY_NORMAL`), and terrain already
  shades most near ground then. A look decision.
- **A login screen is to come.** Another kind of `Cover`, over `Starting` or
  `Loading`, so the world is made while it stands —
  [20261103-what-stands-over-the-app.md](./minutes/20261103-what-stands-over-the-app.md).

## Parked, with the reason on file

- **Inside a ray: sibling loads, ESVO's child masks, subgroups.** The holes' queue
  (persistent lanes, rays parked, nodes asked once) landed 11-06; the rolled lattice's
  beams and samples still run outside it. What is left of the wavefront line is
  exposing a ray's own parallelism; whether subgroups are offered on the S23 and on
  Safari is unchecked. After the lattice joins the queue.
- **A query over what is drawn, for an agent to read.** An agent debugging the
  view guesses a pixel's cause from its color in a screenshot. The profiler's
  half is answered: the console's `pxl.profiler.report()` gives its window as
  text (`ts/client/src/shell/debug/console.ts`). What a screen point draws is
  not: the block its ray met, with its face, zoom and distance, and where that
  block was read from, asked of the running page and answered as text. The
  answer is the GPU's, the ray's words read back (`pxl-engine ray.rs`), since
  the CPU's `first_hit` (`pxl-kernel domain/query/ray.rs`) can disagree where
  the samples miss a thin face
  ([20261024-a-proxy-only-starts-the-ray.md](./minutes/20261024-a-proxy-only-starts-the-ray.md)).
  Not urgent: no debugging under way needs it.
- **WASI, as a subsystem over the contract.** An adapter that offers a WASI
  program WASI's calls translated into the contract's requests, which is what
  would let an existing program in. Not without one to build it against; guests
  stay `wasm32-unknown-unknown` meanwhile —
  [20260930-wasi-and-the-boundary.md](./minutes/20260930-wasi-and-the-boundary.md).
