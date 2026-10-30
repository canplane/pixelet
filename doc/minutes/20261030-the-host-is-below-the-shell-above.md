# The host is below, the shell is above, and neither stands where the other does

**2026-10-30** — recorded with the user. Main was at `127a83946`. The hostcall
below is built on the branch `worktree-sync-call`, not committed. Everything
under "Proposed" is a design, not a decision.

## What was asked

The user asked whether xpute, with the renderer taken out, could carry other
work on the web, such as a small model's training or inference. That led
through the GPU's calls (records gathered in a stream against a call per
operation), to whether a synchronous path should stand beside the stream, to
the hostcall, which was built. From there the user asked what IPC xpute has,
whether the whole reads as a VM, and whether its names fit. They then said
where the trouble lies: xpute and pixelet stack in opposite directions, and
xpute's host and pixelet's UI sit in one place only by accident. They asked
whose `intr` and the quota are, how the host should boot, what the April
checkpoint (`checkpoint/20260405-rpc`) did, and whether its message passing can
be fused with today's one thread, through the little memory of September.

## What the user decided

- **The hostcall.** A guest's call into its host, answered before it returns,
  is named `hostcall` (Wasmtime's and Lucet's word). On a device, where the
  guest runs on its own thread while the JavaScript thread waits in `intr`, a
  hostcall is handed back to that waiting thread and answered there, so the page
  and the device answer through the same TypeScript (`ts/client/src/abi/text.ts`
  `text_call`).
- **The two stacks run opposite ways.** xpute's host and WebAssembly guest are
  a VM's arrangement, right for the web's limits, and pixelet's kernel is the
  workload on that guest. The kernel has no UI and no outside events of its own;
  those are the shell's, which stands above it. So xpute's below and above are
  pixelet's above and below. The two look like one only because xpute's host
  and pixelet's UI are both TypeScript in one process.
- **The ABI is divided by whose it is**, xpute's or pixelet's. The host and the
  UI need not be in one place: the top and the bottom happen to share one, and
  are not one thing to bind together.
- **`intr` and the quota are given one owner each.**
- **xpute is a layer that manages computing resources, a VM and nothing else.**
  It assumes no GUI and draws nothing; a screen is an outside device's matter.
  Its host could be a C++ command-line program.
- **Pacing turns to a screen is policy.** xpute keeps the mechanism, and the
  policy is injected when xpute boots.
- **The rings, the turn and its synchronous and asynchronous paths are xpute's**,
  to the guest a hypercall interface in the wide sense; the strobe is xpute's.
- **The vocabulary is redefined to fit the layers**, by the field's word where
  the field names the same thing. Decided: `intr` becomes `run`, as KVM's
  `KVM_RUN` runs a vCPU until it exits; the host's `Quantum` becomes a
  scheduling policy, Linux's word, and the page's is frame pacing, Android's;
  a signal becomes an event, as NVMe's controller posts an asynchronous event
  and Xen raises one on a channel. The quota (Seastar's task quota), the
  hostcall, the doorbell and the strobe keep their names. "Turn" stays the
  word for the span one run takes.
- **The host's boot gets a settled developer experience**, and what the April
  checkpoint did, when its code was polished by hand, may be taken as it is
  where it fits.
- **This design is written** ("해봅시다"). What it proposes is not adopted by
  being written.
- **The layers read as Linux's display stack does** (the user, on the two
  commands below): the shell is the compositor and desktop shell, the kernel a
  client drawing into the surface it is lent, xpute's GPU host and the
  platform's input the kernel's devices.
- **`SURFACE_*` is the upper ABI's**, a canvas the shell lends the kernel as a
  compositor lends a subsurface; binding that canvas to the GPU device
  (`GpuHost.bind`, `xpute_gpu_surface_bind`) is the lower's.
- **`WORLD_VISIBILITY` is the lower ABI's**, for the duty it cannot drop: the
  platform is about to stop the process, so turns stop and the store is flushed,
  as logind's `PrepareForSleep` is a system's word and not a compositor's. "No
  one is looking", which pauses the sensors that only serve the screen, is the
  compositor's suspended state, and splits off to the upper ABI when the UI
  stands elsewhere.

## The IPC as it stands

Four things cross: one memory, one call in with one number out, and two calls
out answered before they return.

| Crossing                | Direction    | What                                               |
| ----------------------- | ------------ | -------------------------------------------------- |
| the core memory         | both         | granted once at boot; everything else lies in it   |
| `intr(quota)`/`wake_ms` | host → guest | the turn                                           |
| `now`                   | guest → host | the clock, answered at once                        |
| `hostcall(at)`          | guest → host | a program's call, answered at once (contract rule 8) |

Laid in the memory over those:

| Channel                                     | Direction    | Kind               | When it is seen          |
| ------------------------------------------- | ------------ | ------------------ | ------------------------ |
| submission ring                             | host → guest | queue of commands  | applied the next turn    |
| completion ring                             | guest → host | replies, signals   | read as the turn returns |
| streams (system calls, GPU)                 | guest → host | records, no reply  | run as the turn returns  |
| in place (directory, snapshot, profiler)    | guest → host | the latest state   | between turns            |
| answers at guest-named addresses (`READ`, `MAP_ASYNC`, `QUEUE_DONE`, `DEVICE`) | host → guest | a future | the next turn |

Each direction has both timings, and each synchronous path reuses an
asynchronous channel's shape: the doorbell's `call` is a submission rung with no
quota; a hostcall is a system call's record run at once. Only `now` returns a
value with no channel under it. Whether a call is an FFI is how a guest's form
implements it (a module's import, a native function pointer, a thread handed
back), not a part of the model.

A stream flush, the stream run up to a point mid-turn, would generalize the
hostcall and give the GPU's calls their overlap with the CPU. It was weighed and
left: it would let calls that wait run inside a turn, it changes rule 5, and it
has one user, the GPU, whose gain is not measured.

## The VM reading

| xpute                                   | A paravirtualized VM            |
| --------------------------------------- | ------------------------------- |
| the core memory                         | guest physical memory           |
| `intr(quota)`, `wake_ms`                | a vCPU's entry; HLT with a timer's deadline |
| streams run as the turn returns         | a virtqueue kicked once at exit |
| the completion ring                     | virtio's used ring              |
| `hostcall(at)`                          | a hypercall, its arguments at a guest address |
| the directory                           | boot parameters, a device tree  |
| handles the issuer checks               | Xen's grant references          |
| the GPU's stream, guest-issued ids      | virtio-gpu's Venus              |
| `MAP_ASYNC` and `QUEUE_DONE` tickets    | fences                          |
| files as Plan 9's                       | virtio-9p                       |

Where it departs, on purpose: there is no preemption and no interrupt injection
mid-turn, since the web's one thread has none; a native guest is not isolated;
the clock is a call, not a shared page, since on the web no host runs to update
a page during a turn; and a native hostcall takes the slow path, as an exit
handled by a userspace VMM does.

## Proposed

### Two stacks, three ABIs

```text
 pixelet, by role                       xpute, by substrate

   shell    the kernel's user              guest  ── the workload: pixelet's kernel
     │  ▲   pixelet's upper ABI              │  ▲   xpute's contract
     ▼  │                                    ▼  │
   kernel   decides, owns no device        host   ── the platform: memory, clock, I/O
     │  ▲   pixelet's lower ABI
     ▼  │
   devices  files, text, GPU, sensors
```

| ABI                    | What                                                    | Who answers                   | Where it is now                   |
| ---------------------- | ------------------------------------------------------- | ----------------------------- | --------------------------------- |
| xpute's contract       | memory, turn, rings, streams, hostcall, directory, files, GPU calls | xpute's host and guest | `xpute/`                     |
| pixelet's lower ABI    | the kernel asking its devices: files (namespace), text, the GPU device, sensors, surfaces, the frame's clock | pixelet's drivers on the host | part of `ts/client/src/abi/`, `namespace/`, `host/` |
| pixelet's upper ABI    | the user asking the kernel: commands for intents, the snapshot, the UI's signals, the profiler | the shell | part of `ts/client/src/abi/`, `shell/` |

Both of pixelet's ABIs ride xpute's channels and mean nothing to xpute, the way
two protocols share one virtqueue. Today they are mixed in two places:

- `spec/client/kernel/domain/control/cmd.json` holds both the shell's intents
  (`INPUT_*`, `SETTING_SET`, `CHAT_SEND`) and the devices' and the clock's
  (`SENSOR_*`, `WORLD_FRAME`, `WORLD_VISIBILITY`, `SURFACE_*`).
- `ts/client/src/abi/` holds the upper ABI's `snapshot.ts`, `shell.ts`,
  `input.ts` and `profiler.ts` beside drivers of the lower one, `text.ts`,
  `sensor.ts`, `gpu.ts` and `surface.ts`, which belong with `namespace/`.

The profiler is the upper ABI's: a developer's UI reading the kernel, in place
as the snapshot is read (pxl-kernel `profiler/frame.rs`,
`ts/client/src/abi/profiler.ts`). What xpute gives it are counters any program
may read (`HeapStat`, `DoorStat`, the tick's `StepStat`), as a VM gives a guest's
tools its vCPU's counters.

The seed of the split is already a rule: pixelet's host gives no number a
meaning (AGENTS.md), so the host carries the upper ABI as a courier and answers
the lower one as a driver.

xpute holds no word of pixelet's. What is mixed into it is the premise that the
guest is an interactive program that draws to a screen, meaningless to a guest
that trains a model:

| Where                                                         | What                                                        |
| ------------------------------------------------------------- | ----------------------------------------------------------- |
| `xpute/rust/guest/src/sched/tick.rs`, `xpute/spec/sched/tick.json` | the phases `interaction`, `visible`, `content`, `cosmetic`, `report`; `content_share` |
| `xpute/ts/host/src/sched/quantum.ts`, `xpute/rust/guest/src/sched/quantum.rs` | `interactive`, `settle_ms`, `batch_frames`, `input()` |
| `xpute/rust/guest/src/ipc/directory.rs`                       | the keys `SETTLE_MS`, `BATCH_FRAMES`                        |
| `xpute/ts/host/src/host.ts`, `xpute/ts/host/src/sched/strobe.ts` | `Host.input` ("makes it interactive"), `Strobe.input`   |

The next section takes them out.

### xpute's four parts

What the guest sees is xpute's, and so is what implements it, but they are not
one part:

| Part                         | What                                                        | Does the guest see it | In a VM                                   |
| ---------------------------- | ----------------------------------------------------------- | --------------------- | ----------------------------------------- |
| the contract                 | the core memory, the turn (`intr`, the quota, `wake_ms`), the rings, the streams, `hostcall`, `now`, the directory, handles | yes | the paravirtual interface: hypercalls, virtqueues, shared pages |
| the host's machinery         | the doorbell, the strobe, `Sys`, `GpuHost`, the native host's thread handed back | no; it keeps the contract | the hypervisor's own: its vCPU scheduler, its device models |
| the guest's library          | the door, the tick, the pass, tasks, the allocators         | the guest picks what it links | a guest kernel's paravirtual drivers, a library OS |
| `kit`                        | collections, codecs, wire formats                           | anyone links it       | —                                         |

Policy enters at two places: the host's machinery takes a pacer, the guest's
library takes the tick's phase names and meaning. "Hypercall", strictly, is the
synchronous trap, xpute's `hostcall`; the rings are virtqueues. Xen names the
hypercalls and its shared memory (grant tables, event channels) together as the
guest's interface, and xpute's documents already name that whole "the
contract", so it needs no new name.

### `intr` and the quota

`intr` is xpute's alone: the host handing the guest the CPU.

The quota fuses two owners today (`@xpute/host/sched/quantum.ts` `grant`):

| What goes into it                                   | What it is                          | Whose         |
| --------------------------------------------------- | ----------------------------------- | ------------- |
| the refresh, measured from animation frames; turns a whole number of refreshes apart | on the web, when the host must give the shared main thread back to the compositor | xpute's |
| `margin_share`; an overrun taken off the next quota | the host's own work and accounting  | xpute's       |
| interactive within `settle_ms` of `input()`         | how the UI answers a person         | pixelet's     |
| `batch_frames` when not interactive                 | the workload's choice of rhythm     | pixelet's     |
| `Grant`'s `refresh_ms` and `paced_ms`, carried back into `WORLD_FRAME` and the profiler's frame record | the frame's clock | pixelet's |

None of the right column is xpute's mechanism. The mechanism is the turn
itself: `intr` with a quota, `wake_ms` obeyed, a turn rung only when one was
asked for, an overrun taken off the next quota. When to ring and how much to
grant are policy, and the policy is injected when xpute boots:

- **A pacer** says when a requested turn is rung. On the web it is the
  display's: the refresh measured from animation frames, turns a whole number
  of refreshes apart (Android Swappy), since the host gives the main thread
  back to the compositor between them. On a command line it is a timer on
  `wake_ms` and the host's clock.
- **A grant** says how much a turn gets: what a frame leaves after a margin on
  the web, a configured slice for a batch job.
- **The program's rhythm** is policy too: whether a turn is interactive after
  input, how many frames a batch spans. Whoever composes the program injects it
  with the rest; `Host.input` and `Strobe.input` become that policy's input,
  not xpute's API.

A display-paced pacer may still ship beside xpute's host, as the GPU's calls
stand beside its two ends: any program on a page needs one, and nothing in it is
pixelet's. What xpute's core must not do is assume it. The tick's phases follow
the same line on the guest's side: the ordered phases and their budgets are
xpute's mechanism; their names (`xpute/spec/sched/tick.json`: `interaction`,
`visible`, `content`, `cosmetic`, `report`) and meaning are the program's.

The frame's clock pixelet reads (`Grant`'s `refresh_ms` and `paced_ms`, in
`WORLD_FRAME` and the profiler's record) comes from the pacer pixelet injected,
not from xpute, as Chromium hands a renderer `BeginFrameArgs` (a frame time, a
deadline, an interval).

The directory's `MARGIN_SHARE`, `BATCH_FRAMES` and `SETTLE_MS` are policy
declared by the guest; injected at boot, they leave the directory. And
`xpute/ARCHITECTURE.md` describes the web host's pacing as xpute's own: `wake_ms`
0 as "the next display frame" (the contract says the next turn), the quantum's
refresh and the strobe's animation frames as the first of three levels, and
Swappy among xpute's precedents. Those passages become one host's policy.

This keeps the line
[20260914-mechanism-and-policy-and-what-xpute-is.md](./20260914-mechanism-and-policy-and-what-xpute-is.md)
drew, mechanism in xpute and policy above, and moves two things it placed: the
central scheduler no longer sizes the quantum from the display's frame interval
as its own mechanism, and the policy's numbers are injected at the host's boot
rather than declared in the guest's directory. Where it lands it is named as a
contract change.

### A host with no screen

The contract already allows it: a host's own are the size of a quota and when to
ring, and host ends hold what their platform needs. A C++ command-line host
would reserve the memory once; run a module under an embedded WebAssembly
runtime, importing `env.memory`, `env.now` and `env.hostcall`, or link a native
guest; ring on `wake_ms` with its own clock and a configured quota; answer the
system calls from the disk and the hostcalls from its program; and reach a GPU,
if at all, headless, since a program that never reaches a GPU links none of its
calls. A surface is the display's, an outside device.

What stands in the way is the checking, not the contract. One TypeScript host
runs the traces against every guest, and the only host-side traces are the
files' (`xpute/spec/conformance/sys/`). A second host end needs host-side
traces for the turn, the rings and the hostcall.

### The shell is a client of the kernel, not a guest of the host

```text
 platform (browser / OS)
   │ DOM, input, a window               │ main thread, memory, GPU, network
   ▼                                    ▼
 shell (TS)                          xpute's host (TS) + pixelet's drivers
   │ pixelet's upper ABI               │ xpute's contract + pixelet's lower ABI
   │                                   ▼
   └────────── port ─────────────▶   kernel (WebAssembly guest)
              the host gives it no meaning

 the composition root (ts/app/*) opens the host and the guest and hands the
 shell the kernel's end of the port; the host does not know the shell exists
```

Making the shell an xpute guest would put on it a contract it has no use for
(memory granted once, turns, I/O by asking) and teach the host more of the UI,
not less. What it needs is one port to the kernel. With it:

- **Input** goes from the platform to the shell, which turns it into intents;
  whether the kernel is interactive now it reads from what it is sent.
- **Surfaces** split in two: the window is the shell's, lent to the kernel as a
  Wayland subsurface is (upper ABI); the GPU device drawing into it is the lower
  ABI's.
- **The frame's clock** belongs where the display is, with the shell. On the
  web `requestAnimationFrame` means both the host's moment to give the thread
  back and the display's vsync, because the browser puts the two in one place.

The port's contract is the April checkpoint's `CommandPort`
(`packages/xpute/spec/src/control/post.ts`): codec and dispatch only, knowing no
transport; the router, not the port, sets `RES` and keeps `ACKREQ`; so one port
mounts unchanged over xpute's rings now and a network later. xpute's frame rules
already hold the same invariants. If the UI ever becomes a WebAssembly guest of
its own, it is two guests under one host, which xpute's contract already allows,
and only the port's carrier changes.

### The boot

The April boot ran bottom up, and today's runs top down:

- **April.** `apps/web/src/index.ts` was `new Xpute(manifest).start()`. The
  manifest declared the platform's adapter (`BrowserDriver`, owned by the app,
  hiding Vite's inline worker URL), what ran where as lazy `() => import(...)`
  loaders, and the UI's module, whose default export took its links and gave
  back what it offered and a `start`, called once everything below had mounted.
- **Today.** `ts/app/web/src/main.tsx` sets the bracket and calls the shell's
  boot (`ts/client/src/shell/boot.ts`), which calls the client's
  (`ts/client/src/abi/boot.ts`), which mounts the host (`open_host`), wires
  sensors, signals, the GPU and the network, calls `WORLD_BOOT` and declares the
  source. The shell's boot owns the host's, though the shell names no host.

Taken as it was: one declaration as the composition root; lazy loaders; the
platform's adapter owned by the app behind a contract (today's bracket); an
endpoint module that returns what it offers and a `start` the root calls; the
order, everything below before the UI; contracts written in comments.

Left: the manifest in xpute (composing pixelet's kernel and shell is pixelet's
work, and xpute keeps `mount` and the guest's opener); the `Switch`, its routing
tables and takeover by port priority, `WorkerID` and `DevicePort`, the manifest's
`net`; the UI called `guest`; string commands and TLV arguments (today's are
numbered from `cmd.json` and carried in XTP).

```ts
// ts/app/web/src/main.tsx — the names are placeholders
new Pixelet({
  platform: page,                                    // the bracket
  kernel: (cfg) => page.guest.open(cfg),             // xpute's Guest
  host: { fs: namespace, program: text_call },       // the lower ABI's drivers
  pacing: display_paced({ margin, settle, batch }),  // policy injected into xpute
  shell: () => import("@pxl/client/shell/main.tsx"), // takes the port, starts last
}).start();
```

The order: the platform (a missing WebGPU halts here), the guest opened, the
host mounted with the drivers and the pacing policy, `WORLD_BOOT`, the port
made, the shell started with it. A device's app is the same declaration with its own platform and
kernel.

### Little memory, and workers fused with one thread

April's signal plane existed because the design was preemptive message passing:
several instances on their own threads, sharing no memory, notified by
fire-and-forget vectors
([20260911-single-thread-core-and-when-to-spawn-a-worker.md](./archive/20260911-single-thread-core-and-when-to-spawn-a-worker.md),
[20260909-april-checkpoint-salvage.md](./archive/20260909-april-checkpoint-salvage.md)).
One thread with cooperative turns left no one to interrupt, and the vectors went
([20260917-the-turn-as-a-clock.md](./archive/20260917-the-turn-as-a-clock.md)).

September had already decided where workers would stand. Big is the one fixed
memory the main guest keeps (today's core memory); little is plain
`ArrayBuffer`s in a capped pool of slabs, a job's each, moved to a worker by
transfer, the cap the backpressure and a slab away a worker's credit, owned by
the central scheduler
([20260911](./archive/20260911-single-thread-core-and-when-to-spawn-a-worker.md),
[20260917](./archive/20260917-the-turn-as-a-clock.md)). Two levels, one shape:
rings and a doorbell inside a worker, slabs moved by transfer between them
([20260913-boundaries-commands-and-what-comes-back-from-april.md](./archive/20260913-boundaries-commands-and-what-comes-back-from-april.md)).

The fusion keeps preemption and asynchrony at the host, at the turn's edge:

```text
 a worker (its own thread)              the main thread (cooperative)
 ┌──────────────────────────┐          ┌────────────────────────────────────┐
 │ its host + a small guest │ ◀─slab── │ host: an arrival, at any time, is  │
 │ (its memory, its loop)   │ ──slab─▶ │ put on the ring and a turn asked   │
 └──────────────────────────┘ transfer │   ▼                                │
                                       │ main guest (pixelet's kernel),     │
                                       │ the core memory, runs in turns     │
                                       └────────────────────────────────────┘
```

- No signal plane inside the guest: the host folds an arrival into a completion
  and asks a turn, as a posted interrupt waits for the next entry.
- The shape exists: the GPU's host does exactly this (a record asks, a ticket
  completes, `wake_on` asks the turn), and so does a file's open (`SYS_OPENED`).
  A worker is a device whose DMA buffer is a slab.
- The contract holds: a worker is a host and guest of its own, and hosts pass
  slabs by ownership, which "several guests under one host, passing buffers by
  ownership" already covers.
- It is the lower ABI's: workers are the kernel's devices, the shell its user.
  April put the UI, the engine and the network on one bus as peers; here each
  finds its layer.
- The mechanism (the pool, the transfer, the ticket, the turn asked) is xpute's,
  an offload beside the GPU's calls; which jobs leave, when and on how many
  credits is pixelet's.
- The cost: the core memory cannot be transferred, so a job copies big to a
  slab and back; a result is seen the next turn at the earliest; without a
  `SharedArrayBuffer`, rings between threads are messages.

It is built when the ledger asks for it, as September left it: the visible
phase spending its whole share frame after frame while pending does not fall
([experiment/20260913-bake-worker-on-the-phone.md](../experiment/20260913-bake-worker-on-the-phone.md)
measured the first candidate and did not take it).

### Names

A name in the contract is chosen by what the guest sees; a name in the host's
machinery, which the guest never sees, by its role in the host. The table is the
reading that led to the decision below it.

| Now                                   | What it is, once the layers are split          | Read                                    | Candidates (source)                       |
| ------------------------------------- | ---------------------------------------------- | --------------------------------------- | ----------------------------------------- |
| `intr(quota_ms)` (contract)           | the host handing the guest the CPU for one turn | short for interrupt, left from `interrupt(vector)`: an interrupt breaks into running code and carries an event, `intr` starts a halted guest and carries a budget | `turn`, the word the documents already define ("the turn is when the guest runs at all"); `run` (KVM's `KVM_RUN`); `upcall` (scheduler activations) |
| `quota` (contract)                    | the CPU granted this turn                      | fits, as cgroups' quota does            | —                                         |
| `wake_ms` (contract)                  | when the guest wants its next turn             | fits; its description loses "the next display frame" | —                            |
| a signal (contract)                   | a completion without `RES`, guest to host       | runs from the guest up to the host below, the opposite of a Unix signal | notification (virtio); event (Xen's event channels) |
| `Quantum` (host's machinery)          | the injected policy: when to ring, how much    | an OS's quantum is the slice's length, not what sizes it | `Pacer` (the code already says `paced`); `GrantPolicy` |
| `Grant` (host's machinery)            | today the quota and the screen's figures       | splits: the quota is xpute's, `refresh_ms`, `paced_ms` and `interactive` the pacer's | —            |
| the strobe, the doorbell              | ringing when asked, as the pacer allows; the turn handed over | fit                     | —                                         |
| the tick's phases (guest's library)   | ordered phases with budgets                    | their names are the program's           | xpute keeps "phase"                       |
| `interactive`, `batch`, `settle`      | the program's rhythm                           | leave xpute                             | —                                         |

`frame` resolves itself: inside xpute it means both the four-word message
(`ipc/frame`) and the display's frame the quantum counts. With the screen's
policy out, xpute's frame is the message alone and a drawn frame is pixelet's
word (`WORLD_FRAME`), two scopes the naming skill allows.

On pixelet's side the kernel keeps its name, the kernel of its shell. AGENTS.md's
"pixelet's host" means where the lower ABI's drivers stand; the port, the
composition root and the pacing policy need names of their own. The shell is not
called a guest: April's guest was the UI and today's is the kernel, and the word
turned over once already.

Decided after the table above (see "What the user decided"): the project's own
coinages first proposed, `turn` for `intr` and `Pacer` for `Quantum`, gave way
to the field's words, `run`, a scheduling policy and frame pacing, and a signal
became an event rather than a notification, since virtio's notification is a
bare "look" and not a message. They land with the pacing's split, across every
language of xpute, its C ABI, its documents and its traces.

## Order of work, proposed

1. The upper and lower ABI split in `cmd.json` and `ts/client/src/abi/`, the
   lower drivers beside `namespace/`, the shell reading only the upper.
2. The port: an interface the shell holds, the doorbell and rings behind it,
   handed over by the composition root.
3. The boot as one declaration, bottom up.
4. The pacing as policy: xpute's host keeps the turn's mechanism and takes a
   pacer, a grant and the program's rhythm at boot; the directory's quantum
   keys and the tick's phase names leave xpute's core.
5. `ARCHITECTURE.md`'s stack redrawn on the two axes, and AGENTS.md's rule for
   `abi/` and `namespace/` restated by layer, on the user's word.

Workers wait for the ledger, and the names for the user.

## Open

- Whether the upper ABI gets channels of its own (a ring pair, its own directory
  keys). Not before the UI stands elsewhere: the condition
  [20261024-plan-9-names-the-files-and-stops-there.md](./20261024-plan-9-names-the-files-and-stops-there.md)
  wrote down to reopen a namespace, a host that is far away.
- What the device's code does that a build has not checked: the hostcall's
  Swift (`JavaScriptValue` kept and called) and Kotlin (`JavaScriptFunction`
  called inside a synchronous function) were type-checked by neither Xcode nor
  Gradle.

## Where it stands

`WORLD_FRAME` carried both layers: the view's size, the shell's, beside the
turn's pacing, the network's credits and the clock, the host's. The size, which
`INPUT_RECT` already sends whenever the canvas moves, left it, so the frame is
the lower ABI's alone (`ts/client/src/driver/frame.ts`) and the kernel takes the
view's size from the rect (pxl-kernel `camera_view_size`).

Where `SURFACE_*` and `WORLD_VISIBILITY` fall is decided (above), and
`WORLD_VISIBILITY` now takes the platform's own path: the place says the page
or the app is hidden or shown (bracket.tsx `visibility`), the client's boot
hands that to its driver (`ts/client/src/driver/visibility.ts`), and the shell's
input carries it no more. The turns run while the world's view is open and the
page is seen, both judged in one place (`ts/client/src/host/mount.ts`), as they
ran before. Measured on the dev page: 60 turns a second shown, none hidden, 61
shown again.

The boot runs bottom up from the app: it sets the place, readies the shell's
words, and calls the client's boot (`ts/client/src/abi/boot.ts`), which opens
the guest, mounts xpute's host with the drivers and the pacing, and boots the
kernel. The shell boots nothing; it reads where the boot stands through the
port (`boot_state`, `watch_boot`) and installs its developer's console once the
kernel runs. The port stays a module boundary (`ts/client/src/port/`): an
object handed to the shell waits for a transport to swap, when the UI stands
elsewhere.

The first step of the order is built on the branch `worktree-layers`: the
names (`run`, an event, a scheduling policy and frame pacing); the strobe
steered by a policy pixelet injects at boot, its numbers in pixelet's
`spec/client/kernel/sys/sched/pacing.json`, the directory's quantum keys retired;
and the tick's phases the program's, xpute's tick holding back what each
phase's policy says (`held_back`), the kernel's five phases in
`spec/client/kernel/sys/sched/phase.json`. The pacing's numbers are no longer declared
in the guest's directory. Step 2 split the client into `port/`, `driver/` and `abi/`; step 3 boots
it bottom up from the app; step 5 redrew `ARCHITECTURE.md`'s stack on the two
axes, with a table of what crosses each edge. A device's build has checked none
of it.
