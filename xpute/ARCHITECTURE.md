# Architecture — xpute

[xpute](./README.md) makes a program's core its first-class citizen: the
core leads, the same compiled code on the web and natively, and JavaScript
does the I/O and the UI around it. The two share one memory, and a pair of
rings in it carries everything else, with no foreign function call for each
piece of the boundary. The **core** is a **unikernel** on a virtual machine,
run inside: the program and xpute's core library, its library OS, built as one
image. The **runtime** is its VMM: it implements the core's devices and gives
it its time.

The core sees one thing outside it, its **world**, whose commands are its
**imports**, which it calls, and its **exports**, which it handles, as a WIT
world says; it never names a destination. A platform's author sees the world
in two parts: the UI outside the VM, and the VMM under it. The program's
**UI**, its screens and components, stands outside the VM, as an application
outside a VM reaches a service inside it over virtio-vsock: a unikernel has no
userspace. The ring pair is the device between them. The UI submits commands
to the core and the core completes them, by io_uring's roles; what the core
tells the UI unasked, more rarely, is one of its imports. That relation is
the app's ABI. The runtime implements the devices the core's machine asks for,
the GPU and the file device, by virtio's convention, and gives the core its
memory, its clock and its turns, as a VMM gives a vCPU's time. The core calls
its devices, and the runtime raises interrupts. That relation is xpute's
predefined ABI.

The core runs apart from the UI on every platform, so a turn holds none of
the UI's events; the runtime is glue, and sits where the core's memory is
reached. Natively the core has its own thread and the runtime shares the UI's
JavaScript thread, which shares the core's memory. On a page, whose main
thread shares no memory with a worker, the core and the runtime run in a
worker and the UI on the main thread, a MessagePort between them
(`@xpute/runtime/remote.ts`) that carries frames: up, the UI's, a lifecycle
call by name and a canvas transferred once; down, a reply or an import the UI
takes, its packet and the memory it names copied before the runtime's head
passes it, the GPU's state and the stop. What is large, the network's, the
files' and the GPU's, stays in the worker. Both relations ride the same ring
pair, told apart by a command's major; the runtime carries the UI's
submissions on the submission ring without reading them. The UI meets the
runtime directly only for the surfaces (a view lent to the GPU) and the
lifecycle (boot, visibility), through its link (`Link`), which `start`
gives on every platform: the runtime itself beside it, or `connect`'s across a
port.

The runtime grants real resources, a real region and a real deadline, and the
core's library OS builds the program's abstractions over them; nothing is
emulated, and xpute does not enforce the grants.

xpute comes in two layers. `kit` is generic code — collections, codecs, a wire
format, an errno table, scalar math — that any program can link and call.
`core` and `runtime` are the arrangement itself, one end each, written against
`kit`, which knows nothing of them. Neither layer knows what a core computes.

Three things cross between the ends and nothing else does: one memory they
share, one call in, one number out. Two calls go the other way, each answered
before it returns: the runtime's clock, and a call the core makes of its world,
answered from what is already held (both below). The sections follow from
that:

1. the memory, the machine that sizes it and the front the runtime lays in it;
2. what is written into it — one shape for every message, its data, the two
   ABIs, and the rings that carry them each way;
3. the call that makes it run, the turn;
4. handles, which name objects across the boundary;
5. streams, files, calls and the GPU, which a core asks of its runtime;
6. the contract all of it comes down to;
7. the languages, and the records that hold them to each other.

---

## The core memory, placed once

The memory is the one thing the two ends share. It is the **core memory**, core
as in a core dump. The runtime reserves it at boot and it never grows, so a view
into it stays valid for as long as the core lives: a ring keeps its words at a
fixed address, and a payload is read where it lies. A page reserves it as a
`WebAssembly.Memory` whose initial size is its maximum
(`@xpute/runtime/mem/reserve.ts` `reserve_mem`); a native program that links its
core in maps it from the system, and the core's own thread then sets its base
(`xpute-runtime/native.rs`, `xpute-core/mem.rs` `set`).

The memory need not begin at address zero. A WebAssembly module's does; a core
linked into a native program has its memory wherever the system put it. So every
address a core names is an offset from the memory's base, never a pointer
(`mem`, `Memory`). It means the same to the runtime however the core is built, and
it crosses as a `u32` however wide the machine's pointers are. A part laid over
a region takes the `Memory` it lies in and its offset, so a test lays it over a
buffer of its own; a program reading its own data goes through the core's own
memory, set once (`core`, `at`).

How big the memory is, and what lies at its front, both ends must agree on
before either runs. A core crate declares it once, as its **machine**
(`machine.json`, its schema `spec/machine.schema.json`, generated as `MACHINE`
for both ends; `xpute-core/machine.rs` `Machine`): the sizes a runtime may give
and the unit a reservation is counted in, the heap the smallest must leave, a
module's stack and a native core's thread's stack, the rings' capacities and
slot size, what one turn posts besides its replies, and the devices it asks
for, `gpu` and `fs`.

The front follows from the machine, the same on both ends: the stack, a page
for the **header**, the two rings and their slots, the GPU stream, then where
a module's linker may place its data (`global_base`). The runtime lays the front
before the first turn, as io_uring's setup lays its rings: the header's words
(`spec/ipc/head.json`; `@xpute/runtime/ipc/head.ts` `lay`, `Front`) and each
ring's own header, its positions at 0. A core that outlived its runtime's reload
keeps the front it has (`laid`). On its first turn the core computes the
front from its machine and holds what the runtime laid to it
(`xpute-core/machine.rs` `boot`): where a word differs it stops with
`ENOEXEC`, the word's place and what it holds. The header names what the runtime
laid, not where to look: MAGIC, the rings' sizes and places, and
the GPU stream's.

```text
one core's memory; the front is the machine's, the rest the core's

 0 ┌─────────────────────────────────┐
   │ a module's stack                │
   ├─────────────────────────────────┤ head_at
   │ header                          │
   │   MAGIC                         │
   │   SQ_CAP · CQ_CAP · SLOT_BYTES  │
   │   SQ_AT ────────────────────────┼──┐
   │   CQ_AT ────────────────────────┼──┼──┐
   │   GPU_AT ───────────────────────┼──┼──┼──┐
   ├─────────────────────────────────┤  │  │  │
   │ submission ring                 │◀─┘  │  │
   ├─────────────────────────────────┤     │  │
   │ completion ring                 │◀────┘  │
   ├─────────────────────────────────┤        │
   │ both rings' slots               │        │
   ├─────────────────────────────────┤        │
   │ GPU stream, where asked for     │◀───────┘
   ├─────────────────────────────────┤ global_base
   │ a module's data                 │
   ├─────────────────────────────────┤
   │ the core's fixed ranges         │
   ├─────────────────────────────────┤
   │ heap, to the memory's end       │
   └─────────────────────────────────┘
```

Past the front, what goes in the memory is the core's, and so is where. It
lays ranges end to end on a unit it chooses (`mem/section`), as a linker script
places output sections without knowing what they hold, and whatever is asked of
a range is checked against its end (`Section::at`, `nth`); a range sized at
compile time is handed to its owner once at boot and read as an array laid in
it (`mem/range` `OwnedRange`, `arr`). The machine checks at compile time that
the smallest size holds the front, the data, the fixed ranges and the heap it
asks for (`Machine::check`).

The heap is reached through a single call (`mem/heap`) whose answer is the
core's choice. `kit` offers two allocators that fit together: a buddy allocator
of fixed pages (`alloc/buddy_tree`), ported from evanw/buddy-malloc, and a slab
of size classes over it (`alloc/slab`), which keeps its bookkeeping inside the
blocks it has not handed out.

---

## The frame, and the packet it carries

Sharing a memory does not say what a submission or its reply looks like, so
xpute gives every message between the ends one shape, the **frame**, of four
words (`ipc/frame`, `spec/ipc/frame.json`):

- `tag`, the sender's id, which a reply keeps, or an import's subject;
- `cmd`, what is asked, in the low 16 bits, with flags in the high 16;
- `result`, what came back, a value or `-errno`;
- where its **packet** lies — the data the frame carries — or 0 for none.

An errno is one table's (`spec/status/errno.json`), and every language reads a
code back into its member, or into none (`Errno::of` in Rust, `errno_of` in the
others).

Two flags decide what a frame means. `ACKREQ`, set by whoever submits, asks for
a reply. `RES`, set by whoever completes, says the frame is a reply. A
frame the core posts without `RES` is one of its **imports**, raised on its
own, as an NVMe controller posts an asynchronous event to its completion queue.

### Two ABIs, one number space

A command is `major << 8 | minor` (`kit` `abi/cmd`), and who defines it decides
its major. xpute's **predefined ABI** takes majors 0x00 to 0x0F, a protocol a
device (`spec/abi/sys.json`, `spec/abi/gpu.json`, `spec/abi/fs.json`). Its
imports are the core's calls to its devices: the file device's (`FS_OPENAT`,
`FS_READ`, `FS_WRITE`, `FS_CLOSE`) and `SYS_ABORT`, the report the core posts as
it stops. Its exports are the runtime's interrupts: `SYS_ATTACH`, a runtime
attached, the first time or again; `SYS_SUSPEND` and `SYS_RESUME`, calls around
a hidden runtime; `SYS_TURN`, a turn the scheduling policy grants, with the wall
clock; `GPU_READY`, the GPU the machine asked for answered; `GPU_SURFACE_OPEN`,
`GPU_SURFACE_RESIZE` and `GPU_SURFACE_CLOSE`, a surface the UI lends the GPU, by
the handle the first answers; and `FS_CHANGED`, a file changed under the core.
`NOP` is the rings' own test. What each life command means to keep or reset is
the core's.

The **app's ABI** takes majors from `PROGRAM_MAJOR`, 0x10, in protocol files of
the app's own, each command an `export` or an `import`, `call` where its sender
waits. The generator refuses an app's command under `PROGRAM_MAJOR`, and a
number two protocols both give (`xpute-tool spec/cmd.rs`). Nothing in a frame
names a port or a destination: the runtime hands a frame to what answers its
command's major (`@xpute/runtime/runtime.ts` `mount`), and the core's door applies
xpute's own and hands the program every other.

The generator writes both ends' code from the machine and the protocol files
(`xpute-gen`, `xpute-tool bin/gen.rs`; xpute's own by `cargo spec`): for each
protocol its messages (encoders, readers, enums), and its bindings, on the runtime
a stub per export and a handler per import (`<name>_exports`,
`<name>_imports`), on the core a function per import and a trait for the
exports (`Exports`, `apply`).

### The packet

Every packet has the same shape, **XTP**, the Xpute Tree Packet (`kit`
`wire/xtp`). One decision explains the rest of it: nothing inside a packet
records where the packet is. Every offset is counted from the branch that holds
it, so the same bytes mean the same thing at any address.

```text
one branch at byte 0, three children, each offset measured from that base

            ┌──────────────────────────────────────────────┐
  byte   0  │ len = 3          │ reserved                  │
  byte   8  │   child A  ──▶ +32 ──────┐  u32              │
  byte  16  │   child B  ──▶ +40 ──────┼──────┐  f32 seq   │
  byte  24  │   child C  ──▶ +560 ─────┼──────┼───────┐    │
            ├──────────────────────────┼──────┼───────┼────┤
  byte  32  │ u32, the value itself   ◀┘      │       │    │
  byte  40  │ len = 128        │ reserved    ◀┘       │    │
  byte  48  │   512 bytes of f32 payload              │    │
  byte 560  │ branch: 2 children                     ◀┘    │
            └──────────────────────────────────────────────┘
```

Three things follow. A payload is read where it lies, in whatever memory holds
it. A typed-array leaf is handed out as a view straight into the packet. And a
packet encoded earlier grafts into a larger one without a number inside it
changing. The format in full is [the note beside the TypeScript
implementation](./ts/kit/src/wire/xtp/README.md), which the other languages
mirror.

**TLV** (`kit` `wire/tlv`) is the other format: typed values in a forward-only
sequence, closed by END. Where XTP is a value walked where it lies, a TLV is
read once, in order. Both formats live in `kit`, since neither knows the program
using it.

---

## The rings, the doorbell and the door

Frames need a queue each way that neither side has to lock, and the **ring**
(`ipc/ring`, `spec/ipc/ring.json`) is that queue. It keeps the four-word frames
in one place and the packets they point at in **slots**, one a frame, in a
separate range of the memory, as virtio's descriptor table and AF_XDP's UMEM do:
the frames are small and stay hot, the payloads are large and do not.

```text
frames                                       payloads, elsewhere in memory
┌──────────────────────────────────────────┐  ┌────────┬────────┬───┬──────────┐
│ header: cap · head · tail ·              │  │ slot 0 │ slot 1 │ … │ slot n-1 │
│               ▲      ▲                   │  └────▲───┴────────┴───┴──────────┘
│ consumer ─────┘      └───── producer     │       │
│  one writer each, and no lock            │       │
│       slot_bytes · slot_base · 3 reserved│       │
├──────────────────────────────────────────┤       │
│ frame n: tag                             │       │
│          cmd (low 16) | flags (high 16)  │       │
│          result (value or -errno)        │       │
│          packet address ─────────────────┼───────┘  (slot n & (cap-1))
└──────────────────────────────────────────┘
```

Positions only grow, so `tail - head` is how many frames wait. Frame _n_ owns
slot _n_, so a slot is free exactly when its frame has been read, and nothing
under the ring allocates. A push fails in two ways only. `EAGAIN` means the ring
is full, and the producer waits. `EMSGSIZE` means the packet is bigger than a
slot, and such a packet crosses by address, in a heap block.

Two rings make a pair, by io_uring's roles. The **SQ**, the submission ring, is
written by the runtime, the side where the outside's interrupts arrive: the
UI's submissions and the runtime's own interrupts. The **CQ**, the completion
ring, is written by the core: its completions and its imports, what it tells
the UI and what it asks its devices, posted as ublk's server posts its
requests, whose results come back on the SQ. Each lies where the header says (`SQ_AT`, `CQ_AT`).

The runtime drives the pair through the **doorbell**
(`@xpute/runtime/ipc/doorbell.ts` `Doorbell`):

- it writes each submission in place, into the SQ's next slot (`writer`,
  `send`);
- it rings the core with a deadline (`ring`), which is one whole turn;
- it then reads the CQ dry, handing each reply to the call waiting for it by
  its tag and every other frame to its one listener (`hear`), which the runtime
  routes by major.

Three ways in sit on top of that. A question the runtime needs answered at
once is a `call`: submitted with `ACKREQ`, tagged with the next id, and rung
with a deadline of 0 until its reply comes back; it waits on a turn, so it is
the runtime's own code's. What a UI wants answered is a `request`, an export
the protocol marks `ackreq`: submitted the same way and sent with the next
turn, its reply read with the completion ring and handed back as a promise,
nothing waiting for it; a core that stops answers every request waiting
ENOTRECOVERABLE. A command from outside the runtime's own code,
such as a developer's console, is `cmd`: its arguments become the packet's
fields (`TreeView.set`), and it is sent and rung. A core that traps is gone,
and the doorbell rings it no more.

The core's end is the **door** (`ipc/door` `Door`), opened on the first turn
over the rings the runtime laid (`xpute-core/machine.rs` `boot`, `door`). It
applies the submissions in order, answers each, and posts what the core raises
into the CQ (`send` for an import its table generates, `Import`, or `send_with`
for a packet written in place), which holds a whole turn's output. What is
xpute's it applies itself (`NOP`), and it hands the program the rest
(`receive`, `Submission`): the core's life (`Life`: `Attach`, `Suspend`,
`Resume`), its GPU answered (`GpuReady`), an open the runtime completed
(`Opened`), and every command of the app's (`Program`). A drain runs before
any phase's budget, so the program records what it is handed and returns.
Before each submission the door checks that the CQ has room for the most one
command may leave — its imports, the machine's `turn_posts`, and its reply —
and where it has not, it applies no more and leaves the rest for the next turn,
in order.

---

## The turn

The turn is when the core runs at all.

It runs inside one call, `run(deadline_ms)`, named as KVM's `KVM_RUN` is, the call
that runs a core until it gives the processor back; when that call returns the
core is not running. The call has two edges, like a clock's. On the **rising
edge** the runtime calls in with a deadline, and the core drains the SQ, does its
work and posts what it has. On the **falling edge** it returns `wake_ms`, when
it would like the next turn: `0` for the next the runtime can run, `n` for that
many milliseconds, `NO_WAKE` for none. The runtime learns nothing else about what
ran.

xpute names that call, as a C runtime names the `main` it calls: the program
implements it and never renames it (`xpute-core/entry.rs` `Entry`, `run`). A
core built as a module exports `run`; a core linked into a native program
exports `xpute_run`, and its memory as `xpute_mem`, under xpute's name and no
program's, since a process holds one core, with one reservation
(`xpute-runtime/native.rs`). What the core's form hands it there is xpute's as
well (`BootInfo`): the memory it was given, where its own ranges may begin, the
runtime's clock, its `call`, its heap's reading, where the core shares the
runtime's process the hook that says why it stopped, and where its GPU runtime may
block taking a frame that runtime's admission.

A core's form follows its target, so a program writes one macro over its
machine and builds one crate for every target
(`xpute-core/machine.rs` `core!`): on wasm32 the module's form
(`xpute-core/wasm.rs` `wasm_abi!`), its link flags printed by its build script
from the machine (`Machine::link`); anywhere else the native form, which is
xpute's as a module's is (`xpute-runtime/native.rs` `native_core!`): its memory
laid out from its machine, its thread, its allocator, xpute's stop hook, the C
ABI (`rust/runtime/include/xpute.h`) and on Android the same calls to JNI under
the Kotlin class `xpute.Core`. A core given its GPU's numbers takes the GPU
runtime's form as well (`xpute-gpu/runtime/native.rs` `native_core!`). The crate is
the artifact, built as `rlib`, `cdylib` and `staticlib`; a React Native app
links it through xpute's native module (`ts/native`, `@xpute/native`), which
builds it for the device as the app is built (`script/build-core.py`).

Every runtime holds the core to one contract (`xpute-core/entry.rs`): the
core runs until it has nothing to do or its deadline passes, where its tasks
yield; it returns when it wants to be woken, `wake_ms`;
and a kick — a submission, a completion, a call — wakes it early. What differs
is who runs it, and on which thread.

When each side runs is the platform's. On a page the core and the runtime share
one thread, the page's event loop, and take turns. Something on the runtime side
has to decide when to ring at all. That is the **scheduler**
(`@xpute/runtime/sched/scheduler.ts` `Scheduler`), as Linux's scheduler runs a
task its scheduling policy picks the moment for. It rings only
when the runtime or the core's last falling edge asked for a turn, at the moment
the scheduling policy below gives; a positive `wake_ms` sets a timer that asks
for a turn when it fires. A core that asks for nothing is rung for nothing.
The program says only whether turns may run (`Runtime.resume`, `pause`, each a
no-op where it already holds) and asks for one where something the core did
not hear of changed (`wake`); once the core stops, the scheduler rings it no
more, and the mount tells the UI (its `stop`). The
doorbell rings too, with a deadline of 0: to make room in a full SQ, to wait for a
call's reply and to send a `cmd`.

Natively the core runs on its own thread, a vCPU that runs while it has work,
as Linux's idle loop and io_uring's submitter have it: submitting is not
stopping (`xpute-runtime/native/thread.rs`). The thread runs a turn whenever it is
kicked, and sleeps otherwise; it keeps no timer. The scheduler is the one
that times the core's turns, as it is a page's: the wake a turn asks, at
once or in `n` ms, reaches it as the thread tells it (`xpute_turns`, or
`xpute_run`'s answer), it times a wait of `n` ms as it does a page's, and the
policy gives and admits the moment, where a frame's grant is submitted and
kicked with its deadline; it asks no turn for what the runtime sent, which the
doorbell kicked. The runtime's JavaScript writes the SQ, stores its tail with
release and kicks (`xpute_kick`). A turn no kick gave a deadline has a
deadline of 0 and applies what was sent, as a page's doorbell rings for it. As a turn ends, the thread tells the JavaScript
side (`xpute_notify_on`), which reads the CQ dry on its own thread and hands
the policy what ran (`Doorbell.turns`); a read kicks too, for the room it
made. A call stays synchronous to its caller: submitted, kicked, and waited
for in `xpute_run` until a turn begun after it has ended. While the program
is paused or hidden the thread is paused too (`xpute_pause`, `xpute_resume`):
it runs only a turn a call rings, and its kicks wait. Three
rules hold the loop:

- **No kick is lost.** A kick sets a flag under the loop's lock and wakes the
  thread; the thread reads the flag and its work under that
  lock before it waits on a condition variable, which gives the lock up as it
  sleeps, so a kick lands before the check or wakes the wait.
- **Every wait has its answerer.** The core's thread waits for an answer only
  while a call stands in the loop's state: the JavaScript thread waiting
  in `xpute_run` answers it there; with none waiting the JavaScript side is
  told and answers it on its thread (`xpute_answer`), as a VMM handles a
  vCPU's exit. The JavaScript thread waits on the core only in `xpute_run`
  and in a device's call, and answers in both, so neither waits on the other.
- **A turn begins only where it can post what one may**: the door's room in
  the CQ (`Door::has_room`), read under the lock; where there is none, the
  thread says so and waits for the kick that follows the next read. Every turn
  drains the SQ before any phase's budget, so a kick's submission is applied
  by the next turn whatever made it run.

How long a turn may take, and how that time is spent, are settled at three
levels, from the outside in.

1. **The scheduling policy** (`@xpute/runtime/sched/policy.ts`
   `SchedulingPolicy`) is the runtime's, and the program injects it, with its
   numbers, when the runtime boots; xpute keeps none of its own. It is what the
   scheduler asks: it gives and admits the moment a turn may run, sets the
   deadline the turn ends by, which the core keeps by returning, and is handed
   what each turn took; the doorbell's turns are outside it. A page's is **frame pacing**
   (`sched/frame_pacing.ts`), named for Android's Frame Pacing library: turns a
   whole number of the display's refreshes apart, no nearer than the interval
   the program last asked (`pace`), each ending by its frame's deadline, as
   Android's Choreographer hands a frame its deadline: from the frame's start,
   so a turn rung late has the less, and one that ran past it owes the next
   nothing. A runtime with no display that shares its thread
   with the core runs a turn as soon as it is asked and ends it a fixed slice
   later (`sched/timeslice.ts`), as SCHED_RR's timeslice does.
2. **Tick** (`sched/tick`) is the core's. It runs the program's phases, by
   their place in its order, against one budget, the time from the rising edge
   to the deadline. The program's policy gives each phase the share of the budget
   held back from it for the phases after it (`held_back`), so a busy phase
   cannot starve a later one; most hold back nothing. What the phases are
   called, and what runs in each, are the program's. A phase's time is
   recorded, never stopped.
3. **Pass** (`sched/pass`) spends what is left of the budget on an ordered list
   of steps until the budget or a step cap says stop. Both are waived until a
   step has done work, so nothing starves behind what sits ahead of it. The hard
   budget, what is left plus the tick's `hard_slack_ms`, is never waived, but
   the first step is always tried.

Work too long for one turn is a **task**, written against `sched/edge` in its
language's form, a future or a coroutine. A task gets the turn's deadline,
`yield_if_spent` to stop once it has passed, and `next_turn` to wait for the
next turn; the core may move the deadline later (`extend_to`). A parked task makes the falling edge 0. Whatever assembles the
core installs an executor (`sched/task`), marks the edges with `rise` and
`fall`, and polls it between them.

```text
runtime · TypeScript                       core
────────────────────                       ────

scheduler  a turn, and only when one
           was asked for
policy     the program's: the moment,
           and the deadline

commands ─────────── SQ ────────────────▶  drained on the rising edge
doorbell ═══════ run(deadline_ms) ═════════▶  ┌ tick   phases in a fixed order
                                           │ tasks  that yield when
                                           │        the deadline has passed
                                           │ asks   what it wants done outside
completions ◀─────── CQ ─────────────────  └ posted before the falling edge
         ◀═══════════ wake_ms ═══════════  next turn · in n ms · never

requests what the turn asked for, run
         before the next rising edge

         … nothing until wake_ms …         … nothing at all …
```

---

## Handles

A handle lets both sides name one object while only one side holds it.

The side that keeps the object hands out a number for it, as an operating system
hands out a file descriptor. That number is a **handle** (`abi/handle`):
`slot | generation << 16`, its slot an index of the table that issued it.

```text
one 32-bit handle
┌──────────────────────┬──────────────────────┐
│      generation      │         slot         │
│     bits 31..16      │      bits 15..0      │
└──────────────────────┴──────────────────────┘
            │                      │
            │                      └─ the far side reads this, and nothing else
            └─ only the issuing table reads this, so a handle kept
               past its release names a generation the slot no longer
               has, and cannot name whatever took the slot next
```

One table issues them (`xpute-core/abi/handle.rs` `Table`): fixed at N slots,
each holding the issuer's value while its handle is live. Slot 0 is never handed
out, so 0 means "none". Handles live in `core` and `runtime`, since a handle is
one side of a boundary.

---

## I/O, by asking

A core cannot do I/O: the runtime owns the connections, as a VMM owns the devices
it implements for its core. So the core asks, in what its turn leaves, and the
runtime hands the ask on and brings back what came of it — in a completion, or into
memory the request named.

How much may be out at once, and which goes first, are the program's; it passes
a bound in its own commands.

### Streams

Some requests come in thousands a turn — every call a renderer makes to a device
it cannot reach — and need neither a reply nor a packet. For those the core
writes a **stream** (`ipc/stream`): records laid end to end during a turn, and
published by storing their count, END, with release. A page's runtime runs them,
all of them, after the turn returns and before it rings again; a native GPU
runtime runs them as END is stored, rung from the publish itself on the core's
thread (`Form::publish`). Either loads END with acquire before it reads a
record, and stores it back to 0 with release only after the last record has
read what it names (wgpu's buffer and texture writes copy their input before
they return); the core touches that memory again only once it has loaded END
as 0 (`ipc/stream` R1 to R3).

```text
0       END    words of records past the head, set as the turn ends
1       HEAD   words before the first record
2…             the rest of the head, the program's
head…   op · n · n words   op · n · n words   …
```

A record is an op the program numbers, a count, and that many words. The stream
says its own head, so a runtime finds the records from the stream alone. The runtime
stores END back to 0 once it has run them, and the core writes the stream
again only after that. Since the whole stream runs between two turns, a record
can name memory for the runtime to write into, and the core reads it next turn:
the order is the acknowledgment.

A record past the range is a sizing bug, and stops the core. Inside a
transaction (`begin`, `commit`), a record that does not fit drops the
transaction whole, and `commit` says so.

### The file device

A core asks its runtime with an **import**, a command of the core's: open,
read, write and close a file, and report the invariant that stopped the core. Each is a frame on the CQ under xpute's majors, posted as
any import is and taken after the turn returns (`ipc/door` `Door::openat`, `read`,
`write`, `close`, `abort`). There is no second way: a program's own question is
a command of its own ABI. The report (`SYS_ABORT`) drops what the turn posted
and goes alone; a core's bug reporter and, in Rust, its panic hook post it
(`stop`), which the core installs (`stop::install`).

Files are a device of xpute's own, major `FS` (`spec/abi/fs.json`, its flags
`spec/ipc/fs.json`), and xpute ships both ends. The machine asks for it (`fs`)
and the runtime attaches it then, backed by the program's storage
(`@xpute/runtime` `boot`): a boot that backs files the machine does not ask for,
or none it does, is refused, and an open or a close from a core without the
device is completed with `ENODEV`. Files borrow Plan 9's file namespace, in POSIX's words:

- the core issues the descriptor, a handle of its own file table (`ipc/file`
  `Files`);
- it names a file by a root the program numbers and a path below it;
- a path walks down a name at a time, and an empty name, `.` or `..` is refused,
  so a root is the whole of what it grants.

An open and a close are requests io_uring's way: the core posts each with
`ACKREQ`, tagged with the descriptor, and the runtime completes each once, on a
later turn, with a frame of the same tag and command flagged `RES` on the
submission ring. An open is completed once the file can be read whole, with its
size or an errno; the door hands it to the program, which finds the open's
owner in its file table (`Files::complete_open`), a failed open leaving no
descriptor. A close completes an open not yet completed with `ECANCELED` first,
and is completed once a file opened for writing is replaced by what was
written, which the runtime reads where the core keeps it: the core keeps a
write's bytes until the close is completed. A read is done before the next
turn and completed by nothing.

The runtime's server runs the calls and asks the program only what a file is
(`@xpute/runtime/ipc/fs.ts` `FsServer`, `Fs`, `FileError`):

```text
the core's turn              the runtime, after it returns the program's Fs
───────────────              ───────────────────────────── ────────────────
FS_READ · _WRITE ─────────▶  FsServer, in the core's memory
FS_OPENAT ────────────────▶  FsServer ──── open ────────▶  Namespace,
◀── its RES, later ────────  FsServer ◀── bytes, or errno  one binding a root
FS_CLOSE ─────────────────▶  FsServer ──── commit ──────▶  Fs
◀── its RES ───────────────  FsServer ◀── 0, or errno
SYS_ABORT ────────────────▶  the mount ─── report ──────▶  log, and the UI
```

The runtime's namespace (`@xpute/runtime/ipc/ns.ts` `Namespace`) is one such `Fs`, its
roots bound when it is made: each call goes to what is bound below the root it
names, `ENOENT` below a root nothing is bound to, `EROFS` for a write below one
that cannot be written. What a file is, where it lies and how it is fetched are
the binding's; xpute only carries the call.

### A call answered before it returns

A frame on the CQ is taken after the turn returns, so a core that needs its
answer to go on waits a turn for it, and a chain of such asks a turn each. Some
need nothing from outside: what answers holds it already, as a page's text
engine holds a line's width. For those a core makes a **call**, an import the
protocol marks `call`: it lays the frame in the CQ's next slot, where nothing
publishes it, and hands the runtime its offset (`ipc/door` `Door::call`). The runtime
hands the frame to what answers its major, as it hands the frames a turn
leaves (`@xpute/runtime/ipc/doorbell.ts` `answer`), and that answers before the
call returns, writing only what the frame names, with a value or `-errno`;
`ENOSYS` where nothing answers. The next frame posted takes the slot.

A call never waits on anything outside: an open, a buffer read back and every
import of xpute's own are frames the turn leaves. It crosses the boundary once
a call where the rest cross once a turn, so it is for the few asks a turn cannot
answer late.

A module imports the entry as `env.call`, beside `env.now`; a core linked
into a native program is handed it with its clock (`BootInfo`). There the core
runs on its own thread, so the native runtime hands each call to the thread that
answers it, the caller waiting in `xpute_run` or else the program, told
(`xpute_notify_on`) and answering on its own thread (`xpute_answer`), the
core's thread waiting meanwhile (`xpute-runtime/native/thread.rs`); the program
says what answers through `xpute_call_on` (`answer_with`), under xpute's
name as the entry is.

### The GPU, a device asked for

A GPU is a device the machine asks for and the runtime attaches, as the file device
is. The machine's `gpu` says what it asks — the stream's bytes, WebGPU's power
preference, the features it must have and those it takes where the adapter has
them — and the runtime opens a device as asked (`@xpute/runtime` `Gpu`, `GpuGrant`,
`GpuAsk`; natively `xpute-gpu/runtime/open.rs` `open`, `Ask::of`). Each time a
device answers, the first time or again after a loss, the runtime tells the core
with `GPU_READY`.

A renderer's calls are what streams were made for, and most of what they say is
WebGPU's, not one program's. So xpute writes them down once, in WebGPU's
vocabulary (`spec/gpu/`). An enumeration is its spec's strings in order, so a
runtime hands the string to the device as it is. A call is an op whose words follow
the descriptor WebGPU takes, with nothing defaulted on the program's behalf.

A core writes them through the encoder (`gpu/encode`), which takes each
object's handle from one table and writes each record twice, once to count its
words and once into the room reserved for them, into the stream the header
names (`GPU_AT`). The **GPU runtime** runs the stream on its device after the turn
returns (`gpu/runtime` `GpuRuntime`): on WebGPU in a page (`@xpute/gpu/web.ts`
`WebGpu`, run by the mount after each turn, `Gpu.after_turn`), on wgpu in a
native program, inside the turn's own call (`@xpute/gpu/native.ts`
`NativeGpu`). What the run throws ends the core, as a trap does. Some calls
finish later — an image copied in, a buffer read back, the queue's work done, a
pipeline made by WebGPU's async call — so each names a fence, a word of the core's
memory and a ticket, the next of a sequence the core alone advances for that
word, one request out on it at a time. Once the call is done, the GPU runtime,
the fence's one writer, stores the ticket there with release, after what
it guards and, for a read back or an image, the result in the word after it;
the core loads the word with acquire and holds the call done where it equals
the ticket it asked with (`ipc/stream` R4, `RESULT`). A core names an async
pipeline in no record until its fence holds its ticket, so nothing it
records waits on the runtime. wgpu has no async call: the native GPU runtime makes
the pipeline at once and writes its fence by the same path.

What a surface shows — a canvas, a view's layer, a window — is lent by the
UI, bound to the GPU runtime under the core's handle for it and let go by that
handle (`bind`, `unbind`); the core names the surface by the handle's slot in
its records. Here the GPU runtime is the side that keeps the object, so it checks
the generation. On a device the view is xpute's (`@xpute/native`
`SurfaceView`).

The native GPU runtime reaches the core's memory by offset (`xpute-runtime/mem.rs`
`Memory`), each read or write only the range the core names, since the runtime's
own blocks may lie in the same memory. Both GPU runtimes pace drawn frames by
one rule (`xpute-gpu/admit.rs` `admits`): a turn draws while fewer of its
drawn frames are unfinished on the GPU than its frame latency, so taking a
surface's texture never blocks and the GPU is never more than that many frames
behind. The native runtime counts the frames it admitted until the queue's
`on_submitted_work_done` (`GpuRuntime::admit`); the page's counts each turn whose
stream took a surface's texture until the queue's `onSubmittedWorkDone`, and
both tell the count in the device's block (spec/gpu DEVICE). The answer reaches
the core with its boot (`Form::admit_frame`), which the program hands its
renderer (`Admit`): beside a native core the runtime's own (`GpuRuntime::admit`),
and in a module the block's count at the program's frame latency
(`admit::page`). The GPU runtime runs by its config (`Config`): the program's numbers
(`Numbers`, the frame latency and the upload chunk) and the platform's
choices, which beside a native core are xpute's defaults
(`runtime/native.rs` `config`): the surface's preferred format, BGRA on an
Apple platform and RGBA elsewhere, where a surface is configured
(`ConfigureAt`), and `Fifo` presentation.

Beside a core linked into its own process, the GPU runtime is a device on the
core's thread: what the device answered is written in before each turn
(`poll`), the stream is run and its frames shown as the core publishes it
(`publish`), and a completion kicks the core (xpute-runtime native `kick`). It
is exported as the entry is, under xpute's names
(`xpute-gpu/runtime/native.rs` `native_abi!`, `rust/gpu/include/xpute_gpu.h`):
`xpute_gpu_open` opens the device and holds it, `xpute_gpu_surface_bind` makes
a surface of a window the platform hands over and binds it under the core's
handle, and `xpute_gpu_surface_unbind` lets it go. It makes a surface's target of
the window itself, a layer on an Apple platform and a native window on
Android, and on Android exports the same calls to JNI under the Kotlin class
`xpute.Gpu`. The GPU runtime is touched on the core's thread alone
(`Core::run`), so a process holds one, as it holds one core.

The GPU's calls stand beside `kit` and the two ends: a program that never
reaches a GPU links none of it. What a program builds over them, such as a
shorthand for its pipelines, stays the program's.

---

## The contract

The contract is the part neither end may change; the rest of this document is
how one end happens to be built.

What cannot change is the rules below and the formats they travel in: the
header, the frame's four words, the ring's layout and the stream's records.
Each rule is one the other end could catch being broken.

The rings keep io_uring's rules:

- **R1.** Each ring has one writer: the runtime the SQ, the core the CQ. Of a
  ring's positions, TAIL is its producer's and HEAD its consumer's.
- **R2.** A position is published with release and read with acquire. A frame
  and its slot are written before the TAIL past them is stored.
- **R3.** A buffer a frame names is the receiver's from the moment the frame is
  published until the receiver moves its HEAD past that frame.
- **R4.** A completion finds its request by id: a reply keeps its request's
  `tag`.
- **R5.** A call never nests, and its answer makes no call.

Around them:

0. **The front.** The runtime lays the header and both rings' headers before the
   core's first turn, as the machine places them. The core holds them to its
   machine, and stops with `ENOEXEC` where a word differs; an unknown MAGIC is
   refused.
1. **The core memory.** Every address that crosses is an offset into it.
   Offset 0 names nothing, and nothing that crosses lies there.
2. **The turn.** The core runs only inside `run`. The deadline is the core's to keep:
   a core can run past it inside one function, and the runtime cannot take
   the turn back until it returns. `wake_ms` is 0 for the next
   turn, a positive number for that many milliseconds from now, and negative
   (`NO_WAKE`) for none. A turn with a deadline of 0 is still a turn, and a question the
   runtime needs answered at once is asked in one, of a command the core answers
   from what it already holds; one that waits on anything outside is answered
   on a later turn.
3. **The frame.** Four words: `tag`, `cmd` with its flags, `result`, and the
   packet. With `ACKREQ` a submission gets exactly one reply; without it, a
   reply only when it failed. A reply carries its submission's `tag` and `cmd`,
   and `RES`. Majors under `PROGRAM_MAJOR` are xpute's own.
4. **What the runtime sends** is applied in order, each submission once. The core
   may leave some for a later turn, never out of order or skipped, and asks for
   the next turn while it does.
5. **What a turn leaves** — replies, imports, stream records — is
   taken by the runtime before the next turn, none dropped. Order holds within one
   place a turn writes to, not between places, so a core that needs one request
   run before another puts both in the same place. Running past the room set
   aside is a sizing mistake, and stops the core.
6. **Handles.** A handle is `slot | generation << 16`, 0 is never one, and the
   issuing side checks the generation.
7. **The clock.** During a turn the core reads the runtime's clock (`now`) in
   milliseconds. It never runs backward, and only differences mean anything.
8. **A call.** During a turn the core may hand the runtime the offset of a frame
   it laid, unpublished, in the CQ's next slot. What answers the frame's major
   answers it before the call returns, reads that frame and writes only what
   it names, with a value or `-errno`, `ENOSYS` where nothing answers. It
   answers from what it already holds and waits on nothing outside, and the
   core runs nothing until it has.

Everything else is one end's own. The core's: where its ranges lie past the
front, its heap, its tick, passes, tasks and files. The runtime's: the size of a
deadline, when to ring, how it reads what a turn left.

The contract describes one runtime and one core. Several cores under one runtime,
passing buffers by ownership, do not change it. Where they share a thread or do
not trust each other, that composition must bring what the contract does not:
isolation, and a way to stop a core that does not return.

Open: R1 to R3 are being carried through every end: atomics on every
position, the GPU stream's END reset by its consumer, and a buffer a frame
names released by the head, not by the next turn. Whether a native core on
its own thread keeps `run(deadline) → wake` as its contract is open.

---

## Written in several languages

The implementations do not all stand in the same relation, and which relation
decides what must match.

- **`kit` is symmetric.** Rust (`rust/kit`), TypeScript (`ts/kit`) and C++
  (`cpp/xpute/kit`) implement the same kit, held to each other by the golden
  records wherever bytes must match. TypeScript's has no `alloc`, since it has
  no linear heap of its own, and no `bug`, since a thrown error carries its
  stack.
- **The two ends face each other.** A runtime end and a core end owe each other
  the contract and its formats; their module lists differ, and a module with
  nothing facing it is whole.
- **Core ends are symmetric with each other.** Rust (`rust/core`) and C++
  (`cpp/xpute/core`) are the same end, either plugged into any runtime, so what
  one gives a program the other gives too.
- **Runtime ends hold what their platform needs.** TypeScript's (`ts/runtime`) boots
  a core as its machine declares (`@xpute/runtime` `boot`), opening it by the
  build's platform, picked by file: `platform.ts`, the page's, reserves a
  module's memory and opens the module over it (`module.ts`), and
  `platform.native.ts`, the device's, reaches the core linked into the app
  through `@xpute/native`. It then mounts on it in one call (`mount`), given
  all the program decides — what backs its files, what hears its own commands
  (`program`), its GPU, its policy, what it submits before each turn, what is
  told when the core stops — and nothing of the order the runtime is put
  together in: it lays the front, binds xpute's majors to their servers, grants
  the GPU, sends `SYS_ATTACH` before anything else and runs the GPU's stream
  as each turn returns. What it gives speaks with the doorbell's verbs, `send`
  and `call`, each asking the scheduler for a turn after, `fits`, `wake`,
  `resume`, `pause` and `visible`, which tells the core at once
  (`SYS_SUSPEND`, `SYS_RESUME`), and `bind` and `unbind`, a canvas lent to
  the GPU. Rust's (`rust/runtime`) reads a core's memory by offset on
  every platform (`mem`), and on a unix one runtimes a core linked into the
  runtime's own native process (`native`). There it reserves the memory, and runs
  the core on its own thread, which loops while the core has work, since the
  core keeps its module-wide values in `Global`s (`xpute-core/global.rs`),
  statics sound on one thread alone, handing each call to the thread that
  answers it. It also provides the allocator that keeps the core's blocks in
  its memory and every other thread's out of it. The React Native module (`ts/native`) holds
  the Kotlin, the Swift and the headers that bind the two, and Expo autolinks
  it.
- **The GPU's calls follow the same split.** Each core language writes them
  (`rust/gpu`, `cpp/xpute/gpu`). The GPU runtimes run them in TypeScript on WebGPU
  (`ts/gpu`), and in Rust on wgpu behind the `runtime` feature.
- **A core built as a module is linked one way.** Its memory is imported from
  the runtime and its stack goes first, at the bottom of the memory, so an overflow
  traps; its data begins past the front (`global_base`), and it declares the
  smallest of its machine's sizes as its least. Rust's build script prints the
  flags (`xpute-core/wasm/link.rs` `link`, `Machine::link`), and the C++ core
  is linked with the same (`cpp/Makefile` `WASM_LINK`).

`kit` asks nothing of a program: link it and call it. `core` and `runtime` ask it
to run xpute's way — memory granted once, work in turns, I/O by asking.

C++ is written from the contract and the formats, beside the Rust: built by the
platform's compiler (`cpp/Makefile`), and held by the same records and traces,
so the cores agree on what the boundary shows. Where C++ differs, it is the
language:

- it writes the codecs its standard library lacks;
- its XTP reads a packet without allocating, handing back views;
- a task is a C++20 coroutine where Rust's is a future;
- it has no `Global`, which Rust needs a type for and C++ has as a plain static;
- it uses RAII, templates and `Result`, with no exceptions and no RTTI.

Its numbers — the errno table, the header's words, a frame's and a ring
header's words — come from `spec/` as the other languages' do.

---

## The golden records

The golden records are what hold the languages to each other.

Under `spec/golden/` are lines of `path<TAB>value`, produced once, checked once,
and frozen. They are read by splitting each line (`kit` `golden`), so no parser
change can make them drift. Each language that implements a part reads that
part's records bit for bit, so a record is never re-recorded to make a test
pass: a difference means one side moved, and a record changed on purpose changes
for every language at once. There are two kinds:

- **Across a protocol** — `wire/xtp`, `wire/tlv`, `codec/json` (which texts are
  JSON at all, and what a number past the doubles reads as, as the runtime's own
  `JSON.parse` judges them), and `gpu/programs` (the words each encoder writes
  and the bytes each GPU runtime reads back). The bytes cross a boundary, so every
  language must produce these exactly.
- **Across languages** — `math/rng` (`rng.32` and `rng.64`), `math/scalar`,
  `collection/deque` and `collection/heap`. Nothing crosses a boundary, but one
  set of values holds every language's implementation: the same draws, the same
  rounding, the same order.

A GPU's floats are held only as far as WGSL holds them. Its `+`, `−` and `×`
round half to even, as every language here does. Its division may be 2.5 ulps
off, and its square root, `length` and the transcendentals have only bounds; a
GPU may be further off than the spec allows (a division has been measured 13
ulps off). WGSL also cannot forbid a product fused into the sum after it, and
a compiler may go further: Metal's, in its default fast math, reorders and
simplifies a sum, so no order written in the source holds. The compiler is
the browser's or the driver's, not the program's. So which operations a rule
takes depends on who decides it:

- **A rule the CPU and a GPU both decide** decides in integers, or in floats
  the CPU rounded once and handed the GPU in a word: a division by a value is
  a product by its reciprocal, a square root the root, each handed. What it
  still sums in floats takes only `+`, `−`, `×`, a power of two's scaling and
  the same constants, and its test names the ulps it allows at the edge and
  counts what took them.
- **A rule a GPU alone decides** is one function every pass calls, so the GPU
  agrees with itself; its test names the ulps it allows.

The **traces** under `spec/conformance/` are a third kind, not outputs at all.
Each says what a runtime does and what it must then see: the completions, stream
records and next-turn asks each turn leaves. They are written from the
contract's rules and name only what a runtime can observe, so they hold every core
to the same thing. The file device is a service the boundary carries, outside
the contract, so its traces stand apart: `spec/conformance/fs/` holds the
runtime's end, and `spec/conformance/file.tsv` every core.

One runtime runs them against every core through the boundary alone
(`@xpute/runtime/conformance.test.ts`): the Rust core
(`xpute-conformance/core.rs`) and the C++ one (`cpp/conformance/core.cpp`),
each over the commands `spec/conformance/core.json` numbers. Each core's boot
takes a seed and lays the front, the header among it, in the order and with the
gaps the seed draws (`LAYOUTS`), and the runtime runs every trace under each: only
a runtime that finds the rings through the header, as rule 0 asks, passes. Each
core is built both as a native library and as a WebAssembly module that
imports nothing but its memory, the clock and the call; the C++ one from
the library's amalgamation (`cpp/amalgamation.awk`), the two files a C++
program takes, so what the traces hold is what ships.

## Prior art

What xpute took from work before it, or found it had arrived at. A link is given
where the tree records the source.

| Precedent                                                   | Where                            | What is taken                                                                  |
| ----------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------ |
| A unikernel (MirageOS, Unikraft) and its library OS         | the core                         | The program and its library OS as one image, with no userspace                 |
| A VMM and virtio's devices                                  | the runtime, the machine         | The runtime implements the devices the machine asks for; interrupts up         |
| virtio-vsock                                                | the UI                           | A UI outside the VM reaching the core's service over a device                  |
| A WIT world                                                 | the two ABIs                     | A world's imports and exports, seen from the core                              |
| Linux's scheduler (`schedule`, `try_to_wake_up`)            | `sched/scheduler`                | Its name: what runs a turn at the moment the policy gives                      |
| Linux's scheduling policies (`sched_setscheduler`)          | `sched/policy`                   | Its name: how turns are scheduled, chosen apart from what rings them           |
| Android's Frame Pacing library (Swappy)                     | `sched/frame_pacing`             | Its name, and turns a whole number of refreshes apart                          |
| "core" for main memory, as in a core dump                   | the core memory                  | Its name                                                                       |
| A linker script's output sections                           | `mem/section`                    | Ranges placed end to end without knowing what they hold                        |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `alloc/buddy_tree`               | The page allocator, ported line for line                                       |
| SLUB                                                        | `alloc/slab`                     | A slab keeps its free list inside its free slots                               |
| virtio's descriptor table, AF_XDP's UMEM                    | the rings (`spec/ipc/ring.json`) | Descriptors kept apart from the payload's slots                                |
| io_uring                                                    | the rings, the front             | SQ and CQ by its roles, its ordering rules, and the setup laying the rings     |
| ublk                                                        | the CQ                           | A server's own requests posted on the completion side                          |
| NVMe's queues and their doorbell registers                  | the doorbell                     | Its name, and that ringing it carries no message                               |
| NVMe's asynchronous events                                  | a frame without `RES`            | An import the core posts unasked                                               |
| KVM's `KVM_RUN`                                             | the turn                         | `run`: the call that runs the core until it gives the processor back          |
| Plan 9's file access, in POSIX's words                      | `ipc/fs`, `ipc/ns`               | Descriptors the core issues; a root and a path; the runtime binds the roots      |
| POSIX and Linux errno                                       | `spec/status/errno.json`         | Negative errno numbers                                                         |
| The host's `JSON.parse`                                     | `codec/json`                     | The grammar, judged as the host's parser judges it                             |
| IEEE 754's round half to even, as WGSL rounds               | `math/scalar`                    | One rounding in every language                                                 |
| splitmix64                                                  | `math/rng`                       | A reproducible generator, its seed's finalizer, and its high half as a u32     |
| Murmur3's 32-bit finalizer                                  | `math/rng`                       | The 32-bit family's mix, for a GPU with no 64-bit integer                      |
| FNV                                                         | `codec`                          | The hash                                                                       |
