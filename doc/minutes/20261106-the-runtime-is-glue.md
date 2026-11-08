# The core apart from the UI, the runtime as glue

**2026-11-06** — decided by the user. The direction xpute takes, for any
program on it.

## What the user decided

- **Three names: the UI, the runtime, the core.** The runtime is what was
  called the host, with every device; the core is what was called the guest.
  `host` and `guest` are retired; pixelet's names (the kernel, the shell) are
  not xpute's.
- **The core runs apart from the UI**, so a running core blocks none of the
  UI's events.
- **The runtime is glue**: it turns what the core leaves on the rings into the
  platform's calls (fetch, files, sensors, text, the GPU) and brings back what
  they give, and it gives the core its time. The program's logic and policy are
  the core's. The runtime sits where the core's memory is reached, so its
  place differs by platform, an asymmetry taken as glue's:

  | | a page | native |
  |---|---|---|
  | the UI | the main thread | React Native's JavaScript thread |
  | the core | a worker | the core's thread |
  | the runtime | the core's worker | the JavaScript thread |

  - On a page the main thread does not reach the core's memory, so the runtime
    goes into the core's worker. The largest data (the network's, the files',
    the GPU's) then stays inside the worker; only the UI's small frames cross,
    and no memory is shared (no SharedArrayBuffer).
  - Natively the JavaScript thread shares the core's memory, so the runtime
    stays there beside the UI.
  - The runtime is TypeScript on both.
- **One ring pair, by io_uring's roles: the core is the kernel.** The SQ goes
  into the core: the UI's commands, the runtime's interrupts, and the
  completions of what the core asked. The CQ comes out of it: its completions,
  its events, and its requests to the devices. The UI writes the SQ itself, not
  through the runtime.
- **Outside the rings**: the turn (`run(quota_ms)` → `wake_ms`), the GPU's
  stream, and the memory a frame names.
- **The UI takes an event at once; the core applies it as its next turn
  rises** (it drains the SQ there), on every platform.
- **Nothing of it reaches `main` before it runs on the S23 and the iPhone.**

## The names and the turn, decided after

- **No `hostcall`.** It was an alias: a core's command to its runtime is an
  **import**, and one answered before it returns is a **call** (`env.call`,
  `xpute_call_on`).
- **No upcall, and no event as a frame's name.** What the core posts on the CQ
  is one of its imports; the generated packet types take the command's name
  alone (`ShellSnapshot`, not `ShellSnapshotEvent`), the door's trait is
  `Import`.
- **The UI, not the client**, in xpute's documents. pixelet's `client` (the
  app, `@pxl/client`) keeps its name.
- **The scheduler, not the strobe** (`sched/scheduler.ts` `Scheduler`), as
  Linux's scheduler runs a task at the moment its scheduling policy picks;
  asking a turn is `wake`, as Linux's `try_to_wake_up`, not `request`.
- **A turn ends by a deadline, not a quota.** A frame's turn is handed its
  frame's deadline, as Android's Choreographer, Apple's `CADisplayLink`
  (`targetTimestamp`) and Chromium's `BeginFrameArgs` hand one: from the
  frame's start, so a turn rung late has the less. An overrun owes the next
  turn nothing: charging it (EEVDF's lag) is fairness among tasks that
  compete, and the core competes with none (`SCHED_DEADLINE`'s reclaiming,
  GRUB). `run(deadline_ms)` is the ms from the rising edge to the deadline,
  as `SCHED_DEADLINE`'s deadline is relative. A runtime with no display that
  shares its thread ends each turn a slice later (`Timeslice`, SCHED_RR's).
  Natively a turn no kick gave a deadline has a deadline of 0 and applies
  what was sent, as a page's doorbell rings for it.

- **Mechanism and policy, named apart.** The core, the runtime and the UI are
  xpute's, the mechanism; the kernel and the shell are pixelet's, the policy
  run on it. pixelet's code names its own by pixelet's (`kernel.wasm`,
  `service/kernel.ts`), xpute's parts by xpute's.
- **Names are singular**: `ts/client/src/service/`; a UI across a thread is
  `@xpute/runtime` `remote.ts` (`connect`, `serve`), not a port, which xpute
  no longer has.

## What follows

- **pixelet's UI commands that wait for the answer now**
  (`"call"` in `rust/client/kernel/shell.json`, read in the probe's journal,
  `doc/experiment/20261106-the-core-in-a-worker/journal.md`):
  - eleven whose answer the UI drops become sends;
  - `surface_open`, `edit_tool` and `edit_marker_text` post with `ACKREQ` and
    take the `RES`;
  - `perf_stats` and `perf_rings` reply with the bytes;
  - `fs_mount` is a device's, beside the core, and stays a call.
- **A command posted with `ACKREQ`**: the core replies once, in the turn that
  applies it; a `RES` nothing waits for is dropped; a stopped core answers
  `ENOTRECOVERABLE`; a paused one leaves it waiting; there is no cancel.
- **Memory a frame names** is the receiver's until it stores HEAD past the
  frame (the rings' R3, `Door::is_consumed`). Bytes that cross to the UI (the
  snapshot's `at` and `len`, the profiler's replies) are copied before HEAD
  passes.
- **What the change is held to**: a core running 200 ms or more blocks
  neither the UI's input nor React's work; no large copy exists for the
  placement alone.

## Open

- The app builds (iOS, Android) after the renames: Kotlin, Swift and JNI names changed.
- The probe on the S23's Chrome and the iPhone's Safari.
