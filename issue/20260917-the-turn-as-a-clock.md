# The turn as a clock

**2026-09-17** — worked out in a conversation after an audit found that the
turn path, the one actually in use, did not match what
[20260914-mechanism-and-policy-and-what-xpute-is.md](./20260914-mechanism-and-policy-and-what-xpute-is.md)
records. This settles the boundary between the host and the guest. What runs
inside the guest's turn is a second pass, not taken here.

## What the audit found

- **The guest ran outside its turn.** The shell called the module
  synchronously wherever it needed an answer at once: a UI action, the room
  for a response body, a model's block, the tab's visibility. Each one
  drained the submission ring there and then, so work queued for the next
  frame ran at whatever moment a response happened to land. None of it was
  under a quota, and a drain between frames was not in the turn's measured
  cost at all.
- **The doorbell no longer said what a turn was for.** Every call was
  `DRAIN`. The frame was a command inside the ring, and nothing told a
  drain apart from a turn.
- **Two quanta, chosen from inside.** The host granted an interactive slice
  and a batch quantum, and the guest picked between them from whether its
  view had moved. The host had to know the guest's inside to size the next
  grant.
- **An overrun was charged to the other side.** The record said a turn that
  runs over costs the next grant what it overran. The code never read the
  overrun; what it did read was the cost outside the tick, and that made
  the _guest's_ next slice smaller when the _host_ was slow.
- **A heartbeat.** The shell rang a turn every 250 ms so the kernel's timers
  — a debounce, a retry, a toast's dismissal — would be read, whether or not
  one was pending. Every such turn rendered.

At the 2026-09-12 checkpoint the TypeScript had the same first property: React
effects and event handlers wrote their stores at once. So this is not a
regression of the move. It is the line the record drew and the code did not
yet follow.

## The picture

The shell is a driver: it talks to the guest through rings and a doorbell,
and nothing else. The turn alternates like a clock's two edges.

```
rising edge   host → guest   interrupt(quota_ms)
              the guest applies what waits and runs its own tasks cooperatively
falling edge  guest → host   the call returns: when it wants the next edge
```

- **The main thread's xpute** has two jobs. It manages memory: the big
  memory, and the little memory attached to each worker by transfer (a
  skeleton for now, `sys/slab.ts`). And it runs the turns
  (`sched/strobe.ts` over `ipc/doorbell.ts`).
- **The guest's xpute** takes the quota, runs its tasks within it and hands
  the turn back. The host never learns which tasks those are.
- **The durations are not equal.** The quota is the guest's share of the
  frame; the host keeps a margin for its own work and its collector.

## Decisions

**One quota, "this is how long you have."** Every precedent looked at hands
the worker a single remaining time and sizes it outside:
`requestIdleCallback`'s `timeRemaining()`, Chromium's `BeginFrameArgs`
`deadline`, React's scheduler's `frameInterval`, an OS time slice, Tokio's and
BEAM's per-task budgets. None of them makes the worker choose between two.

**The host classifies a turn from behaviour, the way an OS scheduler does.**
ULE and Windows' foreground quantum decide "interactive" from what a process
did, not from what it says. So a turn is interactive when there was input
within `settle_ms`, or when the guest's last falling edge asked for the very
next frame. The second rule catches the camera still easing after the hand
has let go. It also counts a still view with chunks building as
interactive; that was accepted over giving the damping a batch-sized quota.
An interactive turn gets the frame less the margin, clamped; any other
gets `batch_ms`.

**An overrun is charged to the guest's own next quota**, down to the floor.
This is a constant bandwidth server's rule. Nothing is preempted, so a turn
overruns by the step that noticed; charging the other side would move the
whole cycle, and the frame period is the display's, not the accounting's.
The host's share of the frame is left alone.

**The falling edge carries when the next turn is wanted: 0 for the next frame,
a delay, or none (`NO_WAKE`).** This is a tickless kernel's next deadline in
place of a periodic tick. Each kernel timer asks for its own time with
`wake_at` whenever it is pending, and the earliest wins. The heartbeat is
gone: a guest asking for nothing is not rung.

**The guest runs only inside a ring.** A ring with a zero quota only applies
what waits. There are three kinds:

- **Boot and configuration queries**, before the first edge or when a canvas
  is attached. This is a driver reading a device's configuration space, and
  it stays.
- **A UI action** is rung at once with no quota. What it asks of the host has
  to be heard inside the gesture: iOS grants orientation only there.
- **The tab hiding** is rung at once with no quota, because a hidden page gets
  no frame and that is the pending save's last sure moment.

**A body crosses by the kernel pulling it, not by the host asking for room.**
The host keeps a response's or a model's bytes and names them by a number in
the message. In its turn the kernel takes a block and calls the `body_read`
import, which copies the bytes and lets them go. This is a driver's `read()`,
the same shape as the storage hooks. `FETCH_BODY` and the model path's
`POOL_ACQUIRE` are gone, and so is the kernel's table of rooms given out and
not yet reported.

**The doorbell's argument is the quota.** The `Interrupt` vectors (`DRAIN`,
`TICK`) are gone from xpute: every call is a turn.

**The little memory's owner key is the I/O demand's key (u64).** A worker job
will be an I/O demand (`sched/io`), so a slab is owned by that key and the
slabs not away are the worker credits. The Rust side's key decided it.

## Inside the turn

Taken the same day, on the same rule the allocator follows: `GlobalAlloc` is
the interface, the slab and the buddy are xpute's parts, `pxl-wasm` installs
one, and `pxl-core` allocates without naming it.

- **The interface is xpute's and names no executor.** `sched/task.rs` is
  `Executor` — spawn a type-erased future at a priority, poll, pending —
  with one install point, as `GlobalAlloc` and `#[global_allocator]` are.
  `sched/edge.rs` holds the quota as a deadline on the host clock, a table of
  tasks parked for the next rising edge (woken there, never by themselves,
  so no executor re-polls them past the quota), `yield_if_spent` and
  `next_turn`, a `Notify` a task with nothing to do waits on without asking
  for a turn, `wake_at`, and the falling edge's value. The table is the
  guest's task count: a task is parked at most once, and one past it aborts.
- **The executor is `pxl-wasm`'s: embassy's raw executor**, no platform
  feature, so no wasm-bindgen, installed once at boot. One embassy task type
  awaits whatever task is spawned, pooled as many times as the kernel
  declares (`TASKS`), so a task added in `pxl-core` touches nothing here. The
  pender only records that a task was queued.
- **A priority from the start, relied on by nothing.** Every spawn carries
  one and embassy's queue does not keep it; an executor that orders by it can
  be installed later without a spawn changing. Until then, work that has an
  order within a turn runs in one task, in that order.
- **Nothing runs every turn on its own.** Keeping a "must run" part of the
  turn apart from the tasks was an RTOS's distinction this kernel has no use
  for: a turn whose draw is skipped is a frame late, which an overrun already
  is. The frame is a task — the effects, the tick's phases, the fetch pump,
  the draw, in that order — and the doorbell polls the tasks on its quota, or
  whenever the host asked for a frame, so a quota overrun to nothing still
  draws. A turn with no quota and no frame runs no task and asks for the next
  edge for the ones it woke.
- **What moved into tasks:** a fetch's body (read, decoded, settled), a
  model's parse, the chunk builds, the decor models' registration and the
  store's save. The drain only queues arrivals, the tick leaves the build task
  its order, and the frame only checks whether a save or a registration is
  wanted.
- **No measured number.** The quota is a share of the frame the display
  sets: the frame less the margin while interactive, `batch_frames` frames
  otherwise. `min_ms`, `max_ms` and `batch_ms` are gone, and a pass's first
  step is the floor.
- **What embassy costs at the boundary.** Its link-name hooks are module
  exports and wasm-ld cannot keep them in: `__pender`, the critical
  section's two, and two timer-queue hooks, one of which carries a panic
  message. Accepted over patching or writing an executor.

## Not done yet

- **A placement's immediate build** (`build_now`) still builds to the end at
  once. The chunk releases what it shows before rebuilding, so deferring the
  build to a task would show a hole, not a late frame.
- **The tick's phase runner stays** inside the frame task, as the order its
  steps run in. The ledger's per-step times still come from it; the other
  tasks' times are not recorded. Embassy's `trace` hooks would give per-task
  times, each needing a name as a number, and an ordering executor would let
  the phases become task priorities.
- **The content phase is empty** while POI is off (BACKLOG P1).

Nothing in these passes has been run; each passed clippy and `check` on its
own changes over the last commit.
