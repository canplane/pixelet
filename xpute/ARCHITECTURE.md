# Architecture — xpute

[xpute](./README.md) runs one program inside another, gives it resources, and
leaves the abstractions over them to it. The outer program, the **host**, owns
the platform: the clock, the memory, whatever reaches outside. The inner one,
the **guest**, owns none of it. The host grants a fixed memory once, and runs
the guest in **turns**, each one call into it with a quota of time; it hands on
what the guest asks of the outside: a file's open, and the commit of one
written, to the program; a GPU call to a device. Over those grants the guest
builds what it needs — an allocator, a scheduler, tables of what it holds — and
the policy each runs by.

The host/guest split follows the exokernel/library-OS division: the host grants
real resources, a real region and a real quota, and the guest builds its own
abstractions over them; nothing is virtualized. xpute does not enforce those
grants.

xpute comes in two layers. `kit` is generic code — collections, codecs, a wire
format, an errno table, scalar math — that any program can link and call.
`guest` and `host` are the arrangement itself, one end each, written against
`kit`, which knows nothing of them. Neither layer knows what a guest computes.

Three things cross between the ends and nothing else does: one memory they
share, one call in, one number out. Two calls go the other way, each answered
before it returns: the host's clock, and a hostcall run now, a question the host
answers from what it already holds (both below). The sections follow from that:

1. the memory;
2. what is written into it — one shape for every message, its data, and the
   queues that carry it each way;
3. the call that makes it run, the turn;
4. handles, which name objects across the boundary;
5. hostcalls and files, and a GPU's calls, which a guest asks of its host;
6. the contract all of it comes down to;
7. the languages, and the records that hold them to each other.

---

## The core memory, placed once

The memory is the one thing the two ends share. It is the **core memory**, core
as in a core dump. The host reserves it at boot and it never grows, so a view
into it stays valid for as long as the guest lives: a queue keeps its words at a
fixed address, and a payload is read where it lies. A page reserves it as a
`WebAssembly.Memory` whose initial size is its maximum
(`@xpute/host/mem/reserve.ts` `reserve_mem`); a native program that links its
guest in maps it from the system, and the guest's own thread then sets its base
(`xpute-host/native.rs` `boot`, `xpute-guest/mem.rs` `set`).

The memory need not begin at address zero. A WebAssembly module's does; a guest
linked into a native program has its memory wherever the system put it. So every
address a guest names is an offset from the memory's base, never a pointer
(`mem`, `Memory`). It means the same to the host however the guest is built, and
it crosses as a `u32` however wide the machine's pointers are. A part laid over
a region takes the `Memory` it lies in and its offset, so a test lays it over a
buffer of its own; a program reading its own data goes through the guest's own
memory, set once (`core`, `at`).

What goes in the memory is the guest's, and so is where. It lays ranges end to
end on a unit it chooses (`mem/section`), as a linker script places output
sections without knowing what they hold, and whatever is asked of a range is
checked against its end (`Section::at`, `nth`). The host is told where the parts
it reads lie by a **toc**, a table of contents as a CD's tells where each track
begins (`ipc/toc`, `spec/ipc/toc.json`), which the guest writes in its first
turn, or in a boot call that returns its offset; the host learns that one offset
from outside and finds everything else from there (`@xpute/host/ipc/toc.ts`
`Toc`). xpute numbers a key for each part of its own — the two queues and the
hostcall stream the sections below describe — and a program numbers its own keys
with the `PROGRAM` bit set.

```text
one guest's layout; the order of its ranges is the guest's, the heap last

 0 ┌─────────────────────────────────┐
   │ a module's stack and data       │
   ├─────────────────────────────────┤ ◀── the one offset the host learns
   │ toc                             │
   │   MAGIC · VERSION · count · 0   │
   │   SQ  · offset ─────────────────┼──┐
   │   CQ  · offset ─────────────────┼──┼──┐
   │   SYS · offset ─────────────────┼──┼──┼──┐
   │   PROGRAM | n · value           │  │  │  │
   ├─────────────────────────────────┤  │  │  │
   │ submission ring · its slots     │◀─┘  │  │
   ├─────────────────────────────────┤     │  │
   │ completion ring · its slots     │◀────┘  │
   ├─────────────────────────────────┤        │
   │ hostcall stream                 │◀───────┘
   ├─────────────────────────────────┤
   │ the guest's own ranges          │
   ├─────────────────────────────────┤
   │ heap, to the memory's end       │
   └─────────────────────────────────┘
```

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

- `tag`, the port at the guest's far side, and an id;
- `cmd`, what is asked, in the low 16 bits, with flags in the high 16;
- `result`, what came back, a value or `-errno`;
- where its **packet** lies — the data the frame carries — or 0 for none.

An errno is one table's (`spec/status/errno.json`), and every language reads a
code back into its member, or into none (`Errno::of` in Rust, `errno_of` in the
others). A command is `major << 8 | minor` (`kit` `abi/cmd`).

Every frame names a **port**, in its tag's high bits: who sent a frame the guest
takes, who is to have one the guest posts. A reply keeps its submission's tag,
so it goes back to its sender. The host is port 0, and what it sends is xpute's
own (`spec/abi/cmd.json`); every other number is the program's, which gives each
its meaning and numbers its commands as it likes. The host hands a frame to the
port it names where one is attached and drops it where none is, as a datagram
goes to the UDP port it names; the ports are the program's ABI, as UDP's
well-known ones are a convention.

Two flags decide what a frame means. `ACKREQ`, set by whoever submits, asks for
a reply. `RES`, set by whoever completes, says the frame is a reply. A
completion without `RES` is an **event**, one the guest raised on its own, for
the port it names, as an NVMe controller posts an asynchronous event to its
completion queue.

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

Two rings make a pair: the submission ring for what the host sends, and the
completion ring for what the guest answers and raises, each where the toc says
(`SQ`, `CQ`). The host drives the pair through the **doorbell**
(`@xpute/host/ipc/doorbell.ts` `Doorbell`):

- it writes each submission in place, into the submission ring's next slot,
  tagged with the port that sends it (`writer`, `send`);
- it rings the guest with a quota (`ring`), which is one whole turn;
- it then reads the completion ring dry, handing each reply to the call waiting
  for it and each event to the port it names, where one is attached
  (`listen`).

Two ways in sit on top of that. A question the host needs answered at once is a
`call`: submitted with `ACKREQ` and rung with no quota until its reply comes
back. A command from outside the host's own code, such as a developer's console,
is `cmd`: its arguments become the packet's fields (`TreeView.set`), and it is
sent and rung. A guest that traps is gone, and the doorbell rings it no more.

The guest's end is the **door** (`ipc/door` `Door`). It applies the submissions
in order, answers each, and posts what the guest raises into the completion
ring, for the port it names (`raise`), which holds a whole turn's output.
What the host sent it applies itself, and hands the program every other sender's
and each open the host completed (`Submission`). A drain runs before any phase's budget, so the program records what it is handed
and returns. Before each submission the door checks that the ring has room for
the most one command may leave — its events, a bound the program gives
(`most_events`), and its reply — and where it has not, it applies no more and
leaves the rest for the next turn, in order.

---

## The turn

The turn is when the guest runs at all.

It runs inside one call, `run(quota_ms)`, named as KVM's `KVM_RUN` is, the call
that runs a guest until it gives the processor back; when that call returns the
guest is not running. The call has two edges, like a clock's. On the **rising
edge** the host calls in with a quota, and the guest drains the submission ring,
does its work and posts what it has. On the **falling edge** it returns
`wake_ms`, when it would like the next turn: `0` for the next the host can run,
`n` for that many milliseconds, `NO_WAKE` for none. The host learns nothing else
about what ran.

xpute names that call, as a C runtime names the `main` it calls: the program
implements it and never renames it (`xpute-guest/entry.rs` `run`). A guest
built as a module exports `run`; a guest linked into a native program exports
`xpute_run`, and its memory as `xpute_mem`, under xpute's name and no
program's, since a process holds one guest, with one reservation
(`xpute-host/native.rs`). What the guest's form hands it there is xpute's as
well (`BootInfo`): the memory it was given, where its own ranges may begin, the
host's clock, its hostcall and its heap's reading.

On the host side, something has to decide when to ring at all. That is the
**strobe** (`@xpute/host/sched/strobe.ts` `Strobe`), named for a circuit's
strobe, the pulse that tells a receiver when to act. It rings only when the host
or the guest's last falling edge asked for a turn, at the moment the scheduling
policy below gives; a positive `wake_ms` sets a timer that asks for a turn when
it fires. A guest that asks for nothing is rung for nothing. The doorbell rings
too, with no quota: to make room in a full submission ring, to wait for a call's
reply and to send a `cmd`; and whatever holds the doorbell may ring it.

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

commands ─────── submission ring ───────▶  drained on the rising edge
doorbell ═══════ run(quota_ms) ═════════▶  ┌ tick   phases in a fixed order
                                           │ tasks  that yield when
                                           │        the quota is spent
                                           │ asks   what it wants done outside
completions ◀───── completion ring ──────  └ posted before the falling edge
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

A guest cannot do I/O, because the host owns the connections, the way an
operating system owns its devices. So the guest asks, in what its turn leaves,
and the host hands the ask on and brings back what came of it — in a completion,
or into memory the request named.

How much may be out at once, and which goes first, are the program's; it passes
a bound in its own commands.

### Streams

Some requests come in thousands a turn — every call a renderer makes to a device
it cannot reach — and need neither a reply nor a packet. For those the guest
writes a **stream** (`ipc/stream`): records laid end to end during a turn, which
the host runs, all of them, after the turn returns and before it rings again.

```text
0       END   how many words of records the turn wrote
1…            the rest of the head, the program's
head…   op · n · n words   op · n · n words   …
```

A record is an op the program numbers, a count, and that many words. Since the
whole stream runs between two turns, a record can name memory for the host to
write into, and the guest reads it next turn: the order is the acknowledgment.

A record past the range is a sizing bug, and stops the guest. Inside a
transaction (`begin`, `commit`), a record that does not fit drops the
transaction whole, and `commit` says so.

### Hostcalls and files

A **hostcall** is a guest's call into its host, the name Wasmtime and Lucet give
one. Some are every program's: open, read, write and close a file, and report
the invariant that stopped the guest; a program numbers its own with the
`PROGRAM` bit. Each is a record (`ipc/sys`, `spec/ipc/sys.json`), run after the
turn from one stream the toc names (`SYS`), or, a program's own, at once
(below). The report (`ABORT`) drops the turn's other calls and is published
alone, at once.

Files borrow Plan 9's file namespace:

- the guest issues the descriptor, a handle of its own file table (`ipc/file`);
- it names a file by a root the program numbers and a path below it;
- a path walks down a name at a time, and an empty name, `.` or `..` is refused,
  so a root is the whole of what it grants.

An open is the one call that waits: the host answers it with `SYS_OPENED`, as
port 0 and on a later turn, once the file can be read whole, and the door
hands it to the program, which finds the open's owner
in its file table (`ipc/file` `Files::opened`), a failed open closed there. A
read, a write and a close are done before the next turn, and a file opened for
writing is replaced at its close by what was written.

The host's end runs some calls itself and hands the rest to the program
(`@xpute/host/ipc/sys.ts` `Sys`, `SysHost`):

```text
the guest's turn          the host, after it returns   the program's SysHost
────────────────          ──────────────────────────   ─────────────────────
READ · WRITE · CLOSE ──▶  Sys, in the guest's memory
OPENAT ────────────────▶  Sys ────── open ──────────▶  fs ─▶ Namespace,
◀── SYS_OPENED, later ──  Sys ◀─── bytes, or errno ──  fs    one binding a root
CLOSE of a write ──────▶  Sys ────── commit ────────▶  fs
ABORT ─────────────────▶  Sys ────── report ────────▶  abort
PROGRAM | n ───────────▶  Sys ────── call ──────────▶  program
```

The host's namespace (`@xpute/host/ipc/ns.ts` `Namespace`) is one such `fs`, its
roots bound when it is made: each call goes to what is bound below the root it
names, `ENOENT` below a root nothing is bound to, `EROFS` for a write below one
that cannot be written. What a file is, where it lies and how it is fetched are
the binding's; xpute only carries the call.

### A hostcall run now

A hostcall from the stream is answered after the turn returns, so a guest that
needs its answer to go on waits a turn for it, and a chain of such calls a turn
each. Some need nothing from outside: the host holds the answer already, as a
page's text engine holds a line's width. For those a program's hostcall is run
now: the guest lays its record past the last its turn has recorded, where
nothing publishes it, and hands the host its offset (`ipc/sys`
`Sys::call_now`). The host runs it before the call returns, reading that
record and writing only what it names, and answers with a value or `-errno`. It
hands the call to the program as it hands one from the stream
(`@xpute/host/ipc/sys.ts` `Sys.call_now`), so the program numbers its
calls once and may have them run either way.

A hostcall run now never waits on anything outside: an open, a buffer read back
and every hostcall of xpute's own go in the stream. It crosses the boundary once
a call where the rest cross once a turn, so it is for the few calls a turn
cannot answer late.

A module imports the entry as `env.hostcall`, beside `env.now`; a guest linked
into a native program is handed it with its clock (`BootInfo`). There the guest
runs on its own thread while the caller of `run` waits, so the native host hands
each hostcall run now back to that caller, which answers it on its own thread
and waits again (`xpute-host/native/thread.rs`); the program says what answers
through `xpute_hostcall_on` (`answer_with`), under xpute's name as the entry is.

### A GPU's calls, in WebGPU's words

A renderer's calls are what streams were made for, and most of what they say is
WebGPU's, not one program's. So xpute writes them down once, in WebGPU's
vocabulary (`spec/gpu/`). An enumeration is its spec's strings in order, so a
host hands the string to the device as it is. A call is an op whose words follow
the descriptor WebGPU takes, with nothing defaulted on the program's behalf.

A guest writes them through the encoder (`gpu/encode`), which takes each
object's handle from one table and writes each record twice, once to count its
words and once into the room reserved for them. The **GPU host** runs the stream
on its device after the turn returns (`gpu/host` `GpuHost`): on WebGPU in a
page, on wgpu in a native program. Three calls finish later — an image copied
in, a buffer read back, the queue's work done — so each names a word of the
guest's memory and a ticket, a number the guest wrote there; once the call is
done, the GPU host writes the ticket back with `DONE` set.

What a surface shows — a canvas, a view's layer, a window — is the program's,
bound to the GPU host under the guest's handle for it and let go by that handle
(`bind`, `unbind`); the guest names the surface by the handle's slot in its
records. Here the GPU host is the side that keeps the object, so it checks the
generation.

The native GPU host reaches the guest's memory by offset (`xpute-host/mem.rs`
`Memory`), each read or write only the range the guest names, since the host's
own blocks may lie in the same memory. It also paces drawn frames, which a
browser does for a page: a turn draws while fewer of the drawn frames it
admitted are unfinished than its frame latency (`GpuHost::admit`), so taking a
surface's texture never blocks. The answer reaches the guest as a value the
program hands its renderer (`xpute-gpu/admit.rs` `Admit`); a page's GPU host
admits every drawn frame (`every`). The GPU host's numbers and choices are the
program's, said in its config (`Config`) and none defaulted, among them where a
surface is configured (`ConfigureAt`). It opens the device too, as the program
asks (`open`, `Ask`); on the web the browser's `requestAdapter` and
`requestDevice` are that call already.

Beside a guest linked into its own process, the GPU host is exported as the
entry is, under xpute's names (`xpute-gpu/host/native.rs` `native_abi!`):
`xpute_gpu_open` opens the device and holds it, `xpute_gpu_surface_bind` makes a
surface of a window the platform hands over and binds it under the guest's
handle, `xpute_gpu_surface_unbind` lets it go, and `xpute_gpu_wake_on` takes the
function a completion calls to ask a turn. The GPU host is touched on the
guest's thread alone (`Guest::run`), so a process holds one, as it holds one
guest.

The GPU's calls stand beside `kit` and the two ends: a program that never
reaches a GPU links none of it. What a program builds over them, such as a
shorthand for its pipelines, stays the program's.

---

## The contract

The contract is the part neither end may change; the rest of this document is
how one end happens to be built.

What cannot change is the rules below and the formats they travel in: the toc,
the frame's four words, the ring's layout and the stream's records. Each rule is
one the other end could catch being broken.

0. **The toc.** The guest writes it in its first turn or in a boot call that
   returns its offset, and the host finds everything from the one offset it
   learns outside. A key appears at most once, an unknown key is passed over,
   and an unknown magic or version is refused.
1. **The core memory.** It never moves or grows while the guest lives, and every
   address that crosses is an offset into it. Offset 0 names nothing, and
   nothing that crosses lies there.
2. **The turn.** The guest runs only inside `run`, and the two sides never run
   at once: the host touches nothing a turn writes until it has returned, but
   what a hostcall run now names while it runs (rule 8). The quota is advice,
   not a deadline: a guest can spend a whole quota inside one function, and the
   host cannot take the turn back until it returns. `wake_ms` is 0 for the next
   turn, a positive number for that many milliseconds from now, and negative
   (`NO_WAKE`) for none. A turn of no quota is still a turn, and a question the
   host needs answered at once is asked in one, of a command the guest answers
   from what it already holds; one that waits on anything outside is answered on
   a later turn.
3. **The frame.** Four words: `tag`, `cmd` with its flags, `result`, and the
   packet. With `ACKREQ` a submission gets exactly one reply; without it, a
   reply only when it failed. A reply carries its submission's `tag` and `cmd`,
   and `RES`. A tag names a port, and what port 0, the host, sends is
   xpute's own.
4. **What the host sends** is applied in order, each submission once. The guest may leave some for a later turn, never out of order or
   skipped, and asks for the next turn while it does.
5. **What a turn leaves** — events, replies, stream records — is taken by the
   host before the next turn, none dropped. Order holds within one place a turn
   writes to, not between places, so a guest that needs one request run before
   another puts both in the same place. Running past the room set aside is a
   sizing mistake, and stops the guest.
6. **Handles.** A handle is `slot | generation << 16`, 0 is never one, and the
   issuing side checks the generation.
7. **The clock.** During a turn the guest reads the host's clock (`now`) in
   milliseconds. It never runs backward, and only differences mean anything.
8. **A hostcall run now.** During a turn the guest may hand the host the offset
   of a hostcall's record. The host runs it before the call returns, reads that
   record and writes only what it names, and answers with a value or `-errno`,
   `ENOSYS` for a call that is not the program's. It answers from what it
   already holds and waits on nothing outside, and the guest runs nothing until
   it has, so the two sides still never run at once.

Everything else is one end's own. The guest's: where its ranges lie, its heap,
its tick, passes, tasks and files. The host's: the size of a quota, when to
ring, how it reads what a turn left.

The contract describes one host and one guest. Several guests under one host,
passing buffers by ownership, do not change it. Where they share a thread or do
not trust each other, that composition must bring what the contract does not:
isolation, and a way to stop a guest that does not return.

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
- **Host ends hold what their platform needs.** TypeScript's (`ts/host`)
  reserves a module's memory and opens the module over it (`module.ts`), and
  mounts the doorbell, the hostcalls' end and the strobe on any guest its
  platform opened, in one call (`@xpute/host/host.ts` `mount`), which runs the
  hostcall stream as each turn returns. `mount` reads the toc as it is called,
  so the turn or the boot call that writes it has run by then. Rust's
  (`rust/host`) reads a guest's memory by offset on every platform (`mem`), and
  on a unix one hosts a guest linked into the host's own native process
  (`native`). There it reserves the memory, and runs every call into the guest
  on the guest's own thread, since the guest keeps its module-wide values in
  `Global`s (`xpute-guest/global.rs`), statics sound on one thread alone,
  handing each hostcall back to the caller that waits. It also provides the
  allocator that keeps the guest's blocks in its memory and every other thread's
  out of it.
- **The GPU's calls follow the same split.** Each guest language writes them
  (`rust/gpu`, `cpp/xpute/gpu`). The GPU hosts run them in TypeScript on WebGPU
  (`ts/gpu`), and in Rust on wgpu behind the `host` feature.
- **A guest built as a module is linked one way.** Its memory is imported from
  the host and its stack goes first, at the bottom of the memory, so an overflow
  traps; its stack's size and any front it keeps below the linker's data are its
  own numbers. Rust's build script prints the flags (`xpute-guest/wasm/link.rs`
  `link`), and the C++ guest is linked with the same (`cpp/Makefile`
  `WASM_LINK`).

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

Its numbers — the errno table, the toc's keys, a frame's and a ring header's
words — come from `spec/` as the other languages' do.

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

The **traces** under `spec/conformance/` are a third kind, not outputs at all.
Each says what a host does and what it must then see: the completions, stream
records and next-turn asks each turn leaves. They are written from the
contract's rules and name only what a host can observe, so they hold every guest
to the same thing. Files are a service the boundary carries, outside the
contract, so their traces stand apart: `spec/conformance/sys/` holds the host's
end, and `spec/conformance/file.tsv` every guest.

One host runs them against every guest through the boundary alone
(`@xpute/host/conformance.test.ts`): the Rust guest
(`xpute-conformance/guest.rs`) and the C++ one (`cpp/conformance/guest.cpp`),
each over the commands `spec/conformance/guest.json` numbers. Each guest's boot
takes a seed and lays what it shares, the toc among them, in the order and with
the gaps the seed draws (`LAYOUTS`), and the host runs every trace under each:
only a host that finds them all through the toc, as rule 0 asks, passes. Each
guest is built both as a native library and as a WebAssembly module that imports
nothing but its memory, the clock and the hostcall; the C++ one from the
library's amalgamation (`cpp/amalgamation.awk`), the two files a C++ program
takes, so what the traces hold is what ships.

## Prior art

What xpute took from work before it, or found it had arrived at. A link is given
where the tree records the source.

| Precedent                                                   | Where                            | What is taken                                                                  |
| ----------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------ |
| The exokernel and its library operating system              | the arrangement                  | The host grants real resources; the guest keeps its own abstractions over them |
| Aegis                                                       | `sched/policy`                   | The quota as a cooperative counterpart of its CPU slice                        |
| Linux's scheduling policies (`sched_setscheduler`)          | `sched/policy`                   | Its name: how turns are scheduled, chosen apart from what rings them           |
| Android's Frame Pacing library (Swappy)                     | `sched/frame_pacing`             | Its name, and turns a whole number of refreshes apart                          |
| A circuit's strobe, the pulse that says when to act         | `sched/strobe`                   | Its name                                                                       |
| "core" for main memory, as in a core dump                   | the core memory                  | Its name                                                                       |
| A linker script's output sections                           | `mem/section`                    | Ranges placed end to end without knowing what they hold                        |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `alloc/buddy_tree`               | The page allocator, ported line for line                                       |
| SLUB                                                        | `alloc/slab`                     | A slab keeps its free list inside its free slots                               |
| virtio's descriptor table, AF_XDP's UMEM                    | the rings (`spec/ipc/ring.json`) | Descriptors kept apart from the payload's slots                                |
| io_uring                                                    | the rings                        | A submission ring and a completion ring                                        |
| NVMe's queues and their doorbell registers                  | the doorbell                     | Its name, and that ringing it carries no message                               |
| UDP's ports                                                 | the tag's port                   | A number a listener binds by convention; to none bound, a frame is dropped     |
| NVMe's asynchronous events                                  | the frame's `RES`                | An event: a completion the guest posts unasked                                 |
| KVM's `KVM_RUN`                                             | the turn                         | `run`: the call that runs the guest until it gives the processor back          |
| Plan 9's file access, in POSIX's words                      | `ipc/sys`, `ipc/ns`              | Descriptors the guest issues; a root and a path; the host binds the roots      |
| A CD's table of contents                                    | `ipc/toc`                        | Its name: where each part begins, read before anything else                    |
| The ELF auxiliary vector                                    | `ipc/toc`                        | Entries a reader skips when it does not know them                              |
| Wasmtime's and Lucet's hostcalls                            | `ipc/sys`                        | Its name: a guest's call into its host                                         |
| POSIX and Linux errno                                       | `spec/status/errno.json`         | Negative errno numbers                                                         |
| The host's `JSON.parse`                                     | `codec/json`                     | The grammar, judged as the host's parser judges it                             |
| IEEE 754's round half to even, as WGSL rounds               | `math/scalar`                    | One rounding in every language                                                 |
| splitmix64                                                  | `math/rng`                       | A reproducible generator, its seed's finalizer, and its high half as a u32     |
| Murmur3's 32-bit finalizer                                  | `math/rng`                       | The 32-bit family's mix, for a GPU with no 64-bit integer                      |
| FNV                                                         | `codec`                          | The hash                                                                       |
