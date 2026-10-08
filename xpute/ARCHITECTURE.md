# Architecture — xpute

[xpute](./README.md) runs one program inside another, and gives it resources rather than abstractions. The outer one, the **host**, owns the platform: the clock, the memory, whatever reaches outside. The inner one, the **guest**, owns none of it on its own account and controls what it is granted. xpute gives it no way to take more memory from the platform, open a connection, or run itself; those belong to the host. The host grants it a fixed memory once and a quota of time on every turn, and does the outside work it asks for. What those resources mean is the guest's: over them it builds what it needs — an allocator, a scheduler, paging, tables of what it holds — and the policy each runs by.

That split is an exokernel's with its library operating system more than an operating system's with a process: the outer side grants concrete resources, a real region and a real quota, and their management lives with the program. Nothing is virtualized, and the guest knows exactly what it was given. Unlike an exokernel, the split is not a protection boundary for a guest that is not trusted, and nothing enforces the grant: a guest can spend a whole quota inside one function, and the host cannot take the turn back until it returns.

xpute is that arrangement, in two layers. `kit` is generic code — collections, codecs, a wire format, an errno table, scalar math — that any program can link and call, host or no host. `guest` and `host` are the arrangement itself, one end each: the memory, the messages, the turn. They are written against `kit`, and `kit` knows nothing of them. `kit` exists in every language xpute has, and each end in the languages that end is built in; what that means for each of them is the last section, once the pieces have names.

Neither layer knows what a guest computes. No domain type of any kind reaches them, which is what lets the arrangement carry from the program it was written for to the next one.

Three things cross between them and nothing else does: one memory they share, one call in, one number out. The sections follow that order — the memory, what travels through it, the call that makes any of it run — then I/O and handles, the contract all of it comes down to, how it exists in several languages, and last the records that hold the languages to it.

---

## The core memory, placed once

The two sides share one memory and nothing else, the **core memory** — core in the old sense of a program's main memory, as in a core dump, not a CPU's. The host reserves it at boot and never grows it, so a view taken into that memory stays valid for as long as the guest lives. Everything below rests on that: a message queue can keep its words at a fixed address, and a payload can be read where it lies rather than copied out of the way first.

What goes in the memory is the guest's business; placing it is the guest end's. The guest hands in a base, a unit and a list of sizes, and gets back where each range landed (`mem/section`). A linker script does the same thing with output sections, for the same reason: whatever places ranges should not need to know what any of them hold. A range comes back as a section, and whatever is asked of it — a span to write, or ranges laid inside it — is checked against where it ends (`Section::at`, `nth`), so nothing written through one range lands in the next.

The memory need not begin at address zero. A WebAssembly module's linear memory does; a guest linked into a native program is given one region instead, wherever the system put it. So every address a guest names — a heap block, a range, a ring, what a record hands its host — is an offset from the memory's base and never a pointer (`mem/base`): a number means the same to the host however the guest is built, and it fits the `u32` a record carries however wide the machine's pointers are. A pointer becomes an offset in one place (`off_of`), and one outside the memory is refused rather than passed on, since a static or a read-only constant lies where the linker put it and not in the region. An empty slice names nothing and is offset 0, since its pointer is only its alignment.

The last range is the heap, and blocks come out of it through a single call (`mem/heap`). What answers that call is the guest's choice. `kit` offers two allocators that fit together. The lower one is a buddy allocator, which hands out fixed pages over a given span (`alloc/buddy_tree.rs`, ported from evanw/buddy-malloc). Above it sits a slab allocator holding size classes (`alloc/slab.rs`), so that a small request takes a slot of its own class instead of a whole page. The slab keeps its bookkeeping inside the blocks it has not handed out, so what it costs does not grow with the span it covers.

The buddy is not ours: it was written for this same situation, a module handed one heap with no system allocator beneath it.

Where anything lies is the guest's choice, so the host has to be told. The guest's first turn writes a **directory** (`ipc/directory`), in xpute's own format: a magic, a version, then entries of a key and a value — where each ring lies, the numbers of its quantum policy, and whatever keys the program numbers for itself. The host learns one offset outside the memory, the directory's — a number both ends are built with, or one the loader hands over — and finds everything else it reads from there.

---

## The message, and the rings that carry it

Sharing a memory does not say what a request looks like. Every request and every answer in xpute is the same four words (`ipc/frame`): who asked (`tag`), what is asked (`cmd` in the low 16 bits, flags in the high 16), what came back (`result`, a value or `-errno`), and where the packet is. An errno is one table's (`spec/status/errno.json`), and every language reads a code back into its member — `Errno::of` in Rust, `errno_of` in the others — or into none where the table names no such code. A command number is two bytes, `major << 8 | minor` (`kit abi/cmd`). The majors below `0x10` are xpute's, every program's commands — the rings' own test and the answer to a file's open (`guest`'s and `host`'s `abi/cmd`) — and a program numbers its own from `0x10` in a table that names xpute's as its base, so no number is given out twice.

Two of the flags in that high half decide what a message means. `ACKREQ` is set by whoever submits, and asks for an answer. `RES` is set by whoever completes, and says the message is an answer — to a call that asked for one, or to a call that failed. A completion without `RES` answers nothing: the guest raised it on its own, and whoever cares about it listens for it.

Four words are small, and a request's data usually is not. So the words go in one place and the data in another. What carries them is a **ring** (`ipc/ring`): the four-word descriptors in one place, and the packets they point at in a separate range of the memory.

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

Keeping descriptors apart from payloads is what virtio's descriptor table and AF_XDP's UMEM do, for the same reason: the descriptors are kilobytes and stay hot, the payloads are megabytes and do not.

Positions only ever grow, so `tail - head` is how many messages are waiting. Message _n_ owns slot _n_, which means a slot is free exactly when its message has been consumed — there is no allocator under the ring and nothing to free.

A push can fail in two ways and no others. `EAGAIN` means the ring is full: the producer waits, and nothing is dropped. `EMSGSIZE` means the packet is bigger than a slot, and a packet that big crosses by address instead, in a heap block the guest hands out and the host writes into.

Two rings make a pair: one for what the host submits, one for what the guest completes. Driving that pair from the host end is the **doorbell** (`ipc/doorbell.ts`). It pushes the commands, rings the guest with a quota, and then reads the completion ring dry, handing each message either to the call that was waiting for it or to whoever listens for signals. One ring of the bell is one whole turn of the guest.

The guest's end, the **door** (`ipc/door`), applies what waits in the submission ring in order, answering each with what the guest made of it, and posts what the guest raises, and its replies, into the completion ring. That ring holds a whole turn's worth: the host reads it dry before it rings again, and the guest stops applying submissions while too little room is left for what one more might raise, leaving the rest for the next turn, in order. Each message is written once, in place.

## Streams, for calls too many to be messages

Some of what a guest asks of its host comes in thousands a turn: every call a renderer makes to a device the guest cannot reach, say. A message each would be a slot and a packet each, for a call that needs neither a reply nor a tree. For those the guest writes a **stream** (`ipc/stream`): records laid end to end in a range of the memory during a turn, which the host runs, all of them, when the turn returns and before it rings again.

```text
0       END   how many words of records the turn wrote
1…            the rest of the head, the user's
head…   op · n · n words   op · n · n words   …
```

A record is an op the user numbers, a count, and that many words. Because the whole stream runs between two turns, a record can name memory for the host to write into, and the guest reads it on its next turn without being told it is there: the order is the acknowledgment. What a stream costs is its range, which bounds what one turn can ask for; a turn that asks for more has outgrown the range it was given, and the writer refuses the record rather than dropping it.

Some of those calls are every program's: open, read, write and close a file, and report the invariant that stopped the guest. Those are xpute's **system calls** (`ipc/sys`), records in one stream the directory names, with numbers xpute gives them; a program's own calls go in the same stream, numbered past a PROGRAM bit, the way its directory keys are.

Files are reached as Plan 9 reaches them: the guest issues the descriptor, a handle of its own, and names a file by a root the program numbers and a path below it, and the host binds each root to wherever it keeps things. An open is the one call that waits — the host answers it with a command on some later turn, once the file can be read whole, however many turns that takes — and a read, a write and a close are done before the next turn. A close ends a descriptor whether or not its open was answered, and nothing is said of it again, so an answer that comes late reaches neither it nor a descriptor issued after it in its slot. A path walks down from its root a name at a time, and a name that is empty, `.` or `..` is refused, so a root is the whole of what it grants. The host's end runs xpute's and hands the program's to the program, so a guest asks for a file the same way whatever host it runs under.

These are a service the boundary carries, not rules of the boundary, so their traces are apart from the contract's: the host's end is held by `spec/conformance/sys/`, and every guest by `spec/conformance/file.tsv`, which checks that an answer reaches only a descriptor still open.

---

## The packet

The address in a message points at a packet, and every packet on the rings has the same shape. That shape is **XTP**, the Xpute Tree Packet (`kit wire/xtp`), and one decision explains the rest of it: nothing inside a packet records where the packet is. Every offset is counted from the branch that holds it, so the same bytes mean the same thing at any address.

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

Three things follow. A payload in a ring slot is read where it lies instead of being copied somewhere stable first. A typed-array leaf is handed out as a view straight into the packet. And a packet encoded earlier grafts into a larger one with the layout an inline node would have had, without a single number inside it changing.

The rest is layout. A tree of scalar leaves, typed-array sequences and branch nodes, little-endian, its tables and headers on 8-byte words, with two words of packet header first — the magic, then the root's size and type. A branch entry carries its child's type whether or not the child is there, so an optional node that is absent still says what it would have been.

The format in full, and the API that writes it, is [the note beside the TypeScript implementation](./ts/kit/src/wire/xtp/README.md), which the other languages mirror.

**TLV** (`kit wire/tlv`) is the other format: a forward-only sequence of typed values, each tag naming its payload's type and length, closed by END. Where XTP is a structured value kept and walked where it lies, a TLV is read once, in order, and nothing it has passed is kept — an argument list as much as values arriving over time. Its reader says how a sequence stopped, at END or where the input ran out between two values, and what either means is the user's: a message may require END, and a journal cut short by its writer is a readable prefix. Both live in `kit`, because neither knows anything about the program using it.

---

## The turn

The guest does not run continuously. It runs inside one call, `interrupt(quota_ms)`, and when that call returns the guest is not running at all.

That call has two edges, like a clock's. On the rising edge the host calls in with a quota. The guest drains the submission ring, spends the quota on its own work, and posts what it has to the completion ring. On the falling edge it returns one number, `wake_ms`, which is when it would like the next turn: `0` for the next frame, `n` for a time, `NO_WAKE` for none. The host learns nothing else about what ran.

How long a turn may take, and how that time is spent inside it, are separate problems. They are settled at three levels, from the outside in.

1. **Quantum** (`sched/quantum`) is the host's, and it decides the quota itself — a cooperative counterpart of the CPU time slice an exokernel hands a program (Aegis), which the program schedules within, ended here by the guest's return rather than by a timer. The size comes from a frame interval the host measures rather than assumes. The shape comes from the guest, which declares a policy the host applies: a margin share for interactive turns, a number of frames for batch turns, a settle time after input, and what the falling edge is allowed to answer with. A turn that runs long is not cut off; the overrun is subtracted from the next quota, down to nothing.
2. **Tick** (`sched/tick`) is the guest end's, and it divides the quota. Its phases are named once for every guest's language (`spec/sched/tick.json`) and run in that order, which is their priority, against one budget counted from the rising edge: `interaction` unbudgeted, `visible`, which may not take the share kept for `content`, then `content`, `cosmetic` and `report`. What runs in each phase, and the shares, are the program's; the tick knows the quota and the clock and nothing else. Nothing enforces the quota — a phase that runs past its share is not stopped, it is simply counted against the turn.
3. **Pass** (`sched/frame_budget`) is one phase's share spent on one ordered list of steps, until a wall-time budget or a step cap says stop. It is not a registry — what is eligible, and in what order, is the caller's. What it does guarantee is forward progress: the first step that has work always runs, so nothing can be starved by whatever sits ahead of it in the list.

Work too long for one turn is a **task**, written against `sched/edge` in its language's own form — a future, a coroutine. That module hands a task the turn's quota as a deadline on the host clock, plus `yield_if_spent` to stop when the quota is gone and `next_turn` to park until the next turn begins. Nothing in that interface names an executor. Whatever module assembles the guest picks one, marks the two edges by calling `rise` and `fall`, and polls the executor between them.

On the host side, something has to decide when to ring at all. That is the **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only when something asked for a turn. Two things can ask — the host, when input arrives or a file opens, and the guest, on its last falling edge. A guest that asks for nothing is rung for nothing, and no turn ever runs on a timer of the host's own.

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

## I/O, by asking

The guest cannot do I/O, because it does not own the connections; the host does, the way an operating system owns its devices. So the guest asks, in what its turn leaves, and the host does the work and brings back what came of it — in a message, or into memory the request named.

How much may be out at once, and which want goes first, xpute does not say. Ordering one demand against another needs to know what the program is for — what is on screen, what is about to be wanted, what a failure means — and none of that is either end's to hold. A program that wants a bound on its I/O passes one in its own messages, beside the quota.

---

## Handles

The two sides need to talk about the same object without either one being able to hold the other's pointers. So one side keeps the object and hands out a number for it, exactly as an operating system keeps a file and hands out a descriptor. That number is a **handle** (`abi/handle`), and it is two fields in one word: `slot | generation << 16`.

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

A table is fixed at N slots and never grows; one over values a side keeps (`Slots`) grows as they come, and gives a removed value's handle to nothing. Slot 0 is never handed out, so 0 is never a valid handle and can be used for "none". A guest's file descriptors are handles of its file table (`ipc/file`), which is why an answer for one already closed finds nothing.

Handles are in `guest` and `host` rather than `kit` because a handle is one side of a boundary, not a container.

---

## The contract

Most of what is above is how one end happens to be built. A tick is how one guest divides its quota, a completion ring sized to a turn is how one guest keeps its output in bounds, a strobe is how one host decides when to ring; each end could be built another way, in another language, and be the same arrangement to the other. What cannot change is shorter: the rules below, and the formats they travel in — the directory, the message's four words, the ring's layout and the stream's records — which are the boundary's and not either end's. Each rule is one the other end could catch being broken.

0. **The directory.** The guest's first turn writes it, and the host finds everything else it reads from the one offset it learns outside the memory. A key appears in a directory at most once, and one the host does not know, it passes over.
1. **The core memory.** It never moves and never grows while the guest lives, and every address that crosses is an offset into it.
2. **The turn.** The guest runs only inside `interrupt`, and the two sides never run at once: the host touches nothing a turn writes until the turn has returned. The call carries a quota, which is advice and not a deadline, and returns `wake_ms`: 0 asks for the next turn as soon as the host schedules one, a positive number for one that many milliseconds from now, and a negative one (`NO_WAKE`) for none. A turn of no quota is still a turn, and a question the host needs answered before it goes on is asked in one. Which commands may be asked that way is each command's own: one the guest answers from what it already holds, waiting on nothing outside, may be; one that waits on anything outside is sent and answered on a later turn. A command is one or the other, never both.
3. **The message.** Four words: `tag`, `cmd` with its flags, `result`, and the packet. With `ACKREQ` a message gets exactly one reply; without it, a reply only when it failed. A reply carries its message's `tag` and `cmd`, which is how an answer finds its caller, and `RES`, so a completion without it is one the guest raised on its own.
4. **What the host sends** is applied in the order it was sent, each message once. The guest may leave some for a later turn, but never out of order and never skipped, and while it leaves any it asks for the next turn (`wake_ms` 0).
5. **What a turn leaves** — the messages it raises, the replies it makes, the records of its streams — is taken by the host before the next turn begins, none dropped. Order holds within each place a turn writes to, the completion ring or one stream, and not between them: the host may take all of one before the other, so a guest that needs one request run before another puts both in the same place. What one turn may leave is bounded by the room set aside for it, and running past that room is a mistake in sizing it, not a state to handle.
6. **Handles.** A handle is `slot | generation << 16`, and 0 is never a handle. The side that issued it checks the generation, so a handle kept past its release does not name what took its slot next.
7. **The clock.** During a turn the guest reads the host's clock (`now`) in milliseconds. It never runs backward, and only the difference between two readings means anything.

Everything else is one end's own. The guest's: where its ranges lie, the heap and its allocator, the tick, its passes and tasks, and its files. The host's: how large a quota is, when to ring, and how it reads what a turn left. Neither end may lean on how the other does any of it.

The unit the contract describes is one host and one guest. Several guests under one host — each with its memory, passing buffers between them by ownership rather than sharing them — do not change it, and are a composition it says nothing of yet. Where they share one thread of execution, or do not trust each other, that composition must bring what this contract does not: isolation, and a way to stop a guest that does not return.

---

## Written in several languages

Everything above is implemented in more than one language, and the implementations do not all stand in the same relation.

`kit` is symmetric. Each language implements the same kit, and the golden records hold them to each other wherever they must produce identical bytes. `guest` and `host` have two relations. A host end and a guest end are the two ends of one arrangement, any host facing any guest, and a module with nothing facing it across the boundary is not a gap waiting to be filled: what the two ends must agree on is not their module lists but the contract above, and the formats it travels in — the directory, the message's four words, the ring's layout, the stream's records. Its guest ends in different languages are the same end, any of them plugged into any host, so they are symmetric as `kit` is: what one guest gives a program, every other gives too, in its own language's shape; and so are host ends, should there be more than one.

**Rust** (`rust/`)

- `kit`: `abi` words and the command number · `alloc` slab and buddy · `codec` JSON · `collection` arena, deque, heap · `math` scalar, rng · `status` errno, error, bug · `wire` XTP, TLV · `golden` the record reader.
- `guest`: `mem` sections, the base, the heap door and a heap to install between bounds the guest gives · `module` what a WebAssembly guest reads of its module: where the linker stopped, the memory given, the host's clock · `ipc` directory, frame, ring, stream, sys and the file table, the guest's door · `sched` quantum, tick, pass, task, edge · `abi` commands, handles · `clock` · `global`.

**TypeScript** (`ts/`)

- `kit`: `abi` `codec` `collection` `math` `status` `wire` `golden` as the Rust, plus `lang` (comparators, iterators, types) and `env` (feature detection); no `alloc`, since a TypeScript program has no linear heap of its own, and no `bug`, since a thrown error carries its stack.
- `host`: `mem` the reservation · `ipc` directory, frame, ring, stream, sys, the host's doorbell · `sched` quantum, **strobe** · `abi` commands, handles.

**C++** (`cpp/`)

- `kit`: `abi` `alloc` `codec` `collection` `math` `status` `wire` `golden` as the Rust, and `codec` hex, base64url and FNV, which the Rust takes as crates.
- `guest`: `mem` the memory, sections and the heap door · `ipc` directory, frame, ring, stream, sys and the file table, the guest's door · `sched` quantum, tick, pass, task, edge · `abi` commands, handles · `clock`.

The two layers differ in what they ask of a program. `kit` asks nothing: link it and call it. `guest` and `host` ask a program to run xpute's way — memory granted once, work in turns, I/O by asking.

C++ is an implementation beside the Rust, not a part of it: written from the contract and the formats rather than from the Rust, built by the platform's compiler (`script/cpp.ts`) rather than through Cargo, and held by the same records and the same traces. That is the point of it — what the guests agree on is what the boundary shows, and not what one was copied from. Where it differs, the language is why: it writes the codecs the standard library lacks; its XTP reads a packet without allocating, handing back views of it, and loads an element where it lies rather than viewing it in place; a task is a C++20 coroutine where Rust's is a future, woken through the executor it runs on; and the memory is a value a call names rather than one base per module, so a native host may run several guests in one process. It has no `global`, the one value per module Rust needs a type for and C++ has as a plain static. It is C++ where that says something C would not — RAII, a template, a `Result` where exceptions would be — and plain where it does not, with no exceptions, no RTTI and none of the standard library's heavier surface. Its numbers — the errno table, the directory's keys, a frame's and a ring header's words — come from `spec/` as the other languages' do; how it reads and writes them is its own.

---

## The golden records

Under `spec/golden/` are lines of `path<TAB>value`, produced once, checked once, and frozen. They are read by splitting each line and nothing more (`kit golden`), so the records cannot drift because a parser changed.

There are two kinds, and the difference decides whether a record may ever be re-recorded.

- **Across a protocol** — `wire/xtp`, `wire/tlv`, and `codec/json`'s grammar: which texts are JSON at all, as the host's own `JSON.parse` judges them, so a document a guest reads in place is one the host would have read. Every language that carries one must produce these bytes and judgments, to the last one, and each one's tests read the same files.
- **Inside a language** — everything else. A record holds one implementation to itself. Where Rust and JavaScript's `Math` round differently, neither is wrong, and each record follows its own implementation.

The rule in one line: inside a language, each follows its own; across a protocol, the results must match exactly.

A third kind is not an output at all. The **traces** under `spec/conformance/` say what a host does and what it must then see: the steps, and the completions, stream records and next-turn asks each turn leaves. One host runs them against every guest through the boundary alone (`ts/conformance/src/trace.test.ts`) — the TypeScript host's own ring, directory and stream reader, over a guest of two C calls. The guest is built as a native library booted into a region the host holds, or as a WebAssembly module instantiated over a memory it reserves, and its boot answers where its directory is. They are written from the contract's rules rather than recorded from a guest, and they name only what a host can observe: a guest's sizes, and how it keeps its output in bounds, are its own. So they hold every guest to the same thing, whatever language it is written in. They run against each guest there is, the Rust one (`conformance guest.rs`) and the C++ one (`cpp/conformance/guest.cpp`), each over the commands `spec/conformance/guest.json` numbers and each laying its memory out its own way. Each is built both ways, and the module imports nothing but its memory and the clock.
