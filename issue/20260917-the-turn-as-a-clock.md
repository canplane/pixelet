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

## Not done here

This is the inside of the guest's turn, the second pass. The executor there
(the hand-written step state, or a static executor such as Embassy's) can be
swapped without touching the boundary.

- **Returning at the moment an overrun is noticed.** Only a pass checks its
  budget. The phases after it, and the effects, `fetch_pump` and
  `render_frame` around the tick, still run.
- **What must run every turn versus what may yield.** The render and the
  report are candidates for the first kind.
- **The quota's clamp** (`min_ms` 2, `max_ms` 8) was derived when the render
  and the effects were outside it. They are inside it now, and they are
  charged as overrun until the numbers are taken again.
- **The content phase is empty** while POI is off (BACKLOG P1).
- **The chunk scheduler's pass is its own** (`under_budget`), not xpute's
  `run_under_budget`.

Nothing in this pass has been run; it passes `deno task check`.
