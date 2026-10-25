# Architecture — xpute

[xpute](./README.md) runs one program inside another and gives it resources rather than abstractions. The outer program, the **host**, owns the platform: the clock, the memory, whatever reaches outside. The inner one, the **guest**, owns none of it. The host grants a fixed memory once and a quota of time on every turn, and hands what the guest asks of the outside to the program: every file call to the program's one `SysHost.open`, which dispatches by root, and a GPU call to a device. Over those grants the guest builds what it needs — an allocator, a scheduler, tables of what it holds — and the policy each runs by.

The host/guest split follows the exokernel/library-OS division, not an operating system's with a process: the host grants resources, a real region and a real quota, and the guest builds its own abstractions over them; nothing is virtualized. xpute does not enforce those grants. A guest can spend a whole quota inside one function, and the host cannot take the turn back until it returns.

xpute comes in two layers. `kit` is generic code — collections, codecs, a wire format, an errno table, scalar math — that any program can link and call. `guest` and `host` are the arrangement itself, one end each, written against `kit`, which knows nothing of them. Neither layer knows what a guest computes, which is what lets the arrangement carry to the next program.

Three things cross between the ends and nothing else does: one memory they share, one call in, one number out. The sections follow from that:

1. the memory;
2. what is written into it — the message and its packet, and the rings that queue messages;
3. the call that makes it run, the turn;
4. handles, which name objects across the boundary;
5. I/O and a GPU's calls, which a guest asks of its host;
6. the contract all of it comes down to;
7. the languages, and the records that hold them to each other.

---

## The core memory, placed once

The memory is the one thing the two ends share, so its rules come first.

It is the **core memory**, core as in a core dump. The host reserves it at boot, or a guest linked into a native program maps it itself and sets its base (`mem/base`). Either way it never grows, so a view into it stays valid for as long as the guest lives. That is what lets a queue keep its words at a fixed address, and a payload be read where it lies.

What goes in the memory is the guest's business. The guest hands in a base, a unit and a list of sizes, and gets back where each range landed (`mem/section`), as a linker script places output sections without knowing what they hold. Whatever is asked of a range is checked against its end (`Section::at`, `nth`), so nothing written through one range lands in the next.

The memory need not begin at address zero. A WebAssembly module's does; a guest linked into a native program has a region wherever the system put it. So every address a guest names is an offset from the memory's base, never a pointer (`mem/base`). It means the same to the host however the guest is built, and it crosses as a `u32` however wide the machine's pointers are. A pointer becomes an offset in one place (`off_of`), which refuses one outside the memory; an empty slice is offset 0. C++'s also refuses an offset past a `u32`, where Rust's counts on the memory being no larger than a `u32` spans.

The last range is the heap, reached through a single call (`mem/heap`) whose answer is the guest's choice. `kit` offers two allocators that fit together:

- a buddy allocator of fixed pages (`alloc/buddy_tree.rs`), ported from evanw/buddy-malloc, which was written for this same situation: a module handed one heap with no system allocator beneath it;
- a slab of size classes over it (`alloc/slab.rs`), which keeps its bookkeeping inside the blocks it has not handed out.

Since the guest places everything, the host has to be told where. The guest writes a **directory** (`ipc/directory`) in its first turn, or in a boot call that returns its offset, as the conformance guests do. A head of four words — a magic, a version, the count of entries and one reserved — comes before the entries, each a key and a value: where each ring lies, the numbers of its quantum policy, and keys the program numbers for itself. The host learns one offset from outside, the directory's, and finds everything else from there. It refuses a directory whose magic or version it does not know, or with a key written twice.

---

## The message, and the packet it carries

Sharing a memory does not say what a request looks like, so xpute gives every request and every answer one shape.

A message is four words (`ipc/frame`):

- `tag`, who asked;
- `cmd`, what is asked, in the low 16 bits, with flags in the high 16;
- `result`, what came back, a value or `-errno`;
- where its **packet** lies — the data the message carries.

An errno is one table's (`spec/status/errno.json`), and every language reads a code back into its member, or into none (`Errno::of` in Rust, `errno_of` in the others). A command number is `major << 8 | minor` (`kit abi/cmd`). The majors below `0x10` are xpute's own commands, and a program numbers its own from `0x10` in a table that names xpute's as its base, so no number is given out twice.

Two flags decide what a message means. `ACKREQ`, set by whoever submits, asks for an answer. `RES`, set by whoever completes, says the message is an answer. A completion without `RES` is one the guest raised on its own, for whoever listens.

Every packet has the same shape, **XTP**, the Xpute Tree Packet (`kit wire/xtp`). One decision explains the rest of it: nothing inside a packet records where the packet is. Every offset is counted from the branch that holds it, so the same bytes mean the same thing at any address.

```text
one branch at byte 0, three children. Each offset is measured from that
base — not from the entry it sits in, and not from the start of the packet

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

no absolute address appears anywhere in it
```

Three things follow. A payload in a ring slot is read where it lies. A typed-array leaf is handed out as a view straight into the packet. And a packet encoded earlier grafts into a larger one without a number inside it changing.

The rest is layout: scalar leaves, typed-array sequences and branches, little-endian, on 8-byte words, behind a two-word header. A branch entry carries its child's type even when the child is absent. The format in full is [the note beside the TypeScript implementation](./ts/kit/src/wire/xtp/README.md), which the other languages mirror.

**TLV** (`kit wire/tlv`) is the other format: typed values in a forward-only sequence, closed by END. Where XTP is a value walked where it lies, a TLV is read once, in order. Its reader says whether a sequence stopped at END or where the input ran out, and what either means is the program's. Both formats live in `kit`, since neither knows the program using it.

---

## The rings, the doorbell and the door

Messages need a queue each way that neither side has to lock, and the ring is that queue.

A **ring** (`ipc/ring`) keeps the four-word descriptors in one place and the packets they point at in a separate range of the memory:

```text
descriptors                                  payloads, elsewhere in memory
┌──────────────────────────────────────────┐  ┌────────┬────────┬───┬──────────┐
│ header: capacity · head · tail ·         │  │ slot 0 │ slot 1 │ … │ slot n-1 │
│                    ▲      ▲              │  └────▲───┴────────┴───┴──────────┘
│      consumer ─────┘      └───── producer│       │
│  one writer each, and no lock            │       │
│       slot_bytes · slot_base · 3 reserved│       │
├──────────────────────────────────────────┤       │
│ message n: tag                           │       │
│            cmd (low 16) | flags (high 16)│       │
│            result (value or -errno)      │       │
│            packet address ───────────────┼───────┘  (slot n & (capacity-1))
└──────────────────────────────────────────┘
```

Keeping descriptors apart from payloads is what virtio's descriptor table and AF_XDP's UMEM do: the descriptors are small and stay hot, the payloads are large and do not.

Positions only grow, so `tail - head` is how many messages wait. Message _n_ owns slot _n_, so a slot is free exactly when its message has been read, and nothing under the ring allocates. A push fails in two ways only. `EAGAIN` means the ring is full, and the producer waits. `EMSGSIZE` means the packet is bigger than a slot, and such a packet crosses by address instead, in a heap block. A packet the host wrote anywhere but the tail's slot is not a third way to fail but a bug, and its push stops on it (`ENOTRECOVERABLE`).

Two rings make a pair: one for what the host submits, one for what the guest completes. The host drives the pair through the **doorbell** (`ipc/doorbell.ts`):

- it writes each command in place, into the submission ring's next slot (`writer`, `send`);
- it rings the guest with a quota (`ring`), which is one whole turn;
- it then reads the completion ring dry, handing each message to the call waiting for it or to whoever listens for signals.

Two ways in sit on top of that. A question the host needs answered at once is a `call`: submitted with `ACKREQ` and rung with no quota until its reply comes back. A command from outside the host's own code, such as a developer's console, is `cmd`: it takes the command's number and its arguments, writes each argument as the packet's next field, lowered as XTP lowers a value (`TreeView.set`), then sends and rings. A guest that traps is gone, and the doorbell rings it no more.

The guest's end is the **door** (`ipc/door`). It applies the submissions in order, answers each, and posts what the guest raises into the completion ring. That ring holds a whole turn's output. Before each submission the door checks that the ring has room for the most one command may leave, its signals and its reply, a bound the program gives (`most_signals`). Where it has not, the door applies no more and leaves the rest for the next turn, in order. The room is checked there and nowhere else, so what a turn raises after its drain is covered only where the drain ran out of submissions, not of room. The door keeps its own peaks since they were last taken, the completion ring's length, the submission ring's as a drain starts and the submissions a drain leaves, for the program to take (`take_stat`).

---

## The turn

The turn is when the guest runs at all.

It runs inside one call, `interrupt(quota_ms)`, and when that call returns it is not running. The call has two edges, like a clock's. On the rising edge the host calls in with a quota, and the guest drains the submission ring, does its work and posts what it has. On the falling edge it returns `wake_ms`, when it would like the next turn: `0` for the next frame, `n` for a time, `NO_WAKE` for none. The host learns nothing else about what ran.

How long a turn may take, and how that time is spent, are settled at three levels, from the outside in.

1. **Quantum** (`sched/quantum`) is the host's, and it sets the quota: a cooperative counterpart of Aegis's CPU slice, ended by the guest's return rather than a timer. The host measures the display's refresh from the gaps between the frames it is handed, and paces turns a whole number of refreshes apart (`paced`). The guest's directory declares the rest: the share of a frame left unspent (`margin_share`), how many frames a batch turn spans (`batch_frames`), and how long after input a turn counts as interactive (`settle_ms`), as is one after a falling edge of 0. A turn that runs long is not cut off; its overrun, the whole ring's time past the quota, comes off the next quota.
2. **Tick** (`sched/tick`) is the guest's. It runs phases named once for every language (`spec/sched/tick.json`) in a fixed order — `interaction`, `visible`, `content`, `cosmetic`, `report` — against one budget, the quota counted from the rising edge, and does not divide it among them. While `visible` runs, the share kept for `content` (`content_share`) is held back from it; no other phase is held back. What runs in each is the program's. A phase's time is recorded, never stopped.
3. **Pass** (`sched/frame_budget`) spends what is left of the budget on an ordered list of steps until the budget or a step cap says stop. Both are waived until a step has done work, so nothing starves behind what sits ahead of it. The hard budget, what is left plus the tick's `hard_slack_ms`, is never waived, but the first step is always tried.

Work too long for one turn is a **task**, written against `sched/edge` in its language's form, a future or a coroutine. A task gets the turn's quota as a deadline, `yield_if_spent` to stop once the deadline has passed, and `next_turn` to wait for the next turn. The guest may move the deadline past the quota (`extend_to`), keeping its tasks a share past what ran before them. A task parked by either makes the falling edge 0, the next frame. Whatever assembles the guest picks an executor, marks the edges with `rise` and `fall`, and polls it between them.

On the host side, the **strobe** (`sched/strobe.ts`) decides when a turn of work is rung. It rings on an animation frame, only when the host or the guest's last falling edge asked for a turn, and only on the refreshes the quantum paces at; a frame sooner than that is skipped, and the next asked for. A positive `wake_ms` sets a timer that asks for a frame when it fires. A guest that asks for nothing is rung for nothing, and no turn runs on a timer of the host's own.

The strobe's are not the only rings. The doorbell rings with no quota to make room in a full submission ring, to wait for a call's reply and to send a `cmd`, and whatever holds the doorbell may ring it too. Those turns are outside what the quantum observes.

```text
host · TypeScript                          guest
─────────────────                          ─────

strobe   a frame, and only when one
         was asked for
quantum  a quota, from the frame it
         measured rather than assumed

commands ─────── submission ring ───────▶  drained on the rising edge
doorbell ═════ interrupt(quota_ms) ═════▶  ┌ tick   phases in a fixed order
                                           │ tasks  that yield when
                                           │        the quota is spent
                                           │ asks   what it wants done outside
completions ◀───── completion ring ──────  └ posted before the falling edge
         ◀═══════════ wake_ms ═══════════  next frame · in n ms · never

requests what the turn asked for, run
         before the next rising edge

         … nothing until wake_ms …         … nothing at all …
```

---

## Handles

A handle lets both sides name one object while only one side holds it.

The side that keeps the object hands out a number for it, as an operating system hands out a file descriptor. That number is a **handle** (`abi/handle`): `slot | generation << 16`.

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

Every table is fixed at N slots, the one over what another side keeps (`HandleTable`) and the one over values a side keeps itself (`Slots`) alike. Slot 0 is never handed out, so 0 means "none". Handles are in `guest` and `host` rather than `kit`, because a handle is one side of a boundary, not a container.

---

## I/O, by asking

A guest cannot do I/O, because the host owns the connections, the way an operating system owns its devices. So the guest asks, in what its turn leaves, and the host (`ts/host`) hands the ask to the program and brings back what came of it — in a message, or into memory the request named. It does not route by root: every file call goes to the program's one `SysHost.open`, and the program dispatches by root to whatever it bound there. What a file is, where it lies and how it is fetched are the binding's; xpute only carries the call.

How much may be out at once, and which want goes first, xpute does not say: ordering demands needs to know what the program is for. A program that wants a bound passes one in its own messages.

### Streams

Some requests come in thousands a turn — every call a renderer makes to a device it cannot reach. A message each would be a slot and a packet each, for calls that need neither a reply nor a tree. For those the guest writes a **stream** (`ipc/stream`): records laid end to end during a turn, which the host runs, all of them, after the turn returns.

```text
0       END   how many words of records the turn wrote
1…            the rest of the head, the program's
head…   op · n · n words   op · n · n words   …
```

A record is an op the program numbers, a count, and that many words. Since the whole stream runs between two turns, a record can name memory for the host to write into, and the guest reads it next turn: the order is the acknowledgment.

A record past the range is a sizing bug, not a state to handle, and stops the guest. Inside a transaction (`begin`, `commit`), a record that does not fit drops the transaction whole, and `commit` says so. A program's call can be tried instead (`try_call`), which records nothing where the call would leave less than the room asked, for the caller to ask again a later turn.

### System calls and files

Some calls are every program's: open, read, write and close a file, and report the invariant that stopped the guest. Those are xpute's **system calls** (`ipc/sys`), records in one stream the directory names. The report (`ABORT`) drops the turn's other calls and is published alone, at once. A program's own calls go in the same stream, numbered past a PROGRAM bit.

Files borrow Plan 9's file namespace:

- the guest issues the descriptor, a handle of its own file table (`ipc/file`), which is fixed at N slots as every table is;
- it names a file by a root the program numbers and a path below it, and the program binds each root to wherever it keeps things;
- a path walks down a name at a time, and an empty name, `.` or `..` is refused, so a root is the whole of what it grants.

An open is the one call that waits: the host answers it on a later turn, once the file can be read whole. A read, a write and a close are done before the next turn. A close ends a descriptor whether or not its open was answered, so a late answer reaches nothing.

These are a service the boundary carries, not rules of the boundary, so their traces are apart from the contract's: `spec/conformance/sys/` holds the host's end, and `spec/conformance/file.tsv` every guest.

### A GPU's calls, in WebGPU's words

A renderer's calls are what streams were made for, and most of what they say is WebGPU's, not one program's. So xpute writes them down once, in WebGPU's vocabulary (`spec/gpu/`). An enumeration is its spec's strings in order, so a host hands the string to the device as it is. A call is an op whose words follow the descriptor WebGPU takes, with nothing defaulted on the program's behalf.

A guest writes them through the encoder (`gpu/encode`), which takes each object's handle from one table and writes each record twice, once to count its words and once into the room reserved for them. A host runs the stream on its device after the turn returns (`gpu/host`). Three calls finish later — an image copied in, a buffer read back, the queue's work done — so each names a word the host sets to its ticket with `DONE`, and the guest reads the word rather than counting on order.

The native host reaches the guest's memory as a view by offset (`Memory`), each read or write only the range the guest names, since the host's own blocks may lie in the same memory. And it paces frames, which a browser does for the web's: a turn draws while fewer than `FRAME_LATENCY` frames it admitted are unfinished (`admit`), so taking a surface's texture never blocks.

The GPU's calls stand beside `kit` and the two ends: a program that never reaches a GPU links none of it. What a program builds over them, such as a shorthand for its pipelines, stays the program's.

---

## The contract

The contract is the part neither end may change; the rest of this document is how one end happens to be built.

What cannot change is the rules below and the formats they travel in: the directory, the message's four words, the ring's layout and the stream's records. Each rule is one the other end could catch being broken.

0. **The directory.** The guest writes it in its first turn or in a boot call that returns its offset, and the host finds everything from the one offset it learns outside. A key appears at most once, an unknown key is passed over, and an unknown version is refused.
1. **The core memory.** It never moves or grows while the guest lives, and every address that crosses is an offset into it.
2. **The turn.** The guest runs only inside `interrupt`, and the two sides never run at once: the host touches nothing a turn writes until it has returned. The quota is advice, not a deadline. `wake_ms` is 0 for the next turn, a positive number for that many milliseconds from now, and negative (`NO_WAKE`) for none. A turn of no quota is still a turn, and a question the host needs answered at once is asked in one — but only of a command the guest answers from what it already holds; one that waits on anything outside is answered on a later turn.
3. **The message.** Four words: `tag`, `cmd` with its flags, `result`, and the packet. With `ACKREQ` a message gets exactly one reply; without it, a reply only when it failed. A reply carries its message's `tag` and `cmd`, and `RES`.
4. **What the host sends** is applied in order, each message once. The guest may leave some for a later turn, never out of order or skipped, and asks for the next turn while it does.
5. **What a turn leaves** — messages, replies, stream records — is taken by the host before the next turn, none dropped. Order holds within one place a turn writes to, not between places, so a guest that needs one request run before another puts both in the same place. Running past the room set aside is a sizing mistake, not a state to handle.
6. **Handles.** A handle is `slot | generation << 16`, 0 is never one, and the issuing side checks the generation.
7. **The clock.** During a turn the guest reads the host's clock (`now`) in milliseconds. It never runs backward, and only differences mean anything.

Everything else is one end's own. The guest's: where its ranges lie, its heap, its tick, passes, tasks and files. The host's: the size of a quota, when to ring, how it reads what a turn left.

The contract describes one host and one guest. Several guests under one host, passing buffers by ownership, do not change it. Where they share a thread or do not trust each other, that composition must bring what the contract does not: isolation, and a way to stop a guest that does not return.

---

## Written in several languages

The implementations do not all stand in the same relation, and which relation decides what must match.

- **`kit` is symmetric.** Rust (`rust/kit`), TypeScript (`ts/kit`) and C++ (`cpp/kit`) implement the same kit, held to each other by the golden records wherever bytes must match. TypeScript's has no `alloc`, since it has no linear heap of its own, and no `bug`, since a thrown error carries its stack.
- **The two ends face each other.** A host end and a guest end owe each other the contract and its formats, not matching module lists; a module with nothing facing it is not a gap.
- **Guest ends are symmetric with each other.** Rust (`rust/guest`) and C++ (`cpp/guest`) are the same end, either plugged into any host, so what one gives a program the other gives too. The host end is TypeScript (`ts/host`).
- **The GPU's calls follow the same split.** Each guest language writes them (`rust/gpu`, `cpp/gpu`). The hosts run them in TypeScript on WebGPU (`ts/gpu`), and in Rust on wgpu behind the `host` feature.
- **A guest built as a module is linked one way.** Its memory is imported from the host and its stack goes first, at the bottom of the memory, so an overflow traps; its stack's size and any front it keeps below the linker's data are its own numbers. A Rust guest's build script prints the flags through `rust/link`, and the C++ guest's Makefile passes the same ones.

`kit` asks nothing of a program: link it and call it. `guest` and `host` ask it to run xpute's way — memory granted once, work in turns, I/O by asking.

C++ is an implementation beside the Rust, not a port of it: written from the contract and the formats, built by the platform's compiler (`cpp/Makefile`), and held by the same records and traces. That is its point — what guests agree on is what the boundary shows, not what one was copied from. Where it differs, the language is why:

- it writes the codecs its standard library lacks;
- its XTP reads a packet without allocating, handing back views;
- a task is a C++20 coroutine where Rust's is a future;
- the memory is a value a call names rather than one base per module, so one native host may run several guests in one process;
- it has no `global`, which Rust needs a type for and C++ has as a plain static;
- it uses RAII, templates and `Result`, with no exceptions and no RTTI.

Its numbers — the errno table, the directory's keys, a frame's and a ring header's words — come from `spec/` as the other languages' do.

---

## The golden records

The golden records are what hold the languages to each other.

Under `spec/golden/` are lines of `path<TAB>value`, produced once, checked once, and frozen. They are read by splitting each line (`kit golden`), so no parser change can make them drift. Rust, TypeScript and C++ each read every record bit for bit, so a record is never re-recorded to make a test pass: a difference means one side moved, and a record changed on purpose changes for all three at once. There are two kinds:

- **Across a protocol** — `wire/xtp`, `wire/tlv`, `codec/json` (which texts are JSON at all, and what a number past the doubles reads as, as the host's own `JSON.parse` judges them), and `gpu/programs` (the words each encoder writes and the bytes each host reads back). The bytes cross a boundary, so every language must produce these exactly.
- **Across languages** — `math/rng` (`rng.32` and `rng.64`), `math/scalar`, `collection/deque` and `collection/heap`. Nothing crosses a boundary, but one set of values holds every language's implementation: the same draws, the same rounding, the same order.

The **traces** under `spec/conformance/` are a third kind, not outputs at all. Each says what a host does and what it must then see: the completions, stream records and next-turn asks each turn leaves. They are written from the contract's rules and name only what a host can observe, so they hold every guest to the same thing.

One host runs them against every guest through the boundary alone (`ts/conformance/src/trace.test.ts`): the Rust guest (`conformance guest.rs`) and the C++ one (`cpp/conformance/guest.cpp`), each over the commands `spec/conformance/guest.json` numbers. Each guest is built both as a native library and as a WebAssembly module that imports nothing but its memory and the clock.

## Prior art

What xpute took from work before it, or found it had arrived at. A link is given where the tree records the source.

| Precedent                                                   | Where                            | What is taken                                                                  |
| ----------------------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------ |
| The exokernel and its library operating system              | the arrangement                  | The host grants real resources; the guest keeps its own abstractions over them |
| Aegis                                                       | `sched/quantum`                  | The quota as a cooperative counterpart of its CPU slice                        |
| Android Swappy's frame pacing                               | `sched/quantum`, `sched/strobe`  | Turns a whole number of refreshes apart                                        |
| "core" for main memory, as in a core dump                   | the core memory                  | Its name                                                                       |
| A linker script's output sections                           | `mem/section`                    | Ranges placed end to end without knowing what they hold                        |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `alloc/buddy_tree`               | The page allocator, ported line for line                                       |
| SLUB                                                        | `alloc/slab`                     | A slab keeps its free list inside its free slots                               |
| virtio's descriptor table, AF_XDP's UMEM                    | the rings (`spec/ipc/ring.json`) | Descriptors kept apart from the payload's slots                                |
| io_uring                                                    | the rings                        | A submission ring and a completion ring, rung by a doorbell                    |
| Plan 9's file access, in POSIX's words                      | `ipc/sys`                        | Descriptors the guest issues; a root and a path; the host binds the roots      |
| The ELF auxiliary vector                                    | `ipc/directory`                  | Entries a reader skips when it does not know them                              |
| POSIX and Linux errno                                       | `spec/status/errno.json`         | Negative errno numbers                                                         |
| The host's `JSON.parse`                                     | `codec/json`                     | The grammar, judged as the host's parser judges it                             |
| IEEE 754's round half to even, as WGSL rounds               | `math/scalar`                    | One rounding in every language                                                 |
| splitmix64                                                  | `math/rng`                       | A reproducible generator, its seed's finalizer, and its high half as a u32     |
| FNV                                                         | `codec`                          | The hash                                                                       |
