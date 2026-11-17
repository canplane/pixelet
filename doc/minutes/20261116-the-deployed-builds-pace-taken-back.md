# The deployed build's pace, taken back

**2026-11-16** — recorded with the user, in the session that read why the measured
figures had fallen behind the deployed build of 2026-11-10 (#98, `ac321a51af`), and
then corrected it. The user said this is close to the last profiling and performance
correction: the project is being wound up and no longer has time set aside for it.
Nothing below was measured after the change; the causes were found by reading the code
alone, as the user asked.

## What was compared

The S23's Chrome, release builds with `PXL_PERF_DRIVE=1`, the same drive: #98's report
(`doc/gpu/experiment/20261110-one-command-buffer-an-encoder/report/queue-ask-s23-chrome.txt`)
against #102's at `040487cc5a`, a run another session took that day (`tmp/perf/s23-1`,
not kept).

| | #98 | #102 |
| --- | --- | --- |
| frames in 19.9 s | 300 | 294 |
| GPU p50 / p95, ms | 52.8 / 158.7 | 52.2 / 106.0 |
| tick p50 / p95, ms | 3.9 / 8.9 | 4.5 / 13.1 |
| ticks over budget | 36 (budget 6.1) | 90 (budget 8.0) |
| effects p95 `location`, ms | 4.8 | 8.1 |
| phase p95 visible / interaction, ms | 8.1 / 1.1 | 11.4 / 2.0 |
| steps, mean ms: scene, demand, companion | 2.8, 1.4, 0.4 | 3.6, 1.7, 0.6 |
| terrain tasks, n and p50 ms | 366, 697 | 128, 1139 |

The GPU had not fallen behind; the core's CPU had, every step by about a fifth to a
third alike, and the terrain's tiles came slower. The temperatures of the two runs were
not compared: #98's run has none recorded.

> Since 2026-11-16 (`deb1fbb08c`): measured by turns after the change, the S23's Chrome
> read HEAD's GPU p50 above #98's, at a lower GPU clock for the same use: a frame's
> cycles are 3% fewer on HEAD, the lights' passes traded between them, and the other
> platforms read at #98
> ([the deployed build's pace, measured](../experiment/20261116-the-deployed-builds-pace-measured/README.md)).

Some of #102's fields no longer meant what #98's did, so they did not compare: `paced`
(#98 the paced interval, #102 one frame's gap), `budget` (#98 the turn's deadline, #102
a constant), `refresh` (64 rAF gaps at #98, 16 vsync answers at #102), and `turns`,
`busy`, `rest`, `tasks`, `missed`, `long` and `wakes`, which #102 no longer has.

## What the reading found

- **A clock read became a crossing.** At #98 `now()` was one import, `env.now`. Since
  `a8e1547d6e` and `73e2174df5` it is `monotonic-clock.now` over the link, and on the
  web each call made a DataView, a frame, a reply object and its closure, a Lower, a
  BigInt and more. The core reads the clock once an item in every budgeted list and twice
  a timed step, so every step grew, and the span it timed grew with its own clock read
  and the collections those objects caused.
- **The entry's budget lost #98's rule.** `a8e1547d6e` replaced the turn's deadline (the
  frame's start plus its paced interval less 15%), the visible phase's reserve of 20% for
  content and the share the tasks got after the frame with a flat 8 ms from the entry's
  start. The visible phase ran about 3 ms longer, and past 8 ms a frame admitted one
  terrain answer, so a tile's slot stayed out longer and fewer reads started.
- **The 60 Hz cap went.** Outside `auto` a frame ran on every vsync, and a refused frame
  was tried again at every vsync; on a 120 Hz display that is twice #98's entries.
- **The display was measured at the answer's arrival**, not at the display's timestamp,
  so a frame's length and the answer's delay entered the measure by any amount.
- **The location effect's code had not changed**; its rise is the two causes above.

The commit messages and minutes of 2026-11-12 to 11-15 left the budget's value and the
pacing's effect open, to be measured; none of these was the user's decision (that
minute's rule: what is not attributed was the agents'). The rework those commits made
was the user's: xpute's core and runtimes were deleted to be written again from their
interfaces. What went with them was the policy the runtime had held, and it was not
taken up by the core, nor measured against #98 while the structure was being reworked,
when a change is checked by building it alone.

## The Safari stop

Apart from the figures, the web core stopped at random on WebKit, at boot, with
`RangeError` in the upload belt's `set` (another session ran it: Mac Safari twice,
the iPhone's Safari once, and once a device lost at 18 s that never came back; Chrome
never). The belt came in for the stall of 2026-11-11
([the Safari stall](../gpu/experiment/20261111-the-safari-stall/README.md)) on the web
alone: the native pool was fixed and waited, the web one grew without bound while the
GPU lagged, since only a frame was admitted, and a host kept running records after the
browser had taken the device's mappings away. A short mapped range is what `set`
refuses.

## What the user decided

- **Back to the deployed build, no further.** The aim is #98's performance; the
  corrections do not chase more speed than that.
- **Structurally right, symmetric and reductive.** A fix goes where the fact is owned,
  holds on every platform alike, and differs by platform only in how it is reached.
- **The session decides and fixes it in one pass**, handing the work to the agents that
  analyzed it, reading what each returns and reviewing it adversarially before it goes
  on; the Safari stop is the session's too.
- **By convention, closing the structure**, for the asymmetries the agents met on the way.
- **The layers' orders and the crates' parts stand** (`rust/tool/src/bin/rule.rs`,
  `xpute/ARCHITECTURE.md`); the fixes are tuned within them.

## What the session decided and changed

**The link (xpute).** The user's rule stands, the link carrying every call. A
synchronous answer is written into the frame it came in, on every side and in every
language: one `call(m, tag, args, ret)` in TypeScript, Rust and C++, the side keeping its
reader, writer, frames and views of the guest's memory (made again only when the memory's
buffer changes), the generated bindings writing straight into the frame. What remains a
call is the BigInt of the u64 address and the u64 `mark`, both the user's contracts, and
the copies of string and list arguments. A later answer given mid-crossing is held until
the crossing ends in the TypeScript End and the C++ Side, as the Rust Side held it
(`xpute/ARCHITECTURE.md`: an answer is never run inside the send that asked for it). The
heap's lock exists only where the target can run threads, in Rust and C++, as std's and
dlmalloc's do.

**The frame's pace (pxl-core rate).** `RATE_MAX_HZ` 60 is back, and the frame's target
interval has one owner, `rate`: the most display intervals within one interval at 60 Hz,
at least one, judged in `auto` up to `RATE_MIN_HZ`. The frame task, the resolution's
budget (no tighter than 1/60 s, the user's word of 2026-11-02) and the profiler's `paced`
read it; `paced` is the target again, as at #98, and the gap has its own field.

**The entry's budget (pxl-core sys/sched).** #98's rule, one function: with B the target
less 15% and c 20%, an entry's deadline is the later of the last frame's start plus B and
its own start plus c·B; the visible phase stops c·B sooner; after the frame the rule is
taken again from then. The top (`frame.rs`, `link.rs`) computes the numbers and hands them
down, so `sys` names nothing above it. The profiler's `budget` is the frame's deadline
from its entry's start again.

**The display.** The vsync answers its frame's start, a `mark` on the host's monotonic
clock: on the web the animation frame's timestamp, natively the display link's. The
display's interval is the low quartile of the gaps between those starts, a gap across a
frame being a whole number of intervals; before any is measured it is taken as one
interval at 60 Hz, as #98's runtime took it. A refused frame waits for the next paced
slot, as a skipped one does. The display's 16 gaps are kept, not #98's 64: they follow a
change of refresh four times sooner, and timestamps leave little noise for more samples
to smooth.

**The GPU's belt (pxl-gpu link, guest and both hosts).**
- A mapping gone or refused is a lost device on both hosts: no record runs after it, one
  path whether the browser or the belt finds it.
- A staged write that may wait enters the stream only where the stream and the belt both
  have room for it, by one packing rule (`pxl-gpu-link` `pack.rs`, the TypeScript host's
  copy alike), and a host wakes the guest as chunks come back. Both pools hold the frame
  latency plus one chunk, made when the host is. A write that cannot wait is always
  recorded; a host with no chunk free makes one more for that run and destroys it when
  it comes back. Neither host waits inside a record any more, the native one included.
- An upload larger than a chunk is split once, in the guest's encoder, into records a
  chunk each, and nothing reads the buffer until it is whole.
- A frame is admitted while fewer frames than the latency are unfinished, as at #98;
  staging has its own rule, not the frames' count, and the belt never refuses a frame's
  own writes. A completion that changes what is drawn still asks for a frame; a chunk's
  return and a perf read do not, the reads being taken in the entry they land in. A
  writer that put a write off for want of room asks for one frame once the room that
  refused it comes back: the stream once its run is answered, the belt once more chunks
  are free than the refused stream left.
- Making the renderer's frame cover all its writes in one transaction, so a shortfall
  drops the frame, was weighed and left: the right shape, but past a correction meant
  to reach #98.
- Texture writes are staged through the belt on both hosts, as buffer writes are; the
  web host's synchronous `writeTexture` is gone.
- After a refused ask for a device, the core asks again with a delay doubling from 1 s to
  30 s, and at once when shown again, one rule for the web and native.

## Left open

- Nothing here is measured. The comparison is `script/perf/gate.py` against #98 when the
  user orders it; the deployed build carries no `PXL_PERF_DRIVE`.

  > Since 2026-11-16 (`deb1fbb08c`): measured on the six platforms against #98 and on the
  > S23 against the tree before the change
  > ([the deployed build's pace, measured](../experiment/20261116-the-deployed-builds-pace-measured/README.md)).
- A display whose every frame outlasts one interval and is never refused reads two
  intervals; #98 read it the same way.

  > Since 2026-11-16 (`deb1fbb08c`): the misread feeds the entry's budget, which may then
  > run past the real next vsync and keep the next one late, so it can hold itself while
  > the entries have work; the Mac's Safari read 28 to 32 ms on both builds with the GPU
  > at 21 ms a frame and about 30 frames a second
  > ([the deployed build's pace, measured](../experiment/20261116-the-deployed-builds-pace-measured/README.md)).

  > Since 2026-11-16 (`3ac60df141`), on the user's word: the host tells the display's
  > gaps, as it sees every frame (natively the display link, on the web the animation
  > frame loop), and answers the first vsync from when the core's frame is due; the core
  > no longer reads the interval from the vsyncs it was answered, so a frame that
  > outlasts one interval no longer moves it. Not measured.
- Effects run outside any budget, and `location_step` re-grounds its zone inside one.

  > Since 2026-11-16 (`deb1fbb08c`): settled before this minute.
  > [Closing the scheduler](./20261109-closing-the-scheduler-the-reads-and-the-memory.md)
  > kept the location indicator outside the turn's budget, with the glTF parse and the
  > objects' apply, and the user closed the rest of a recompute's cost, each height's
  > first ray from the root, as over-improvement. The code is the same as at #98.

  > Since 2026-11-16 (`c55e180b1a`): the note above reads more into that minute than
  > it holds. It found the location indicator outside the budget; the user closed one
  > way of cutting a recompute's cost, handing a ray's descent from one height to the
  > next. Moving the recompute under the budget was not weighed, and is open.
  > `place::show` re-asks a ray for each visible place when a place's detail zoom
  > changes, also outside the budget's pass.

  > Since 2026-11-17 (`cfc72c4ac5`, `ff07ca506e`): on the user's word, both are asked
  > under the budget (`sched::run`), and a pin crowd's stand with them. The zone is
  > laid once every cell of a table of its cells' ground has been asked, the last laid
  > shown till then; a place stands on its last surface until its own is asked.
  > Not measured.

  > Since 2026-11-17: taken back on the user's word, the location and the markers as at
  > `bec3061dc4`
  > ([the location zone, by the code's own axioms](./20261117-the-location-zone-by-the-codes-own-axioms.md)).
- The C++ heap's branch for wasm without atomics has not been compiled.

  > Since 2026-11-16 (`deb1fbb08c`): it is the branch `make -C xpute/cpp wasm` compiles,
  > for `wasm32-wasip1` without `-matomics`, and it compiles; the branch no target
  > compiles is wasm with atomics.
