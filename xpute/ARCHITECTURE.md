# Architecture — xpute

[xpute](./README.md) makes a program's core its first-class citizen: the core
leads, the same compiled code on the web and natively, and JavaScript does the
I/O and the UI around it. The two share one memory, and a pair of rings in it
carries everything else, with no foreign function call for each piece of the
boundary. The core is the **guest**, as WebAssembly means it, run inside; the
**host** is what runs it.

The core sees one thing outside it, its **world**, whose commands are its
**imports**, which it calls, and its **exports**, which it handles, as a WIT
world says; it never names a destination. A platform's author sees the world
in two parts, and the stack reads **ui**, core, host: an application over a
kernel over its hardware. The ui submits commands to the core and the core
completes them, as an application and io_uring's kernel do; what the core
sends the ui unasked, more rarely, is an upcall. That relation is the app's
ABI. The host is the core's hardware, its HAL: the memory, the clock and the
turns, the GPU, the storage. The core calls it, and it raises interrupts. That
relation is xpute's predefined ABI.

Physically the ui and the host share one JavaScript runtime, and natively the
host has Rust pieces beside it: the guest's thread, its memory and the GPU's
run. Both relations ride the same ring pair, told apart by a command's major;
the host carries the ui's submissions on the submission ring without reading
them. The ui meets the host directly only for the surfaces (a view lent to the
GPU) and the lifecycle (boot, visibility), through the host's TypeScript API.

The host grants real resources, a real region and a real quota, and the guest
builds its own abstractions over them, as a library operating system does over
an exokernel; nothing is virtualized, and xpute does not enforce the grants.

xpute comes in two layers. `kit` is generic code — collections, codecs, a wire
format, an errno table, scalar math — that any program can link and call.
`guest` and `host` are the arrangement itself, one end each, written against
`kit`, which knows nothing of them. Neither layer knows what a guest computes.

Three things cross between the ends and nothing else does: one memory they
share, one call in, one number out. Two calls go the other way, each answered
before it returns: the host's clock, and a call the guest makes of its world,
answered from what is already held (both below). The sections follow from
that:

1. the memory, the machine that sizes it and the front the host lays in it;
2. what is written into it — one shape for every message, its data, the two
   ABIs, and the rings that carry them each way;
3. the call that makes it run, the turn;
4. handles, which name objects across the boundary;
5. streams, files, calls and the GPU, which a guest asks of its host;
6. the contract all of it comes down to;
7. the languages, and the records that hold them to each other.

---

## The core memory, placed once

The memory is the one thing the two ends share. It is the **core memory**, core
as in a core dump. The host reserves it at boot and it never grows, so a view
into it stays valid for as long as the guest lives: a ring keeps its words at a
fixed address, and a payload is read where it lies. A page reserves it as a
`WebAssembly.Memory` whose initial size is its maximum
(`@xpute/host/mem/reserve.ts` `reserve_mem`); a native program that links its
guest in maps it from the system, and the guest's own thread then sets its base
(`xpute-host/native.rs`, `xpute-guest/mem.rs` `set`).

The memory need not begin at address zero. A WebAssembly module's does; a guest
linked into a native program has its memory wherever the system put it. So every
address a guest names is an offset from the memory's base, never a pointer
(`mem`, `Memory`). It means the same to the host however the guest is built, and
it crosses as a `u32` however wide the machine's pointers are. A part laid over
a region takes the `Memory` it lies in and its offset, so a test lays it over a
buffer of its own; a program reading its own data goes through the guest's own
memory, set once (`core`, `at`).

How big the memory is, and what lies at its front, both ends must agree on
before either runs. A guest crate declares it once, as its **machine**
(`machine.json`, its schema `spec/machine.schema.json`, generated as `MACHINE`
for both ends; `xpute-guest/machine.rs` `Machine`): the sizes a host may give
and the unit a reservation is counted in, the heap the smallest must leave, a
module's stack and a native guest's thread's stack, the rings' capacities and
slot size, what one turn posts besides its replies, and the GPU it requests.

The front follows from the machine, the same on both ends: the stack, a page
for the **header**, the two rings and their slots, the GPU stream, then where
a module's linker may place its data (`global_base`). The host lays the front
before the first turn, as io_uring's setup lays its rings: the header's words
(`spec/ipc/head.json`; `@xpute/host/ipc/head.ts` `lay`, `Front`) and each
ring's own header, its positions at 0. A guest that outlived its host's reload
keeps the front it has (`laid`). On its first turn the guest computes the
front from its machine and holds what the host laid to it
(`xpute-guest/machine.rs` `boot`): where a word differs it stops with
`ENOEXEC`, the word's place and what it holds. The header names what the host
laid, not where to look: MAGIC and VERSION, the rings' sizes and places, and
the GPU stream's.

```text
one guest's memory; the front is the machine's, the rest the guest's

 0 ┌─────────────────────────────────┐
   │ a module's stack                │
   ├─────────────────────────────────┤ head_at
   │ header                          │
   │   MAGIC · VERSION               │
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
   │ GPU stream, where requested     │◀───────┘
   ├─────────────────────────────────┤ global_base
   │ a module's data                 │
   ├─────────────────────────────────┤
   │ the guest's fixed ranges        │
   ├─────────────────────────────────┤
   │ heap, to the memory's end       │
   └─────────────────────────────────┘
```

Past the front, what goes in the memory is the guest's, and so is where. It
lays ranges end to end on a unit it chooses (`mem/section`), as a linker script
places output sections without knowing what they hold, and whatever is asked of
a range is checked against its end (`Section::at`, `nth`); a range sized at
compile time is handed to its owner once at boot and read as an array laid in
it (`mem/range` `OwnedRange`, `arr`). The machine checks at compile time that
the smallest size holds the front, the data, the fixed ranges and the heap it
asks for (`Machine::check`).

The heap is reached through a single call (`mem/heap`) whose answer is the
guest's choice. `kit` offers two allocators that fit together: a buddy allocator
of fixed pages (`alloc/buddy_tree`), ported from evanw/buddy-malloc, and a slab
of size classes over it (`alloc/slab`), which keeps its bookkeeping inside the
blocks it has not handed out.

---

## The frame, and the packet it carries

Sharing a memory does not say what a submission or its reply looks like, so
xpute gives every message between the ends one shape, the **frame**, of four
words (`ipc/frame`, `spec/ipc/frame.json`):

- `tag`, the sender's id, which a reply keeps, or an event's subject;
- `cmd`, what is asked, in the low 16 bits, with flags in the high 16;
- `result`, what came back, a value or `-errno`;
- where its **packet** lies — the data the frame carries — or 0 for none.

An errno is one table's (`spec/status/errno.json`), and every language reads a
code back into its member, or into none (`Errno::of` in Rust, `errno_of` in the
others).

Two flags decide what a frame means. `ACKREQ`, set by whoever submits, asks for
a reply. `RES`, set by whoever completes, says the frame is a reply. A
completion without `RES` is an **event**, one the guest raised on its own, as
an NVMe controller posts an asynchronous event to its completion queue.

### Two ABIs, one number space

A command is `major << 8 | minor` (`kit` `abi/cmd`), and who defines it decides
its major. xpute's **predefined ABI** takes majors 0x00 to 0x0F
(`spec/abi/cmd.json`, `spec/abi/fs.json`). Its imports are the guest's calls to
its HAL: the file device's (`FS_OPENAT`, `FS_READ`, `FS_WRITE`, `FS_CLOSE`) and
`SYS_ABORT`, the report the guest posts as it stops. Its exports are the HAL's
interrupts: `SYS_ATTACH`, a host attached, the first time or again;
`SYS_SUSPEND` and `SYS_RESUME`, calls around a hidden host; `GPU_READY`, the
GPU the machine asked for answered; and `FS_OPENED`, an open answered. `NOP` is
the rings' own test. What each life command means to keep or reset is the
guest's.

The **app's ABI** takes majors from `PROGRAM_MAJOR`, 0x10, in protocol files of
the app's own, each command an `export` or an `import`, `call` where its sender
waits. The generator refuses an app's command under `PROGRAM_MAJOR`, and a
number two protocols both give (`xpute-tool spec/cmd.rs`). Nothing in a frame
names a port or a destination: the host hands a frame to what answers its
command's major (`@xpute/host/host.ts` `mount`), and the guest's door applies
xpute's own and hands the program every other.

The generator writes both ends' code from the machine and the protocol files
(`xpute-gen`, `xpute-tool bin/gen.rs`; xpute's own by `cargo spec`): for each
protocol its messages (encoders, readers, enums), and its bindings, on the host
a stub per export and a handler per import (`<name>_exports`,
`<name>_imports`), on the guest a function per import and a trait for the
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
written by the host, the side where the outside's interrupts arrive: the ui's
submissions and the HAL's own interrupts. The **CQ**, the completion ring, is
written by the guest: its completions and upcalls, and its own calls to the
HAL, posted as ublk's server posts its requests, whose results come back on the
SQ. Each lies where the header says (`SQ_AT`, `CQ_AT`).

The host drives the pair through the **doorbell**
(`@xpute/host/ipc/doorbell.ts` `Doorbell`):

- it writes each submission in place, into the SQ's next slot (`writer`,
  `send`);
- it rings the guest with a quota (`ring`), which is one whole turn;
- it then reads the CQ dry, handing each reply to the call waiting for it by
  its tag and every other frame to its one listener (`hear`), which the host
  routes by major.

Two ways in sit on top of that. A question the host needs answered at once is a
`call`: submitted with `ACKREQ`, tagged with the next id, and rung with no
quota until its reply comes back. A command from outside the host's own code,
such as a developer's console, is `cmd`: its arguments become the packet's
fields (`TreeView.set`), and it is sent and rung. A guest that traps is gone,
and the doorbell rings it no more.

The guest's end is the **door** (`ipc/door` `Door`), opened on the first turn
over the rings the host laid (`xpute-guest/machine.rs` `boot`, `door`). It
applies the submissions in order, answers each, and posts what the guest raises
into the CQ (`send` for an event its table generates, `Event`, or `send_with`
for a packet written in place), which holds a whole turn's output. What is
xpute's it applies itself (`NOP`), and it hands the program the rest
(`receive`, `Submission`): the guest's life (`Life`: `Attach`, `Suspend`,
`Resume`), its GPU answered (`GpuReady`), an open the host completed
(`Opened`), and every command of the app's (`Program`). A drain runs before
any phase's budget, so the program records what it is handed and returns.
Before each submission the door checks that the CQ has room for the most one
command may leave — its events, the machine's `turn_posts`, and its reply —
and where it has not, it applies no more and leaves the rest for the next turn,
in order.

---

## The turn

The turn is when the guest runs at all.

It runs inside one call, `run(quota_ms)`, named as KVM's `KVM_RUN` is, the call
that runs a guest until it gives the processor back; when that call returns the
guest is not running. The call has two edges, like a clock's. On the **rising
edge** the host calls in with a quota, and the guest drains the SQ, does its
work and posts what it has. On the **falling edge** it returns `wake_ms`, when
it would like the next turn: `0` for the next the host can run, `n` for that
many milliseconds, `NO_WAKE` for none. The host learns nothing else about what
ran.

xpute names that call, as a C runtime names the `main` it calls: the program
implements it and never renames it (`xpute-guest/entry.rs` `Entry`, `run`). A
guest built as a module exports `run`; a guest linked into a native program
exports `xpute_run`, and its memory as `xpute_mem`, under xpute's name and no
program's, since a process holds one guest, with one reservation
(`xpute-host/native.rs`). What the guest's form hands it there is xpute's as
well (`BootInfo`): the memory it was given, where its own ranges may begin, the
host's clock, its hostcall, its heap's reading, where the guest shares the
host's process the hook that says why it stopped, and where its GPU host may
block taking a frame that host's admission.

A guest's form follows its target, so a program writes one macro over its
machine and builds one crate for every target
(`xpute-guest/machine.rs` `guest!`): on wasm32 the module's form
(`xpute-guest/wasm.rs` `wasm_abi!`), its link flags printed by its build script
from the machine (`Machine::link`); anywhere else the native form, which is
xpute's as a module's is (`xpute-host/native.rs` `native_guest!`): its memory
laid out from its machine, its thread, its allocator, xpute's stop hook, the C
ABI (`rust/host/include/xpute.h`) and on Android the same calls to JNI under
the Kotlin class `xpute.Guest`. A guest given its GPU's numbers takes the GPU
host's form as well (`xpute-gpu/host/native.rs` `native_guest!`). The crate is
the artifact, built as `rlib`, `cdylib` and `staticlib`; a React Native app
links it through xpute's native module (`ts/native`, `@xpute/native`), which
builds it for the device as the app is built (`script/build-core.py`).

When each side runs is the platform's. On a page the guest and the host share
one thread, the page's event loop, and take turns. Something on the host side
has to decide when to ring at all. That is the **strobe**
(`@xpute/host/sched/strobe.ts` `Strobe`), named for a circuit's strobe, the
pulse that tells a receiver when to act: the web host's policy. It rings only
when the host or the guest's last falling edge asked for a turn, at the moment
the scheduling policy below gives; a positive `wake_ms` sets a timer that asks
for a turn when it fires. A guest that asks for nothing is rung for nothing.
The program says only whether turns may run (`Host.resume`, `pause`, each a
no-op where it already holds) and asks for one where something the guest did
not hear of changed (`wake`); once the guest stops, the strobe rings it no
more, and the mount tells the program (its config's `onterminated`). The
doorbell rings too, with no quota: to make room in a full SQ, to wait for a
call's reply and to send a `cmd`. Natively every call into the guest runs on
the guest's own thread (`xpute-host/native/thread.rs`).

How long a turn may take, and how that time is spent, are settled at three
levels, from the outside in.

1. **The scheduling policy** (`@xpute/host/sched/policy.ts`
   `SchedulingPolicy`) is the host's, and the program injects it, with its
   numbers, when the host boots; xpute keeps none of its own. It is what the
   strobe asks: it gives and admits the moment a turn may run, sets the quota, a
   cooperative slice ended by the guest's return, and is handed what each turn
   took; the doorbell's turns are outside it. A page's is **frame pacing**
   (`sched/frame_pacing.ts`), named for Android's Frame Pacing library: turns a
   whole number of the display's refreshes apart, each given what its frame
   leaves. A host with no display gives each turn a fixed quota as soon as it is
   asked (`sched/fixed_quota.ts`).
2. **Tick** (`sched/tick`) is the guest's. It runs the program's phases, by
   their place in its order, against one budget, the quota counted from the
   rising edge. The program's policy gives each phase the share of the budget
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
language's form, a future or a coroutine. A task gets the turn's quota as a
deadline, `yield_if_spent` to stop once the deadline has passed, and `next_turn`
to wait for the next turn; the guest may move the deadline past the quota
(`extend_to`). A parked task makes the falling edge 0. Whatever assembles the
guest installs an executor (`sched/task`), marks the edges with `rise` and
`fall`, and polls it between them.

```text
host · TypeScript                          guest
─────────────────                          ─────

strobe   a turn, and only when one
         was asked for
policy   the program's: the moment,
         and the quota

commands ─────────── SQ ────────────────▶  drained on the rising edge
doorbell ═══════ run(quota_ms) ═════════▶  ┌ tick   phases in a fixed order
                                           │ tasks  that yield when
                                           │        the quota is spent
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

One table issues them (`xpute-guest/abi/handle.rs` `Table`): fixed at N slots,
each holding the issuer's value while its handle is live. Slot 0 is never handed
out, so 0 means "none". Handles live in `guest` and `host`, since a handle is
one side of a boundary.

---

## I/O, by asking

A guest cannot do I/O, because the host owns the connections, as hardware does
under a kernel. So the guest asks, in what its turn leaves, and the host hands
the ask on and brings back what came of it — in a completion, or into memory
the request named.

How much may be out at once, and which goes first, are the program's; it passes
a bound in its own commands.

### Streams

Some requests come in thousands a turn — every call a renderer makes to a device
it cannot reach — and need neither a reply nor a packet. For those the guest
writes a **stream** (`ipc/stream`): records laid end to end during a turn, which
the host runs, all of them, after the turn returns and before it rings again.

```text
0       END    words of records past the head, set as the turn ends
1       HEAD   words before the first record
2…             the rest of the head, the program's
head…   op · n · n words   op · n · n words   …
```

A record is an op the program numbers, a count, and that many words. The stream
says its own head, so a host finds the records from the stream alone. The host
stores END back to 0 once it has run them, and the guest writes the stream
again only after that. Since the whole stream runs between two turns, a record
can name memory for the host to write into, and the guest reads it next turn:
the order is the acknowledgment.

A record past the range is a sizing bug, and stops the guest. Inside a
transaction (`begin`, `commit`), a record that does not fit drops the
transaction whole, and `commit` says so.

### The file device

A **hostcall** is a guest's call into its host, the name Wasmtime and Lucet give
one: open, read, write and close a file, and report the invariant that stopped
the guest. Each is a frame on the CQ under xpute's majors, posted as an event
is and taken after the turn returns (`ipc/door` `Door::openat`, `read`,
`write`, `close`, `abort`). There is no second way: a program's own question is
a command of its own ABI. The report (`SYS_ABORT`) drops what the turn posted
and goes alone; a guest's bug reporter and, in Rust, its panic hook post it
(`stop`), which the guest installs (`stop::install`).

Files are a device of xpute's own, major `FS` (`spec/abi/fs.json`, its flags
`spec/ipc/fs.json`), and xpute ships both ends. They borrow Plan 9's file
namespace, in POSIX's words:

- the guest issues the descriptor, a handle of its own file table (`ipc/file`
  `Files`);
- it names a file by a root the program numbers and a path below it;
- a path walks down a name at a time, and an empty name, `.` or `..` is refused,
  so a root is the whole of what it grants.

An open is the one call that waits: the host answers it with `FS_OPENED` on a
later turn, once the file can be read whole, and the door hands it to the
program, which finds the open's owner in its file table
(`Files::complete_open`), a failed open closed there. A read, a write and a
close are done before the next turn, and a file opened for writing is replaced
at its close by what was written.

The host's server runs the calls and asks the program only what a file is
(`@xpute/host/ipc/fs.ts` `FsServer`, `Fs`, `FileError`):

```text
the guest's turn             the host, after it returns    the program's Fs
────────────────             ──────────────────────────    ────────────────
FS_READ · _WRITE · _CLOSE ▶  FsServer, in the guest's memory
FS_OPENAT ────────────────▶  FsServer ──── open ────────▶  Namespace,
◀──── FS_OPENED, later ────  FsServer ◀── bytes, or errno  one binding a root
FS_CLOSE of a write ──────▶  FsServer ──── commit ──────▶  Fs
SYS_ABORT ────────────────▶  the mount ─── report ──────▶  onabort
```

The host's namespace (`@xpute/host/ipc/ns.ts` `Namespace`) is one such `Fs`, its
roots bound when it is made: each call goes to what is bound below the root it
names, `ENOENT` below a root nothing is bound to, `EROFS` for a write below one
that cannot be written. What a file is, where it lies and how it is fetched are
the binding's; xpute only carries the call.

### A call answered before it returns

A frame on the CQ is taken after the turn returns, so a guest that needs its
answer to go on waits a turn for it, and a chain of such asks a turn each. Some
need nothing from outside: what answers holds it already, as a page's text
engine holds a line's width. For those a guest makes a **call**, an import the
protocol marks `call`: it lays the frame in the CQ's next slot, where nothing
publishes it, and hands the host its offset (`ipc/door` `Door::call`). The host
hands the frame to what answers its major, as it hands the frames a turn
leaves (`@xpute/host/ipc/doorbell.ts` `answer`), and that answers before the
call returns, writing only what the frame names, with a value or `-errno`;
`ENOSYS` where nothing answers. The next frame posted takes the slot.

A call never waits on anything outside: an open, a buffer read back and every
hostcall of xpute's own are frames the turn leaves. It crosses the boundary once
a call where the rest cross once a turn, so it is for the few asks a turn cannot
answer late.

A module imports the entry as `env.hostcall`, beside `env.now`; a guest linked
into a native program is handed it with its clock (`BootInfo`). There the guest
runs on its own thread while the caller of `run` waits, so the native host hands
each call back to that caller, which answers it on its own thread and waits
again (`xpute-host/native/thread.rs`); the program says what answers through
`xpute_hostcall_on` (`answer_with`), under xpute's name as the entry is.

### The GPU, a resource granted

A GPU is a resource the machine requests and the host grants, as memory is.
The machine's `gpu` says what it asks — the stream's bytes, WebGPU's power
preference, the features it must have and those it takes where the adapter has
them — and the host opens a device as asked (`@xpute/host` `Gpu`, `GpuGrant`,
`GpuAsk`; natively `xpute-gpu/host/open.rs` `open`, `Ask::of`). Each time a
device answers, the first time or again after a loss, the host tells the guest
with `GPU_READY`.

A renderer's calls are what streams were made for, and most of what they say is
WebGPU's, not one program's. So xpute writes them down once, in WebGPU's
vocabulary (`spec/gpu/`). An enumeration is its spec's strings in order, so a
host hands the string to the device as it is. A call is an op whose words follow
the descriptor WebGPU takes, with nothing defaulted on the program's behalf.

A guest writes them through the encoder (`gpu/encode`), which takes each
object's handle from one table and writes each record twice, once to count its
words and once into the room reserved for them, into the stream the header
names (`GPU_AT`). The **GPU host** runs the stream on its device after the turn
returns (`gpu/host` `GpuHost`): on WebGPU in a page (`@xpute/gpu/web.ts`
`WebGpu`, run by the mount after each turn, `Gpu.after_turn`), on wgpu in a
native program, inside the turn's own call (`@xpute/gpu/native.ts`
`NativeGpu`). What the run throws ends the guest, as a trap does. Some calls
finish later — an image copied in, a buffer read back, the queue's work done, a
pipeline made by WebGPU's async call — so each names a word of the guest's
memory and a ticket, a number the guest wrote there; once the call is done, the
GPU host writes the ticket back with `DONE` set. A guest names an async
pipeline in no record until its word says `DONE`, so nothing it records waits
on the host. wgpu has no async call: the native GPU host makes the pipeline at
once and writes its word by the same path.

What a surface shows — a canvas, a view's layer, a window — is lent by the ui,
bound to the GPU host under the guest's handle for it and let go by that handle
(`bind`, `unbind`); the guest names the surface by the handle's slot in its
records. Here the GPU host is the side that keeps the object, so it checks the
generation. On a device the view is xpute's (`@xpute/native` `SurfaceView`).

The native GPU host reaches the guest's memory by offset (`xpute-host/mem.rs`
`Memory`), each read or write only the range the guest names, since the host's
own blocks may lie in the same memory. It also paces drawn frames, which a
browser does for a page: a turn draws while fewer of the drawn frames it
admitted are unfinished than its frame latency (`GpuHost::admit`), so taking a
surface's texture never blocks. The answer reaches the guest with its boot
(`BootInfo::admit`), which the program hands its renderer
(`xpute-gpu/admit.rs` `Admit`); a page's GPU host admits every drawn frame
(`every`). The GPU host runs by its config (`Config`): the program's numbers
(`Numbers`, the frame latency and the upload chunk) and the platform's
choices, which beside a native guest are xpute's defaults
(`host/native.rs` `config`): the surface's preferred format, BGRA on an
Apple platform and RGBA elsewhere, where a surface is configured
(`ConfigureAt`), and `Fifo` presentation.

Beside a guest linked into its own process, the GPU host runs at the edges of
each turn on the guest's thread, what the device answered written in before it
and the turn's stream run and its frames shown after it (`before_turn`,
`after_turn`), and is exported as the entry is, under xpute's names
(`xpute-gpu/host/native.rs` `native_abi!`, `rust/gpu/include/xpute_gpu.h`):
`xpute_gpu_open` opens the device and holds it, `xpute_gpu_surface_bind` makes
a surface of a window the platform hands over and binds it under the guest's
handle, `xpute_gpu_surface_unbind` lets it go, and `xpute_gpu_wake_on` takes
the function a completion calls to ask a turn. It makes a surface's target of
the window itself, a layer on an Apple platform and a native window on
Android, and on Android exports the same calls to JNI under the Kotlin class
`xpute.Gpu`. The GPU host is touched on the guest's thread alone
(`Guest::run`), so a process holds one, as it holds one guest.

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

- **R1.** Each ring has one writer: the host the SQ, the guest the CQ. Of a
  ring's positions, TAIL is its producer's and HEAD its consumer's.
- **R2.** A position is published with release and read with acquire. A frame
  and its slot are written before the TAIL past them is stored.
- **R3.** A buffer a frame names is the receiver's from the moment the frame is
  published until the receiver moves its HEAD past that frame.
- **R4.** A completion finds its request by id: a reply keeps its request's
  `tag`.
- **R5.** A call never nests, and its answer makes no call.

Around them:

0. **The front.** The host lays the header and both rings' headers before the
   guest's first turn, as the machine places them. The guest holds them to its
   machine, and stops with `ENOEXEC` where a word differs; an unknown MAGIC or
   VERSION is refused.
1. **The core memory.** Every address that crosses is an offset into it.
   Offset 0 names nothing, and nothing that crosses lies there.
2. **The turn.** The guest runs only inside `run`. The quota is advice, not a
   deadline: a guest can spend a whole quota inside one function, and the host
   cannot take the turn back until it returns. `wake_ms` is 0 for the next
   turn, a positive number for that many milliseconds from now, and negative
   (`NO_WAKE`) for none. A turn of no quota is still a turn, and a question the
   host needs answered at once is asked in one, of a command the guest answers
   from what it already holds; one that waits on anything outside is answered
   on a later turn.
3. **The frame.** Four words: `tag`, `cmd` with its flags, `result`, and the
   packet. With `ACKREQ` a submission gets exactly one reply; without it, a
   reply only when it failed. A reply carries its submission's `tag` and `cmd`,
   and `RES`. Majors under `PROGRAM_MAJOR` are xpute's own.
4. **What the host sends** is applied in order, each submission once. The guest
   may leave some for a later turn, never out of order or skipped, and asks for
   the next turn while it does.
5. **What a turn leaves** — events, replies, hostcalls, stream records — is
   taken by the host before the next turn, none dropped. Order holds within one
   place a turn writes to, not between places, so a guest that needs one request
   run before another puts both in the same place. Running past the room set
   aside is a sizing mistake, and stops the guest.
6. **Handles.** A handle is `slot | generation << 16`, 0 is never one, and the
   issuing side checks the generation.
7. **The clock.** During a turn the guest reads the host's clock (`now`) in
   milliseconds. It never runs backward, and only differences mean anything.
8. **A call.** During a turn the guest may hand the host the offset of a frame
   it laid, unpublished, in the CQ's next slot. What answers the frame's major
   answers it before the call returns, reads that frame and writes only what
   it names, with a value or `-errno`, `ENOSYS` where nothing answers. It
   answers from what it already holds and waits on nothing outside, and the
   guest runs nothing until it has.

Everything else is one end's own. The guest's: where its ranges lie past the
front, its heap, its tick, passes, tasks and files. The host's: the size of a
quota, when to ring, how it reads what a turn left.

The contract describes one host and one guest. Several guests under one host,
passing buffers by ownership, do not change it. Where they share a thread or do
not trust each other, that composition must bring what the contract does not:
isolation, and a way to stop a guest that does not return.

Open: R1 to R3 are being carried through every end: atomics on every
position, the GPU stream's END reset by its consumer, and a buffer a frame
names released by the head, not by the next turn. Whether a native guest on
its own thread keeps `run(quota) → wake` as its contract is open.

---

## Written in several languages

The implementations do not all stand in the same relation, and which relation
decides what must match.

- **`kit` is symmetric.** Rust (`rust/kit`), TypeScript (`ts/kit`) and C++
  (`cpp/xpute/kit`) implement the same kit, held to each other by the golden
  records wherever bytes must match. TypeScript's has no `alloc`, since it has
  no linear heap of its own, and no `bug`, since a thrown error carries its
  stack.
- **The two ends face each other.** A host end and a guest end owe each other
  the contract and its formats; their module lists differ, and a module with
  nothing facing it is whole.
- **Guest ends are symmetric with each other.** Rust (`rust/guest`) and C++
  (`cpp/xpute/guest`) are the same end, either plugged into any host, so what
  one gives a program the other gives too.
- **Host ends hold what their platform needs.** TypeScript's (`ts/host`) boots
  a guest as its machine declares (`@xpute/host` `boot`), opening it by the
  build's platform, picked by file: `platform.ts`, the page's, reserves a
  module's memory and opens the module over it (`module.ts`), and
  `platform.native.ts`, the device's, reaches the guest linked into the app
  through `@xpute/native`. It then mounts on it in one call (`mount`), given
  all the program decides — what backs its files, what hears its own commands
  (`program`), its GPU, its policy, what it submits before each turn, what is
  told when the guest stops — and nothing of the order the host is put
  together in: it lays the front, binds xpute's majors to their servers, grants
  the GPU, sends `SYS_ATTACH` before anything else and runs the GPU's stream
  as each turn returns. What it gives speaks with the doorbell's verbs, `send`
  and `call`, each asking the strobe for a turn after, and `wake`, `resume`,
  `pause` and `visible`, which tells the guest at once (`SYS_SUSPEND`,
  `SYS_RESUME`). Rust's (`rust/host`) reads a guest's memory by offset on
  every platform (`mem`), and on a unix one hosts a guest linked into the
  host's own native process (`native`). There it reserves the memory, and runs
  every call into the guest on the guest's own thread, since the guest keeps
  its module-wide values in `Global`s (`xpute-guest/global.rs`), statics sound
  on one thread alone, handing each hostcall back to the caller that waits. It
  also provides the allocator that keeps the guest's blocks in its memory and
  every other thread's out of it. The React Native module (`ts/native`) holds
  the Kotlin, the Swift and the headers that bind the two, and Expo autolinks
  it.
- **The GPU's calls follow the same split.** Each guest language writes them
  (`rust/gpu`, `cpp/xpute/gpu`). The GPU hosts run them in TypeScript on WebGPU
  (`ts/gpu`), and in Rust on wgpu behind the `host` feature.
- **A guest built as a module is linked one way.** Its memory is imported from
  the host and its stack goes first, at the bottom of the memory, so an overflow
  traps; its data begins past the front (`global_base`), and it declares the
  smallest of its machine's sizes as its least. Rust's build script prints the
  flags (`xpute-guest/wasm/link.rs` `link`, `Machine::link`), and the C++ guest
  is linked with the same (`cpp/Makefile` `WASM_LINK`).

`kit` asks nothing of a program: link it and call it. `guest` and `host` ask it
to run xpute's way — memory granted once, work in turns, I/O by asking.

C++ is written from the contract and the formats, beside the Rust: built by the
platform's compiler (`cpp/Makefile`), and held by the same records and traces,
so the guests agree on what the boundary shows. Where C++ differs, it is the
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
  JSON at all, and what a number past the doubles reads as, as the host's own
  `JSON.parse` judges them), and `gpu/programs` (the words each encoder writes
  and the bytes each GPU host reads back). The bytes cross a boundary, so every
  language must produce these exactly.
- **Across languages** — `math/rng` (`rng.32` and `rng.64`), `math/scalar`,
  `collection/deque` and `collection/heap`. Nothing crosses a boundary, but one
  set of values holds every language's implementation: the same draws, the same
  rounding, the same order.

A GPU's floats are held only as far as WGSL holds them. Its `+`, `−` and `×`
round half to even, as every language here does. Its division may be 2.5 ulps
off, and its square root, `length` and the transcendentals have only bounds; a
GPU may be further off than the spec allows (a division has been measured 13
ulps off). WGSL also cannot forbid a product fused into the sum after it. So
which operations a rule takes depends on who decides it:

- **A rule the CPU and a GPU both decide** takes only `+`, `−`, `×`, a
  comparison, a bit operation, a power of two's scaling and the same
  constants, so the two agree within an ulp. A division by a constant is a
  product by its reciprocal, a constant both hold. Where a sum may be fused,
  the rule's test names the ulps it allows at the edge and counts what took
  them.
- **A rule a GPU alone decides** is one function every pass calls, so the GPU
  agrees with itself; its test names the ulps it allows.

The **traces** under `spec/conformance/` are a third kind, not outputs at all.
Each says what a host does and what it must then see: the completions, stream
records and next-turn asks each turn leaves. They are written from the
contract's rules and name only what a host can observe, so they hold every guest
to the same thing. The file device is a service the boundary carries, outside
the contract, so its traces stand apart: `spec/conformance/fs/` holds the
host's end, and `spec/conformance/file.tsv` every guest.

One host runs them against every guest through the boundary alone
(`@xpute/host/conformance.test.ts`): the Rust guest
(`xpute-conformance/guest.rs`) and the C++ one (`cpp/conformance/guest.cpp`),
each over the commands `spec/conformance/guest.json` numbers. Each guest's boot
takes a seed and lays the front, the header among it, in the order and with the
gaps the seed draws (`LAYOUTS`), and the host runs every trace under each: only
a host that finds the rings through the header, as rule 0 asks, passes. Each
guest is built both as a native library and as a WebAssembly module that
imports nothing but its memory, the clock and the hostcall; the C++ one from
the library's amalgamation (`cpp/amalgamation.awk`), the two files a C++
program takes, so what the traces hold is what ships.

## Prior art

What xpute took from work before it, or found it had arrived at. A link is given
where the tree records the source.

| Precedent                                                   | Where                            | What is taken                                                                  |
| ----------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------ |
| The exokernel and its library operating system              | the arrangement                  | The host grants real resources; the guest keeps its own abstractions over them |
| A kernel over its HAL                                       | the host                         | The host read as the core's hardware: calls down, interrupts up                |
| WebAssembly's host and guest, a WIT world                   | the two ABIs                     | The names; a world's imports and exports, seen from the guest                  |
| Aegis                                                       | `sched/policy`                   | The quota as a cooperative counterpart of its CPU slice                        |
| Linux's scheduling policies (`sched_setscheduler`)          | `sched/policy`                   | Its name: how turns are scheduled, chosen apart from what rings them           |
| Android's Frame Pacing library (Swappy)                     | `sched/frame_pacing`             | Its name, and turns a whole number of refreshes apart                          |
| A circuit's strobe, the pulse that says when to act         | `sched/strobe`                   | Its name                                                                       |
| "core" for main memory, as in a core dump                   | the core memory                  | Its name                                                                       |
| A linker script's output sections                           | `mem/section`                    | Ranges placed end to end without knowing what they hold                        |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `alloc/buddy_tree`               | The page allocator, ported line for line                                       |
| SLUB                                                        | `alloc/slab`                     | A slab keeps its free list inside its free slots                               |
| virtio's descriptor table, AF_XDP's UMEM                    | the rings (`spec/ipc/ring.json`) | Descriptors kept apart from the payload's slots                                |
| io_uring                                                    | the rings, the front             | SQ and CQ by its roles, its ordering rules, and the setup laying the rings     |
| ublk                                                        | the CQ                           | A server's own requests posted on the completion side                          |
| NVMe's queues and their doorbell registers                  | the doorbell                     | Its name, and that ringing it carries no message                               |
| NVMe's asynchronous events                                  | the frame's `RES`                | An event: a completion the guest posts unasked                                 |
| KVM's `KVM_RUN`                                             | the turn                         | `run`: the call that runs the guest until it gives the processor back          |
| Plan 9's file access, in POSIX's words                      | `ipc/fs`, `ipc/ns`               | Descriptors the guest issues; a root and a path; the host binds the roots      |
| Wasmtime's and Lucet's hostcalls                            | `hostcall`                       | Its name: a guest's call into its host                                         |
| POSIX and Linux errno                                       | `spec/status/errno.json`         | Negative errno numbers                                                         |
| The host's `JSON.parse`                                     | `codec/json`                     | The grammar, judged as the host's parser judges it                             |
| IEEE 754's round half to even, as WGSL rounds               | `math/scalar`                    | One rounding in every language                                                 |
| splitmix64                                                  | `math/rng`                       | A reproducible generator, its seed's finalizer, and its high half as a u32     |
| Murmur3's 32-bit finalizer                                  | `math/rng`                       | The 32-bit family's mix, for a GPU with no 64-bit integer                      |
| FNV                                                         | `codec`                          | The hash                                                                       |
