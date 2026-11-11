# Architecture — xpute

[xpute](./README.md) is a foreign function interface between a program's
compiled **core** and the **runtime** that runs it. It is a calling convention
over three host functions, through which the two pass frames laid in the core's
memory; bindings for both ends, which a generator writes from the program's
protocols as WIT's tools write a component's; the turn, the one call through
which the runtime gives the core its time; and two ways to move large data while
that memory stays fixed: a read into a buffer the caller names, as WASI's
`fd_read` reads, and the GPU stream, a command buffer the core records and the
runtime runs. The core owns nothing but its memory. The runtime implements only
the devices that lend that memory across turns, the GPU and the files, and
nothing else. Everything else belongs to the **program**, and it travels over
one general channel whose messages xpute delivers and does not interpret.

The core is the program's compiled code, built with xpute's core library into
one image, which runs as a WebAssembly module in a web page and as a library
linked into a native app. The runtime is xpute's TypeScript, with Rust beside it
on a device. The program is what the app decides: what its commands mean, what
backs its files, when its turns should run, and its UI.

Each section below takes one part of xpute, in the order the parts depend on
each other: the memory the core owns, the turn that gives it time, the channel
that carries everything else, the two devices that lend the memory, and the
languages, with the records that hold them to each other. The document ends with
the boundaries, the invariants every end keeps, what xpute took from earlier
work, and a map of the code. The contract's layouts, those of the frame, the
stream, the header and each device's commands, are written once in
`spec/`, and this document points there instead of restating them.

---

## The core memory, placed once

The core owns one thing, its memory, which is also the one thing the core and
the runtime share. It is the **core memory**. The runtime reserves it before the
first turn, at one of the sizes the core allows, and it never grows or moves. A
view into it therefore stays valid for as long as the core lives: a frame stays
where it was laid, and a packet is read where it lies.

Every address the core names is an offset from the memory's first byte, never a
pointer. A module's memory begins at address zero, and a native core's memory
begins wherever the system mapped it, so only an offset means the same to the
runtime in both. It crosses the boundary as a 32-bit word, however wide the
platform's pointers are, and offset 0 names nothing.

How big the memory is, and what lies at its front, both ends must agree on
before either runs. The core declares this once, in its **manifest**, its own
declaration of what it needs, as an Android app's manifest is: a JSON file from
which the generator writes the same constants into both ends. From the
manifest, each end computes the **front**: the stack, a page for the header, and
the GPU stream where the manifest asks for a GPU. The runtime writes the header
before the first turn.
On its first turn the core computes the same front from its own manifest and
holds the header to it. Where a word differs, the core stops with
ENOEXEC and reports the word's place and what it held. The header records what
the runtime laid; it does not tell the core where to look.

```text
one core memory: the front is the manifest's, the rest the core's

   ┌─────────────────────────────────┐
   │ a module's stack                │  first, so that an overflow traps
   ├─────────────────────────────────┤
   │ the header                      │  what the runtime laid
   ├─────────────────────────────────┤
   │ the GPU stream, where asked for │
   ├─────────────────────────────────┤  the end of the front
   │ a module's data                 │
   │ the core's fixed ranges         │
   │ the heap, to the memory's end   │
   └─────────────────────────────────┘
```

Past the front, what the memory holds is the core's to decide, and so is where
it goes. The core lays its ranges end to end on pages, as a linker script places
output sections without knowing what they hold, and every access through a range
is checked against the range's end. As the core compiles, its manifest checks
that the smallest memory it allows holds the front, the fixed ranges and the
heap it asks for, so a manifest that cannot run does not build. The heap is
reached through one call, and what answers that call is the core's choice; `kit`
offers a buddy allocator of pages and a slab of size classes over it.

A core that outlives its runtime, as a native core outlives a reload of the
app's JavaScript, keeps the front it has, and the next runtime finds the header
already laid. The header's words are in `spec/io/head.json`, and the manifest's
fields in `spec/manifest.schema.json`.

## The turn

The core runs only inside a turn, and a turn is one call, `run(deadline_ms)`,
which runs the core until it gives the processor back. The runtime calls in with
a deadline, and the core receives what waits for it, does its work and sends
what it has. Then the core returns `wake_ms`, which says when it wants the next
turn: 0 for as soon as the runtime may run one, a positive number for that many
milliseconds from now, and a negative number for none until something arrives.
The runtime learns nothing else about what ran. xpute names this entry, `run`,
and the program implements it under that name.

Nothing preempts a turn. The deadline is the core's to keep: the core can run
past it inside one function, and the runtime cannot take the turn back until the
call returns. The turn is a cooperative loop, and the core's library is built
for it. A **pass** spends the time left to the deadline, less what the
program holds back for the work after it, on ordered lists of steps. Every item
after the first is held to a hard limit a little past the budget, so a list
whose steps report no work still cannot run on through the whole of it. Work
too long for one turn is a **task**, which checks the deadline at its own yield
points, yields once it has passed and parks until a later turn. What runs the
tasks and in what order is the program's own executor; xpute keeps only the
tasks parked until the next turn.

On the runtime's side, something has to decide when to run a turn at all. That
is the **scheduler**. It runs a turn only when the runtime or the core's last
turn asked for one, and only at a moment that the **scheduling policy**
gives and admits, so a core that asks for nothing is given no turn. The policy
is the program's, injected with the program's numbers when the runtime boots.
xpute ships two: frame pacing for a display, which runs turns on its frames, a
whole number of refreshes apart, each ending by the deadline of its frame; and a
timeslice for a runtime with no display, which runs a turn as soon as it is
asked for and ends it a fixed slice later.

```text
the runtime                                 the core
───────────                                 ────────
the scheduler runs a turn only when one
was asked for, at the policy's moment

commands ────────── receive ──────────▶     taken as the turn starts
scheduler ════════ run(deadline) ═════▶     ┌ passes  steps, in order
                                            │ tasks   yield once the
                                            │         deadline has passed
replies, asks ◀───── send ────────────      └ sent before the call returns
         ◀═════════ wake_ms ═══════════     next · in n ms · none

the GPU stream run, what the turn sent      nothing at all
read, then nothing until a turn is asked
for
```

Where the core runs differs by platform, and the turn does not. On a page, the
core and the runtime share one thread, a worker's, and take turns on it: the
scheduler starts a turn, the core runs, the call returns, and the runtime runs
the GPU stream and reads what the turn sent before it runs another. The UI runs
on the page's main thread and reaches the worker through a message port, so a
turn never holds up the UI's events.

On a device, a process holds one core, and the core runs on a thread of its own,
because its global state is sound on one thread only. That thread sleeps until
a display frame finds a turn wanted, and the platform's display link tells it of
each frame, so how late the JavaScript thread is does not move the turn. It paces
the turns itself, by frame pacing with the program's numbers, the one policy a
device's runtime runs. The runtime has a thread of its own, in Rust: it queues
what the core is sent and asks for a turn, and it serves what the core sent. The
core's thread receives and sends by copying a frame into or out of that queue,
and does nothing else under its lock. The UI's JavaScript thread holds no
runtime; it reaches the runtime as a page's UI reaches its worker, posting
frames and taking those it hears in a batch, and only what backs the files
stays beside it. Three rules hold that loop together: no turn asked for is
lost, every wait has someone to answer it, and every turn takes what waits for
it first.

The runtime also gives the core its clock. During a turn the core reads the
runtime's time in milliseconds, which never runs backward, and only the
difference between two readings means anything.

## One channel for everything else

Outside its memory, the core sees one thing: a channel. Everything the program
says to its core, and everything the core says back, travels over it as
**frames**, and xpute reads only enough of each frame to deliver it.

A frame is four words: a tag, what it asks, what came back, and how long its
packet is. The tag is the sender's id for the frame, which a reply keeps, and
on an import it says what the import is about. The command shares its word with two flags. ACKREQ, set by whoever
submits, asks for a reply in every case, and RES, set by whoever completes,
marks a reply. A frame that the core sends without RES is one of its
**imports**, raised on its own, not in answer to anything. The packet
is the frame's data, an XTP tree written right past the frame's words. The
frame is laid out in `spec/io/frame.json`, and the packet in
[XTP's README](./ts/kit/src/wire/xtp/README.md).

The core reaches its runtime through three functions, its **host**, which the
runtime hands it at boot. The core keeps two frames of its own on its heap, one
to receive into and one to send from. `receive` writes the next frame that
waits for the core into the first; `send` takes a frame the core laid in the
second, with the memory that frame names; and `call` takes one and answers it
before it returns. A page's runtime gives them as the
module's imports, and a native one as functions of its process. Each names a
frame by its offset in the core memory, so a frame crosses in one call, and
neither side keeps a position or a capacity of the other's: what the core is
sent waits in the runtime's queue until a turn takes it, and what the core sends
is the runtime's once `send` returns.

A packet must fit a frame's room. One larger than the manifest's packet room is
refused with EMSGSIZE where it is sent, so the manifest bounds the commands a
program can send.

```text
        the runtime                                 the core
  ┌──────────────────────┐                    ┌─────────────────────┐
  │ the connection       │                    │ the door            │
  │   queues what the ───┼───── receive ─────▶│   applies what it   │
  │   core is sent       │ commands, and      │   is sent, in order │
  │                      │ completions        │                     │
  │   reads what the ◀───┼────── send ────────┼── sends replies and │
  │   turn sent, and     │ replies, imports,  │   what it raises    │
  │   routes each frame  │ the stop report    │                     │
  │   by the major of    │                    │                     │
  │   its command        │                    │                     │
  └────┬────────────┬────┘                    └─────────────────────┘
       │            │
  0x00 to 0x0F   0x10 and up
       ▼            ▼
  xpute's own    the program's handler, and the UI for
                 the commands the UI takes
```

The runtime's end of the channel is the **connection**. It queues each
submission for the core, asks the scheduler for a turn, and then reads what the
turn sent, handing each reply to
whoever waits on its tag and every other frame to one listener, so a listener
never runs inside a turn. The core's end is the **door**. It receives every
submission that waits, applies them in order, replies where it must, and sends
what the core raises.

A command's number is a major and a minor, and whoever defines the command
decides its major. Majors 0x00 to 0x0F belong to xpute, and a program numbers
its own commands from 0x10. The runtime routes each frame by its major: xpute's
go to xpute's own handlers, and every other frame goes to the program's one
handler. The UI hears every command it lists as well, xpute's among them, such
as the stop report. On the core's
side, the door applies xpute's commands itself or hands them to the program as
what they mean, and it hands the program every other command with its number and
its packet, unread. Nothing in a frame names a port or a destination: xpute
frames the program's messages and delivers them, and only the program knows
what they say.

xpute's own traffic on the channel is small. The runtime tells the core that a runtime attached, the first
time or again after a reload; that the program was hidden and the core is about
to be frozen, or that it was shown again; and that a turn is one the policy
granted, with the wall clock. As the core stops, it sends the report of the
invariant it broke. What each of these means the core should keep or reset is
the core's to decide. The commands are in `spec/abi/sys.json`.

There are three ways to ask over the channel. A **send** goes with the next turn
and is answered only if it fails. A **request** asks for a reply, which comes
with a later turn while nothing waits for it, and which may hand over memory of
the core's, named as any frame names it. A **call** is answered before it
returns. The runtime calls the core by queueing the frame and running turns with
a deadline of zero until the reply comes back, so its call is a turn of its own,
which a UI across a port cannot wait for. The core calls the runtime by laying
the frame in the frame it sends from and handing it to `call`, and whatever
answers the frame's major answers at once, from what it already holds, as a
page's text engine holds the width of a line. A call never nests, and its
answer makes no call. The core waits for its answer, so a call is kept for the
few questions a turn cannot answer late.

A **protocol** is a JSON file of commands, each an export, which the core
handles, or an import, which the core sends, as a WIT world names them from the
core's side. The **generator** writes both ends' code from the manifest and the
protocols, each end holding only its own direction, as WIT's guest and host
bindings do. The core's end sends each import and reads each export into the
program's trait (`apply`), and it reads a request's completion. The runtime's
end writes each export and reads each import into the runtime's handler
(`serve`, or the interface a TypeScript runtime routes to), and it reads a
record the core lays out by a view that reads each field when asked. xpute's
own protocols are generated the same way, so every language's numbers come
from `spec/`, and the generated files are committed. The
generator refuses a program's command under 0x10, and a number that two
protocols both give.

When the core stops on a broken invariant or a panic, it sends its report after
what the turn has already sent, and runs no more; a trap stops it the same way
without a report. The runtime reads what the core sent, answers every request still waiting with ENOTRECOVERABLE, tells the UI
that the core stopped, and never runs it again. Where the core shares its
process with the runtime, a hook ends the core's thread rather than the process.

## The devices that lend the memory

A frame on the channel is complete in itself: what it names goes with it, and
once `send` returns the core may write over all of it. Some work cannot be
said that way, because it reaches into the core's memory after the turn that
asked for it has returned: a file's bytes copied into a buffer the core named, a
buffer the GPU uploads from, a word the GPU writes when a request finishes. Such
work needs a rule that says who owns which bytes of the core's memory and when
they come back, and only xpute sees both sides of that rule. So the runtime
implements exactly the devices whose work lends the core's memory across turns,
the GPU and the files, and the program implements everything else over the
channel.

Both are optional. The manifest names the devices the core needs, and the runtime attaches only what the manifest asks for. An open or a
close, which ask for a reply, from a core whose manifest has no file device is
answered ENODEV, and so is a read or a write, as any failure is. A boot that backs files the
manifest does not ask for, or backs none where it does, is refused.

Both name their objects by **handles**. The side that keeps an object hands out
a number for it. A handle holds a slot of the table that issued it and that
slot's generation, so a handle kept past its release names a generation the slot
no longer has, and it cannot name whatever took the slot next. The issuing side
checks the generation, as does a GPU runtime that keeps an object under the
core's handle, and 0 is never a handle. A table also issues tags, for what
holds no object, such as a written file: a tag is slot 0 under its next
generation, holds no slot, and is never a live handle.

### The file device

A core's files are WASI's: the core opens a file by its path (`path_open`),
reads it into a buffer it names (`fd_read`), writes it (`fd_write`) and closes
it (`fd_close`), each through a descriptor. The core issues each descriptor
itself, as a handle of its own file table, which keeps the owner of each open
file, and every frame of a descriptor carries it once, as its tag. A
descriptor is let go as its close is sent, as POSIX `close()` lets one go,
and what was in flight on it completes by its id, as io_uring's `user_data`
finds a completion: the close's completion finds what awaits it by its tag,
the descriptor with its generation, so a slot taken again never takes an old
completion.
The one difference from WASI is the path: a tuple of tokens, each an i32 or a
string, as many as the program uses, and not a string relative to a preopened
directory. xpute gives no token a meaning.

A file is read whole or written whole. Its open and its close are requests in
io_uring's manner. The core sends each with ACKREQ, and the runtime completes each once, on a later turn, with a frame of the same
tag and command, flagged RES. An open to read completes once the file can be
read whole, with its size or an errno, and a failed open leaves no descriptor.
A read names the memory it fills, and nothing completes it: the core sends its
close with it, the runtime serves a descriptor's frames in the order they came,
and so the close's completion says the read is done. What a read names is the
runtime's until then, and a read into memory that is not the core's, or at
offset 0, fills nothing, and its close completes with EFAULT. An open to write
completes at once, with 0, and an open whose flags ask for neither completes
with EINVAL. A write's
bytes go with its frame, as any frame carries the memory it names, so the core
keeps nothing, and the close replaces the file with what was written: it
completes with 0, or with the errno the file was refused with, as a close
reports a failed flush. A written file takes no slot of the core's file table:
its descriptor is a tag of slot 0, which no open file holds. The table holds a
read's buffer until its close completes, and hands each completion back to what
awaits it: a read's buffer, filled, or a write's result. A read
of a file opened to write, or a write of one opened to read, leaves the file
alone, and its close completes with EBADF. A close completes an open that has
not yet completed with ECANCELED first, and a runtime that reloads its program
cancels what the last one left, so the owners send again. When what backs a
file reports that it changed, the core is told, and it opens the file again to
read it.

The runtime's file server runs these calls, and it asks the program only for
its **binding**, one object the program hands it once, which answers every
path: it opens a file to be read whole, and takes a written file whole at its
close, refusing a file it does not write with EROFS, as a filesystem does.
Where a file lies and how its bytes are fetched are the binding's; xpute only
carries the call. The commands are in
`spec/abi/fs.json`.

### The GPU device

A renderer makes thousands of calls a turn to a device it cannot reach, and none
of them needs a reply. For those the core writes a **stream**: records laid end
to end during a turn, each an op, a count and that many words, published at once
by storing their count with release. The runtime runs every record and then
stores the count back to zero, which hands the stream back to the core. Until
then the records, and any memory a record names for the runtime to read, are the
runtime's, and the core neither rewrites nor frees them. On a page the runtime
runs the stream after the turn returns; on a device the GPU runtime takes it as
the count is stored and runs it beside the core. No turn waits on the device:
while the runtime holds the stream the core records nothing and goes on turning.
Either way the order is the acknowledgment: a record may name memory for the
runtime to write into, and the core reads that memory on a later turn.

The calls themselves are WebGPU's, written down once in WebGPU's own vocabulary.
An enumeration is its specification's strings in order, so a runtime hands the
string to the device as it is, and a call's words follow the descriptor WebGPU
takes, with nothing defaulted on the program's behalf. The core writes the calls
through the **encoder**, which names each object by a handle from one table. The
**GPU runtime** runs them, on WebGPU in a page and on wgpu in a native app, and
whatever a run throws ends the core, as a trap does.

Some calls finish on a later turn: an image copied in, a buffer read back, the
queue's work done, or a pipeline made by WebGPU's asynchronous call. Each such
call names a **fence**, a word of the core's memory, and a **ticket**, the next
value of a sequence that the core alone advances for that word. The GPU runtime
is the fence's only writer: once the call is done, it stores the ticket in the
fence with release, after the result the fence guards. The core loads the fence
with acquire and holds the call done when it equals the ticket the core asked
with, so a late answer to a request that was dropped carries an older ticket and
never matches. The device's block, which holds its limits and features, its
preferred format, the counts of its uncaptured errors and its unfinished frames
and its status, is written by the same rule, where the core asked for it.

Both GPU runtimes admit a frame by one rule: a turn draws only while fewer of
its frames are unfinished on the GPU than its frame latency, so taking a
surface's texture never blocks, and the GPU is never more than that many frames
behind. What a surface shows, a canvas on a page or a view's layer on a device,
is lent by the UI. The core answers a surface's open with a handle, and the UI
binds its canvas under that handle, to the GPU runtime, which keeps the surface
from then on. The manifest's GPU ask says which adapter to prefer, WebGPU's power preference,
and which features the core needs; the GPU plugin asks the platform for that
device, and `GPU_READY` tells the core when the device has answered, the first
time and again after a loss, with the device block's status saying what the
device is now. The ops and their words, the fence's results and the device's
block are in `spec/gpu/api.json`, and the stream's own rules are written out
beside the core's stream. The GPU's calls stand apart from `kit` and from both
ends, so a program that never draws links none of them.

A GPU's floats are held only as far as WGSL holds them. Its addition,
subtraction and multiplication round half to even, as every language here does,
but its division may be 2.5 ulps off and its square root and transcendentals
have only bounds, and a GPU may be further off than the specification allows: a
division has been measured 13 ulps off. A compiler may also fuse a product into
the sum after it, and Metal's, in its default fast math, reorders a sum, so no
order written in a shader holds. So a rule the CPU and a GPU both decide is
decided in integers, or in floats the CPU rounded once and handed to the GPU in
a word, a reciprocal in place of a division; and a rule the GPU alone decides
is one function every pass calls, so that the GPU at least agrees with itself.

## The languages, held to each other

xpute is written in TypeScript, Rust and C++, and the implementations do not all
stand in the same relation to each other. Which relation they stand in decides
what must match.

The kit is symmetric. Each language implements the same collections, codecs,
wire formats, errno table and scalar math, held to each other wherever bytes
must match. TypeScript's kit has no allocator and no bug reporter, because it
has no linear heap of its own and a thrown error carries its own stack.

The two ends face each other, and they owe each other the contract and its
formats, not matching lists of modules. The core end exists in Rust and in C++.
The manifest is Rust's, and the generator writes a program's protocols in Rust
and TypeScript; the C++ conformance core lays out its own front and writes its
header itself. The runtime ends hold what their platform needs. TypeScript's
boots a core on a page, or on a device through the native module. Rust's
reserves a native core's memory, runs its thread, keeps the core's heap inside
the core's memory, and exports the C ABI and, on Android, JNI. The GPU runtimes
split the same way, TypeScript's on WebGPU and Rust's on wgpu.

The C++ library is written from the contract and the formats, beside the Rust,
and where it differs, the language is the reason: it writes the codecs its
standard library lacks, its XTP reader hands back views into the packet rather
than copies, its tasks are C++20 coroutines where Rust's are futures, and it
uses RAII, templates and results, with no exceptions and no RTTI.

What holds the languages to each other is data, not a shared implementation.
Under `spec/golden/` are lines of a path and a value, produced once, checked
once and then frozen. Each language that implements a part reads that part's
records bit for bit, so a record is never recorded again to make a test pass: a
difference means that one side moved, and a record changed on purpose changes
for every language at once. Some records cover bytes that cross a boundary, such
as the packet formats and the words each GPU encoder writes; others cover values
that cross none but must still agree, such as the random generators and the
scalar math. Every rounding in the tree is one rule, half to even as WGSL
rounds (`math/scalar`), so the core, the UI and a shader round alike; a call
that rounds otherwise is refused by the tools. The kit still offers its other
roundings by their own names, as a library offers options.

The **traces** under `spec/conformance/` are a third kind of record. Each trace
says what a runtime does and what it must then see, written from the contract's
rules below and naming only what a runtime can observe. One runtime runs every
trace against every core, Rust's and C++'s, each built both as a native library
and as a module that imports nothing but its memory, the clock and its host.
Each core's boot lays its front in an order, and with gaps, that a seed draws,
and every trace runs under each seed, so only a runtime that reads the header
and takes each frame where the core names it passes. The C++ core is built from the two files a C++
program takes, so what the traces hold is what ships. The file device's server
has traces of its own, apart from the contract's.

---

## Boundaries

What xpute leaves out is part of its design.

- **The kit knows nothing of turns, runtimes or memories.** It is generic code
  that any program can link and call. The core end depends on it, and it never
  depends on either end.
- **No command of the program's has a meaning in xpute.** No domain type reaches
  the kit or either end, and the runtime routes a program's frame by its major
  and reads nothing of what it says.
- **The memory past the front is the core's.** The manifest says the sizes; what
  the core's ranges hold, where they lie, and what answers the heap are the
  core's.
- **The policy is the program's.** When turns run and how long they may last,
  what each life command means the core keeps or resets, and how much I/O may be
  out at once are all decided outside xpute.
- **The devices are only the two that lend memory.** Any other I/O the program
  does, a network's included, is the program's, over its own commands or behind
  its files, and xpute itself reaches nothing outside the device it runs on.
- **xpute is not real time.** A deadline is how long a turn should take, and a
  turn that runs past it is not stopped.
- **xpute does not enforce its grants.** The runtime grants a region and a
  deadline, and nothing stops a core that ignores them. The contract describes
  one runtime and one core; several cores under one runtime would have to bring
  what the contract does not, isolation and a way to stop a core that does not
  return.

## Invariants

The contract is the part that no end may change; the rest of this document is
how one end happens to be built. Each rule is one the other end could catch
being broken, and the traces hold every core to the rules they name by number.

The door keeps three rules, which the core's door writes out in full:

**R1.** Memory that a frame names goes with the frame, a reply's as an
import's, and once `send` returns it is the core's again, so nothing reads the
core's memory after the turn that sent it. What a file's read names is the runtime's until the
close of its descriptor completes.

**R2.** A completion finds its request by its tag.

**R3.** A call never nests, and its answer makes no call.

Around them, the contract's rules are these:

0. **The front.** The runtime lays the header before the core's first turn,
   where the manifest places it. The core holds it to its manifest and stops with
   ENOEXEC where a word differs, and a header without xpute's MAGIC is refused.
1. **The core memory.** Every address that crosses the boundary is an offset
   into the core memory. Offset 0 names nothing, and nothing that crosses lies
   there.
2. **The turn.** The core runs only inside `run`. The deadline is the core's to
   keep: a core can run past it inside one function, and the runtime cannot take
   the turn back until `run` returns. `wake_ms` is 0 for the next turn, a
   positive number for that many milliseconds from now, and a negative number
   for none. A turn with a deadline of 0 is still a turn. A question the runtime
   needs answered at once is asked in one, of a command the core answers from
   what it already holds; a question that waits on anything outside is answered
   on a later turn.
3. **The frame.** A frame is four words: its tag, its command with its flags,
   its result, and its packet's length, the packet lying right past them. With ACKREQ a submission gets exactly one
   reply; without it, a reply comes only when the submission failed. A reply
   carries its submission's tag and command, and RES. Majors under 0x10 are
   xpute's own.
4. **What the runtime sends** is applied in order, each submission once.
5. **What a turn sends**, its replies, its imports and its stream's records, is
   taken by the runtime, and none is dropped. Order holds within each place a
   turn writes to, not between places, so a core that needs one request run
   before another puts both in the same place.
6. **Handles.** A handle is a slot and that slot's generation, 0 is never a
   handle, and the issuing side checks the generation.
7. **The clock.** During a turn the core reads the runtime's clock in
   milliseconds. It never runs backward, and only differences mean anything.
8. **A call.** During a turn the core may hand the runtime a frame it laid in
   the frame it sends from (`call`). Whatever answers the frame's
   major answers it before the call returns: it reads that frame, writes only
   what the frame names, and answers with a value or a negative errno, or ENOSYS
   where nothing answers. It answers from what it already holds and waits on
   nothing outside, and the core runs nothing until it has answered.

Everything else is one end's own. The core's own are where its ranges lie past
the front, its heap, its passes and its tasks. The runtime's own are the
length of a deadline, when to run a turn, and how it reads what a turn left. The
two devices' protocols are xpute's, written in `spec/abi/` and `spec/gpu/`, and
they stand beside these rules: the rules say what the channel carries, and a
device says what its calls mean.

## Prior art

xpute's bindings come from WebAssembly's component model, its files from WASI,
its requests from io_uring, and its stream from a GPU's command buffer.

| Precedent | What is taken |
| --- | --- |
| A WIT world and its generated bindings | A protocol: its commands, each an export or an import, named from the core's side, and the code both ends are generated with |
| WASI's `path_open`, `fd_read`, `fd_write` and `fd_close` | The file device: descriptors, a read into the caller's buffer, a write and a close; a path is a tuple of tokens, each an i32 or a string, not a string relative to a preopened directory |
| A GPU's command buffer, as WebGPU's command encoder records one | The GPU stream: calls recorded during a turn and run by the runtime after it |
| Android's Frame Pacing library (Swappy) | Frame pacing, the scheduling policy for a display: turns a whole number of refreshes apart |
| An Android app's manifest | The manifest: the core's own declaration, at build time, of the memory, stack, packet room and devices it needs |
| A linker script's output sections | The core's ranges: laid end to end on pages without knowing what they hold |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `kit`'s buddy allocator of pages, ported line for line |
| io_uring | A request and its completion: a frame sent with ACKREQ, completed on a later turn by a frame of its tag flagged RES |
| POSIX and Linux errno | Every result: a negative errno number |
| IEEE 754's round half to even, as WGSL rounds | The one rounding in every language |

## Codemap

The code, in the order its parts depend on each other.

- `spec/` holds the contract as JSON: the header, the frame and the stream
  (`io/`), xpute's own commands and the devices' (`abi/`, `gpu/`), the
  device runtime's private batch to its UI (`io/batch.json`), the
  errno table (`status/`), the manifest's schema, the golden records (`golden/`)
  and the traces (`conformance/`).
- `rust/gen` is the generator: `xpute-gen` writes a program's manifest and
  protocols into both ends. `rust/dev/tool` is xpute's own use of it: `cargo
  spec` checks xpute's generated files, and `cargo spec --write` writes them.
- `rust/kit`, `ts/kit` and `cpp/xpute/kit` are the kit: `collection`, `codec`,
  `wire/xtp` and `wire/tlv`, `status` (the errno table and `bug!`),
  `math/scalar` and `math/rng`, and `alloc` (`buddy_tree`, `slab`).
- `rust/core` is the core end: `mem`, `manifest`, `entry`, `io`, `abi/handle`,
  `sched`, `host`, `clock`, `stop` and `wasm`. `cpp/xpute/core` holds the same
  parts but the manifest and the entry, which a C++ core writes for itself.
- `rust/gpu` is the GPU's calls, and behind its `runtime` feature the native GPU
  runtime with the GPU protocol's runtime end (`runtime::abi`). `cpp/xpute/gpu` is the encoder alone.
- `rust/runtime` runs a native core: `native`, where the core's thread and the
  rules of its loop live, `abi`, the runtime end of xpute's own protocols, `mem`
  and `jni`, with the C ABI in `ts/native/ios/CXpute/xpute.h`.
- `ts/runtime` (`@xpute/runtime`) is the runtime end: `runtime.ts`, `start.ts`
  and `remote.ts` (the port), `platform.ts` and `module.ts` beside
  `platform.native.ts`, `io/` (the connection, the frame, the header and
  the file server), and `sched/`.
- `ts/gpu` (`@xpute/gpu`) is the GPU runtime on WebGPU (`GpuRuntime`, `WebGpu`),
  and on a device `NativeGpu` and the `SurfaceView`, over its own Expo module.
- `ts/native` (`@xpute/native`) is the React Native module: the Kotlin and the
  Swift over the C ABI and JNI.
- `rust/dev/golden`, `ts/dev/golden.ts` and `cpp/dev/test/` are what the tests
  take: the golden records' reader, and in C++ the tests' runner.
- `rust/dev/conformance` and `cpp/dev/conformance` are a core in each language for the
  traces, and `ts/runtime/src/conformance.test.ts` is the one runtime that runs
  them.
- `rust/dev/example` and `ts/dev/example` are the smallest core, the test that drives
  it, and the page that runs it.
- `cpp/amalgamation.awk` makes the C++ library's two files.
