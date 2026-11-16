# Figma's turn and the ring's

**2026-11-12** — recorded with the user. Main was at `58b248d9c8` when this was
written. The user asked which side holds the turn in Figma, where the UI and the
wasm share the main thread: whether the JavaScript enters the C++ each frame, or
the C++ calls out to the JavaScript. Then the user asked how that compares with
pixelet's turn on rings, as it stood at `3cccd2cf7a` (2026-09-28), before the rings
and the turn were taken apart in November. What was observed is in
[the experiment](../experiment/20261112-figma-s-frame-loop.md). An outside review
the user brought in is folded in below where it changed what is claimed.
[The mechanism minutes](../../minutes/20260914-mechanism-and-policy-and-what-xpute-is.md)
said no account of Figma's scheduler was at hand. This is one.

## Figma's answer

The JavaScript holds the turn. A `requestAnimationFrame` callback of the app's
calls the C++'s `onFrame()`. The C++ asks for its next frame through an import,
`Async.requestAnimationFrame`, and that request joins the UI's own requests in one
function. The C++ also hands work to the browser's scheduler: `scheduler.postTask`
at three priorities and `setTimeout`. The browser decides when each piece handed
over runs. Whether the C++ also orders or splits its work by a policy of its own,
or limits its time inside a frame, was not seen.

## Set against the ring, at 3cccd2cf7a

At that commit the core was on the page's main thread too. The shell's `Strobe`
(`@xpute/runtime sched/strobe.ts`) ran a turn on an animation frame, and only when
asked:

- by the host, for an input or an arrival;
- by the core, through the value its last turn returned.

To run a turn, the host:

1. took a grant (`quantum.ts`);
2. wrote the frame's `WORLD_UPDATE` into the submission ring;
3. called the one export, `interrupt(quota_ms)`.

Inside the turn, the core:

1. took what waited on the ring;
2. ran its own tick, its phases in their fixed order against the quota
   (`xpute-runtime sched/tick.rs`);
3. recorded its host calls into a stream, and on a page its GPU calls into another;
4. returned when it wanted the next turn: 0 for the next frame, a delay in ms, or
   none.

Then the host:

1. ran the two streams;
2. read the completion ring.

On a device the core drew through wgpu inside the turn, and recorded no GPU
stream (pxl-native `lib.rs`). The table below is the page's.

The host also entered the core outside the frame, with a quota of 0:

- `ring_now`, which applied a shell command or a sensor's open and close at once;
- `Doorbell.call`, rung until its reply came back, so the host had the answer
  inside the call, as for the boot's `WORLD_CREATE`, a surface's create and
  delete, and the profiler's reads;
- a submission ring that refused a command for want of room, rung until it took
  it;
- a completion ring that came back full, rung again until it did not.

| | Figma | the ring, 3cccd2cf7a, on a page |
| --- | --- | --- |
| who enters | the app's rAF callback calls `onFrame()`; any task may call one of 5,092 C++ functions, the `WebSocket`'s `receive` among them | `Strobe`'s rAF callback calls `interrupt(quota_ms)`; outside the frame, `interrupt(0)` |
| what goes in | nothing | a quota, and the frame's command on the ring |
| the next frame | an import call, `Async.requestAnimationFrame`; when the C++ makes it not seen | the turn's return value |
| the UI's frame | the same callback: the C++, the other views, redux and the listeners | its own; the core's snapshot reached React as a signal read after the turn |
| work outside the frame | handed to the browser: `postTask` at three priorities, `setTimeout`; any policy inside the C++ not seen | the core's own: tasks and phases in the turn under the host's quota; a delay returned for a timer |
| time | not passed in; the frame loop's callback, `onFrame` with the rest, measured 0.4 to 24 ms; a `WebSocket` task that hands its bytes to the C++ 1,172 ms | a quota sized from the measured frame interval; an overrun came off the next one |
| calls out during the turn | WebGPU, synchronously, seen on the stack; the binding objects are imports, their calls not seen on a stack | none but the clock; GPU and host calls recorded, then run by the host after the turn |
| a result from the host | a synchronous import's at once, inside the call; a task's or a timer's later, through its promise or `timeoutCallback` | the next turn, once the host ran the stream |
| idle | nothing drawn in two seconds; whether a frame ran without drawing not seen | no wake value, so no turn runs |

Both give the main thread back after every entry, and neither runs a loop of the
C++'s own. They differ in what crosses:

- Figma's boundary is a call graph. The C++ calls out through its imports, the
  frame is a function with no arguments, and its tasks are handed to the browser's
  priorities.
- The ring's boundary was a turn. The host handed in time, and priority was the
  core's own phases inside that time. On a page the core called nothing out until
  it returned, and only the host called in outside the frame, through
  `interrupt(0)`.

## How long an entry may run

The user's question was how long the C++ may keep the thread, and when it gives
it back. That is what the quota at `3cccd2cf7a` was for, while the core shared
the page's thread.

The browser gives no budget to a call already running, and it cannot stop one.
What each of its APIs gives is this:

- `requestAnimationFrame` gives the end of the last frame's rendering as a
  timestamp, and no deadline
  ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame)).
- `scheduler.yield` hands the thread back where the code awaits it and continues
  in a new task. It does not preempt
  ([Chrome](https://developer.chrome.com/blog/use-scheduler-yield)).
- `IdleDeadline.timeRemaining`, in an idle callback, estimates the milliseconds
  left in the idle period. It can drop to 0
  ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/IdleDeadline/timeRemaining)).

So a quota is a number the program sets for itself.

Figma passes none into `onFrame`. Measured
([the experiment](../experiment/20261112-figma-s-frame-loop.md)):

- the frame loop's callback, which holds `onFrame` and the rest of the frame, took
  0.4 to 24 ms and ended at no one time after the frame's timestamp;
- a `WebSocket` task that logs its message and hands the bytes to the C++ took
  1,172 ms.

These bound the C++'s share from above. Whether the C++ holds itself to a budget
in a frame is not known. That the thread was held for over a second in one task
whose work is handing bytes to the C++ is known.

What Figma has published says no more:

- To plugin authors, Figma advises doing a fixed number of nodes, or a few
  milliseconds of work, then giving the thread back through `setTimeout`. It gives
  no number, and says nothing of its own engine
  ([frozen plugins](https://developers.figma.com/docs/plugins/frozen-plugins/)).
- Its client's runtime systems once scheduled themselves ad hoc. A shared
  orchestration layer now runs them in one order: materialize, then variables,
  then layout. It uses push-based invalidation and dependencies recorded as they
  are read. The post does not say whether this is the C++, and gives no time
  ([component instances, 2026-03-17](https://www.figma.com/blog/how-we-rebuilt-the-foundations-of-component-instances/)).
- A file's incremental load is split into smaller chunks. The layout and boolean
  metadata of what is not shown are skipped. The post does not say how a chunk is
  resumed
  ([incremental frame loading, 2024-01-23](https://www.figma.com/blog/incremental-frame-loading/)).

## Where the C++ gives the thread back: a hypothesis

The user's reading: Figma does not lay explicit yields through its work. It
leaves asynchronous checkpoints between steps and lets the event loop decide what
runs next. This is a hypothesis about Figma's inside, not something seen.

What fits it:

- `onFrame` takes no quota (the experiment);
- the C++'s imports for a frame, a task or a timer put in a request and return at
  once (the experiment); whether the C++ then returns was not seen;
- while the larger file loaded, the frame loop's callbacks ended at no one time
  after the frame's timestamp (the experiment);
- one task that hands bytes to the C++ held the thread for over a second (the
  experiment);
- a file's incremental load is split into chunks, with no word on how a chunk is
  resumed (Figma's post, above).

What could count against it: the C++ read `performance.now` hundreds to thousands
of times in a frame, and whether any of those reads decides when it returns was
not determined.

A checkpoint can be one of two kinds:

- **at the end of a step**: a step finishes, schedules the next, and returns. A
  parse that schedules layout would be one; no such case was seen in Figma. No
  budget is needed.
- **inside unfinished work**: a loop checks a condition between items, by time or
  by count, schedules its continuation and returns. That is cooperative yielding
  proper.

The thread comes back only when the entry unwinds and its task ends. A C++ that
calls `postTask` or takes a promise and goes on computing still holds it. Nothing
could suspend the C++'s stack instead: the glue has neither Emscripten's Asyncify
nor JSPI (the experiment).

`3cccd2cf7a` was of the second kind. Its tick ran a step to completion, its steps
small by construction, and stopped between them by time or by a count of steps
(`xpute-runtime sched/tick.rs`, `sched/frame_budget.rs`). What set its time was a
quota the host granted. The two share the limit: a step that runs for a second
holds the thread for a second, there as here.

## Why Figma stays on the main thread, and pixelet left it

The user asked why Figma stays cooperative when pixelet moved its core off the
UI's thread, and whether that binds Figma to a web view.

**What was seen.** The app and the C++ call each other synchronously:

- the app can call 5,092 C++ functions;
- the C++ calls 134 binding objects of the app's.

Each entry runs to completion and returns. A task whose work is handing bytes to
the C++ held the thread for over a second. The 5,092
are entries that can be called, not tasks, and not all of them are the UI reading
state. No common scheduler over every entry was seen.

**What Figma says.** A file's load could not move to a worker, since the state it
reads and writes is on the main thread alone. The load is split into chunks
instead ([incremental frame loading](https://www.figma.com/blog/incremental-frame-loading/)).

**A guess, not checked.** Figma began before a canvas could be drawn from a
worker, and its synchronous calls grew on that. How its first design was decided
is not known.

**The platforms.** The editor is bound to the browser. The C++ is not:

- the same C++ renderer is compiled to a native x64/arm64 program for rendering
  on the server, testing and debugging
  ([WebGPU](https://www.figma.com/blog/figma-rendering-powered-by-webgpu/), 2025-09-18);
- Electron's showcase lists the desktop app as an Electron app; no Figma page read
  says so ([Electron apps](https://www.electronjs.org/apps));
- Figma Design files and Slides decks cannot be edited in the mobile app, and a
  phone's browser gets a view-only version. The iPad app edits FigJam boards
  ([mobile app](https://help.figma.com/hc/en-us/articles/1500007537281-Guide-to-the-Figma-mobile-app),
  [system requirements](https://help.figma.com/hc/en-us/articles/360039827194-What-browsers-does-Figma-support)).

So the Figma Design editor runs only where a browser or Electron runs it, and
does not have its threads laid out alike across native platforms.

**Pixelet's case.** The same core has to run in real time on a page and on a
device. A device shows what a browser hides
([the runtime leaves the UI's thread](../../minutes/20261109-the-runtime-leaves-the-ui-thread.md)).

- On a device, the core runs on a thread of its own.
- On a page, it runs in a worker.

The two platforms enter the core by different mechanisms, but the call contract
the core depends on is the same on both.

## Set against what replaced the ring

From `f59a933e21` (2026-11-11) to main:

- the rings are one crossing each way, the runtime's `link_in` into the core and
  the core's `link_out` (`afd0294867`), and what crosses is a method called on an
  object opened by URI (`2072b18c6b`);
- the runtime enters the core only with a request or an answer (`edda87cf28`);
- the core asks for its next entry through `timer.sleep(ms)`, served on
  `setTimeout`, or xpute-gpu's vsync, served on rAF (`3544bcd81d`);
- there is no quota, wake value or scheduler in xpute;
- the core keeps its tick and phases on embassy's executor, with a budget of its
  own for each entry (`BUDGET_MS`) and `yield_if_spent` to leave it
  (`3e44a1a8cd`, pxl-core `sys/sched.rs`);

  > Since 2026-11-16 (`deb1fbb08c`): an entry's budget runs from the frame's start
  > and its paced interval, and the vsync answers its timestamp
  > ([the deployed build's pace, taken back](../../minutes/20261116-the-deployed-builds-pace-taken-back.md)).
- on a page the core runs in a worker (`4d95242546`).

At `3cccd2cf7a` two time policies were laid one on the other: the host's quota
and the core's budget against it. Now there is one, the core's. xpute connects
calls and no longer runs turns, and the policy is pixelet's.

At the boundary this is Figma's shape: the runtime is the mechanism that enters,
and the core asks for its own entries. Two things still differ:

- Figma's C++ shares the UI's thread and frame. Pixelet's core shares neither, so
  its work does not stop the UI, and what crosses between them is a message.
- Figma's C++ hands tasks to the browser at its priorities. Pixelet's core asks
  only for a vsync and a timer, and orders its work by its own phases and budget.

That the boundary matches says nothing yet of responsiveness. That waits on the
core running again and on measurements on the devices.

## The same question asked of pixelet's workload

The user asked whether pixelet could have bound its stack as Figma does: the core
on the page's main thread, the UI and the core calling each other synchronously,
each entry run to completion. This was read from the records and the code; nothing
was run. Every core figure below predates the link, from builds with the turn and
the quota, and says nothing of `HEAD`.

| where | what each frame must run, measured | what holds it up |
| --- | --- | --- |
| Mac Chrome | the core busy 3.9 ms at p95; tick 2.0, effects 0.9, stage 0.5 at p95 (`doc/experiment/20261107-the-lights-tail/report/mac-base`) | in range of Figma's frame |
| S23 Chrome | the core busy 13.8 ms at p50, 27.4 at p95, 31.6 at most, in intervals 38 ms apart at p50 since the GPU held it near 13 fps (`doc/experiment/20261109-the-app-that-did-not-move/report/s23-web.txt`) | half the intervals within a 60 Hz frame, one in twenty past 27 ms; none past 50 ms |
| the S23 app | wgpu's present waited up to 397 ms ([the runtime leaves the UI's thread](../../minutes/20261109-the-runtime-leaves-the-ui-thread.md)) | the one thread a stack could be shared on is React Native's JavaScript thread, with the GPU on its own; never built or measured |

- **The page on a desktop could have held it.** The core was on the page's main
  thread until November. What held that thread long then were steps that ran
  outside the budget, places' `show` at 125 to 140 ms among them
  ([the turn holds](../../minutes/20261028-the-turn-holds-and-what-runs-outside-it.md)).
  It runs under the budget now. The steps still run whole in a frame are the location
  indicator's recompute, the objects' shard apply, the pointer's hover ray, the
  companion and the renderer's recording. The shard apply grows with its data and
  is not measured; it is the nearest to Figma's 1,172 ms task.
- **A phone's page would share the thread unevenly.** Half the core's intervals
  fit in a 60 Hz frame, and one in twenty would hold React for more than 27 ms.
  The S23's frame rate is the GPU's either way, but input latency is not.
- **The worker isolates the UI from a long call, and not only on average.** In
  Safari, `writeBuffer` held the core's worker for 15 s at a time, and the page's
  UI was not held with it; that the worker is why is read, not stated
  ([the Safari stall](../../gpu/experiment/20261111-the-safari-stall/README.md)).
  The belt has since removed the synchronous `writeBuffer` wait, though runs with
  the belt still stalled at the canvas. The
  rule the move was held to is that a core running 200 ms or more blocks neither
  the UI's input nor React's work
  ([the runtime is glue](../../minutes/20261106-the-runtime-is-glue.md)).
- **What the worker costs.** A worker's frame lies 3.6 ms (Chrome) and 4 ms
  (Safari) at the median from the page's nearest
  (`doc/experiment/20261106-the-core-in-a-worker/journal.md`). Answers that were
  synchronous are now messages: a marker's text fit, a surface's open, the
  profiler's reads.
- **On a device**, the worker and the core's own thread are not one
  implementation. They answer one requirement, the UI's context kept apart from
  the core's, each as its platform allows.
- **Pixelet needs frames at rest** for characters, the location puck, the clouds
  and the sky by the clock, as Figma does not. Threads do not change that. Three
  builds of 2026-11-09 presented 13 to 46 times a second with no touch, which is
  not explained (`doc/experiment/20261109-the-app-that-did-not-move/README.md`).

## Resolved

The user's starting picture was threads that poll, processes passing work by
IPC. Read that way, Figma looked like cooperative IPC. It is one context of
execution, whatever the language. Three boundaries need not move together:

- **language**: whether the two sides are written in different languages;
- **memory**: whether each reaches the other's memory directly;
- **execution**: whether each is scheduled on its own.

Figma's editor has the first two and not the third. Pixelet's UI and core have
the third, and that is the one place a message has to cross. A boundary of
language or memory alone needs no queue.

The observation is closed. Figma's editor couples the JavaScript and the C++ on
the main thread through synchronous calls. It has to split long work, and no time
budget common to every C++ entry was seen.

Figma's boundary between the JavaScript and the C++ is one of language, not of
execution: the two share one stack, one frame and the browser's event loop. The
C++ is called and returns; between calls it is not running, and work that
advances over many frames keeps its progress on the heap until the next call.
The ring drew an execution boundary on one thread, and the link draws it between
threads.

September's turn split time so the UI could answer. November's link moved the core
out of the UI's context. The problem the two answer is the same one, but the link
needs none of the turn's mechanisms. So Figma is a case for understanding a design
on the main thread. It is no reason to restore the rings or the host's quota.

The turn at `3cccd2cf7a` also stopped between steps, as Figma's work can. What
differed was who owned the rule: the host granted the quota and the core kept to
it, where now the core keeps its own budget.

A desktop page could have held pixelet's frames on the main thread. On a phone
the core's longer intervals would hold the UI past a frame, and the worker keeps
the UI apart from a long call. Figma's main thread fits its product and its
synchronous reads of state. It is no reason for pixelet to return there.

## Open

Worth comparing, not queued:

- **What wakes the app with no touch.** Builds before the link presented 13 to 46
  times a second with no touch. Count it at `HEAD`, and tell content's frames from
  a timer or an invalidation that repeats.
- **The core's steps at `HEAD`, in the worker.** Measure tick, effects and stage
  again, since every figure above predates the link. Measure for the first time
  the renderer's recording alone and the objects' shard apply with its spread.
- **The main thread against the worker at `HEAD`**, if the worker's 3.6 to 4 ms
  proves to matter. Read Long Animation Frames, INP and React's commits during
  drags and loads, on a desktop and on a phone.

> Since 2026-11-16 (`deb1fbb08c`, `95d62a532f`): none of these is a question of
> this case study, which asked about the structure and is closed above; the
> structure has since moved further its way, and what it concludes holds. They
> were measurements beside it. With the core in a worker by design, a long step
> holds the core's own frame and not the UI's. The core's steps were measured
> after the link on the S23 and its tick on the six platforms the app runs on
> ([taken back](../../minutes/20261116-the-deployed-builds-pace-taken-back.md),
> [measured](../../experiment/20261116-the-deployed-builds-pace-measured/README.md)).
