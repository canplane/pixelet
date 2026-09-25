# Architecture — xpute

[xpute](./README.xpute.md) runs one program inside another. The outer one, the **host**, owns the machine: the clock, the memory, the I/O. The inner one, the **guest**, computes and owns nothing. It cannot take memory from the system, cannot open a connection, and cannot decide when it runs. Everything it has, the host gave it, and the host gives each thing a slice at a time — the memory once at boot, a quota of time on every turn, credits for the I/O it asks for.

That is the arrangement an operating system has with a process, without the hardware that enforces it: nothing stops a guest from spending a whole quota inside one function.

xpute is that arrangement, in two layers. `core` is generic code — collections, codecs, a wire format, an errno table, a libm — that any program can link and call, host or no host. `runtime` is the arrangement itself: the memory, the messages, the turn, the credits. `runtime` is written against `core`, and `core` knows nothing of `runtime`. Both layers exist twice, once in Rust and once in TypeScript; what that means for each of them is the last section, once the pieces have names.

Neither layer knows what a guest computes. No domain type of any kind reaches them, which is what lets the arrangement carry from the program it was written for to the next one.

Three things cross between them and nothing else does: one memory they share, one call in, one number out. The sections follow that order — the memory, what travels through it, the call that makes any of it run — then credits and handles, and last how all of it exists twice.

---

## One memory, placed once

The two sides share one linear memory and nothing else. The host reserves it at boot and never grows it, so a view taken into that memory stays valid for as long as the guest lives. Everything below rests on that: a message queue can keep its words at a fixed address, and a payload can be read where it lies rather than copied out of the way first.

What goes in the memory is the guest's business; placing it is the runtime's. The guest hands in a base, a unit and a list of sizes, and gets back where each range landed (`mem/section.rs`). A linker script does the same thing with output sections, for the same reason: whatever places ranges should not need to know what any of them hold. A range comes back as a section, and whatever is asked of it — a span to write, or ranges laid inside it — is checked against where it ends (`Section::at`, `nth`), so nothing written through one range lands in the next.

The last range is the heap, and blocks come out of it through a single call (`mem/heap.rs`). What answers that call is the guest's choice. `core` offers two allocators that fit together. The lower one is a buddy allocator, which hands out fixed pages over a given span (`alloc/buddy_tree.rs`, ported from evanw/buddy-malloc). Above it sits a slab allocator holding size classes (`alloc/slab.rs`), so that a small request takes a slot of its own class instead of a whole page. The slab keeps its bookkeeping inside the blocks it has not handed out, so what it costs does not grow with the span it covers.

The buddy is not ours: it was written for this same situation, a module handed one heap with no system allocator beneath it.

One kind of buffer cannot live in that memory at all. Work sent off the thread needs something transferable, and a `WebAssembly.Memory` is not. The host keeps a small pool of fixed-size transferable buffers for that case (`mem/transfer.ts`), made on first need, reused when they come back, and let go once idle. The pool is capped, and the cap is the backpressure: when every buffer is out, whatever wanted one waits.

---

## The message, and the rings that carry it

Sharing a memory does not say what a request looks like. Every request and every answer in xpute is the same four words (`ipc/frame.rs`): who asked (`tag`), what is asked (`cmd` in the low 16 bits, flags in the high 16), what came back (`result`, a value or `-errno`), and where the packet is. A command number is two bytes, `major << 8 | minor` (`core abi/cmd`).

Two of the flags in that high half decide what a message means. `ACKREQ` is set by whoever submits, and asks for an answer. `RES` is set by whoever completes, and says the message is an answer — to a call that asked for one, or to a call that failed. A completion without `RES` answers nothing: the guest raised it on its own, and whoever cares about it listens for it.

Four words are small, and a request's data usually is not. So the words go in one place and the data in another. What carries them is a **ring** (`ipc/ring.rs`): the four-word descriptors in one place, and the packets they point at in a separate range of the memory.

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

## Streams, for what the host must do before the next turn

A message on the completion ring waits until the host reads it, and the guest cannot tell when that is. Some requests cannot wait like that: a call the host has to make before anything else reaches a device, or one whose result the guest will read on its next turn. For those the guest writes a **stream** (`ipc/stream.rs`): records laid end to end in a range of the memory during a turn, which the host runs, all of them, when the turn returns and before it rings again.

```text
0       END   how many words of records the turn wrote
1…            the rest of the head, the user's
head…   op · n · n words   op · n · n words   …
```

A record is an op the user numbers, a count, and that many words. Because the whole stream runs between two turns, a record can name memory for the host to write into, and the guest reads it on its next turn without being told it is there: the order is the acknowledgment. What a stream costs is its range, which bounds what one turn can ask for; what a full stream means is the user's to decide.

---

## The packet

The address in a message points at a packet, and every packet on the rings has the same shape. That shape is **XTP**, the Xpute Tree Packet (`core wire/xtp`), and one decision explains the rest of it: nothing inside a packet records where the packet is. Every offset is counted from the branch that holds it, so the same bytes mean the same thing at any address.

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

The format in full, and the API that writes it, is [the note beside the TypeScript implementation](./packages/xpute/core/src/wire/xtp/README.md); the Rust mirrors it.

**TLV** (`core wire/tlv`) is the other format, flat and self-delimiting, for arguments that arrive as a stream rather than a tree. Both live in `core`, because neither knows anything about the program using it.

---

## Changes, for what is only read

Not everything that crosses deserves a message. A packet describing state the other side only reads would be paying twice, since the reader goes and reads that state anyway. So the read direction is not a message but three numbers. A **change** (`ipc/change.rs`) is: a topic (`u16`, a kind of state the guest numbers), a key (`u64`, the one unit within that topic), and a revision (`u32`). A writer calls `bump`. The topic's counter advances, and its new value becomes the key's revision, written in two places: a table indexed by key, and one fixed-size journal ring.

The two places exist because readers come in two kinds. A reader that cares about a handful of keys compares each key's revision against the one it remembers. A reader that cares about everything walks the journal forward from its own cursor — and if the journal has lapped that cursor, it gives up on the journal and re-reads the keys of the topics it follows.

A change carries no value; the value is read where it lives. How a reader is woken — by polling on each turn, by a listener (`ipc/change_waker`), by a React hook — is built on top of this and is not part of it.

---

## The turn

The guest does not run continuously. It runs inside one call, `interrupt(quota_ms)`, and when that call returns the guest is not running at all.

That call has two edges, like a clock's. On the rising edge the host calls in with a quota. The guest drains the submission ring, spends the quota on its own work, and posts what it has to the completion ring. On the falling edge it returns one number, `wake_ms`, which is when it would like the next turn: `0` for the next frame, `n` for a time, `NO_WAKE` for none. The host learns nothing else about what ran.

How long a turn may take, and how that time is spent inside it, are separate problems. They are settled at three levels, from the outside in.

1. **Quantum** (`sched/quantum`) is the host's, and it decides the quota itself. The size comes from a frame interval the host measures rather than assumes. The shape comes from the guest, which declares a policy the host applies: a margin share for interactive turns, a number of frames for batch turns, a settle time after input, and what the falling edge is allowed to answer with. A turn that runs long is not cut off; the overrun is subtracted from the next quota, down to nothing.
2. **Tick** (`sched/tick`) is the guest's, and it divides the quota. The guest runs its phases in a fixed order against one budget, counted from the rising edge. Which phases exist and what share each gets is the guest's; the tick knows the quota and the clock and nothing else. Nothing enforces the quota — a phase that runs past its share is not stopped, it is simply counted against the turn.
3. **Pass** (`sched/frame_budget`) is one phase's share spent on one ordered list of steps, until a wall-time budget or a step cap says stop. It is not a registry — what is eligible, and in what order, is the caller's. What it does guarantee is forward progress: the first step that has work always runs, so nothing can be starved by whatever sits ahead of it in the list.

Work too long for one turn is a **task**: a plain future written against `sched/edge`. That module hands a task the turn's quota as a deadline on the host clock, plus `yield_if_spent` to stop when the quota is gone and `next_turn` to park until the next turn begins. Nothing in that interface names an executor. Whatever module assembles the guest picks one, marks the two edges by calling `rise` and `fall`, and polls the executor between them.

```text
host · TypeScript                          guest · Rust
─────────────────                          ────────────

strobe   a frame, and only when one
         was asked for
quantum  a quota, from the frame it
         measured rather than assumed

commands ─────── submission ring ───────▶  drained on the rising edge
doorbell ═════ interrupt(quota_ms) ═════▶  ┌ tick   phases in a fixed order
                                           │ tasks  futures that yield when
                                           │        the quota is spent
                                           │ queue  what I/O it wants run
completions ◀───── completion ring ──────  └ posted before the falling edge
         ◀═══════════ wake_ms ═══════════  next frame · in n ms · never

credits  runs what that queue started,
         as far as the grant goes

         … nothing until wake_ms …         … nothing at all …
```

On the host side, something has to decide when to ring at all. That is the **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only when something asked for a turn. Two things can ask — the host, when input arrives or a fetch lands, and the guest, on its last falling edge. A guest that asks for nothing is rung for nothing, and no turn ever runs on a timer of the host's own.

---

## I/O by credits

The guest cannot do I/O, because it does not own the connections; the host does, the way an operating system owns its devices. What the host grants it instead is a number: this turn's **credits**, how many reads may be out at once. Which reads those are is the guest's to say.

That the guest says which is the whole of the split. Ordering one demand against another needs to know what the program is for — what is on screen, what the camera is about to want, what a failure means — and none of that is a runtime's to hold. A queue for it ran here once and went for that reason (`io.rs`).

That leaves two budgets, and they never touch. A tick spends a quota of time on the guest's own work. The credits spend a grant of concurrency on work the host does for the guest.

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

A table is fixed at N slots and never grows. Slot 0 is never handed out, so 0 is never a valid handle and can be used for "none".

Handles are in `runtime` rather than `core` because a handle is one side of a boundary, not a container.

---

## Written twice

Everything above is implemented in both Rust and TypeScript, and the two implementations are not the same kind of pair.

`core` is symmetric. Each language implements the same kit, and the golden records hold the two to each other wherever they must produce identical bytes. `runtime` is not symmetric and is not meant to be: its two halves are the two ends of one arrangement. The host end is TypeScript and the guest end is Rust. A module with nothing facing it across the boundary is not a gap waiting to be filled. What the two ends must agree on is not their module lists but their formats: the message's four words, the ring's layout, the memory's sections.

|             | Rust (`crates/xpute/`)                                                                                                                                                                                        | TypeScript (`packages/xpute/`)                                                                                                                                                      |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **core**    | `abi` words and the command number · `alloc` slab and buddy · `codec` JSON · `collection` arena, deque, heap · `math` scalar, rng · `status` errno, error, bug · `wire` XTP, TLV · `golden` the record reader | `abi` `codec` `collection` `math` `status` `wire` as the Rust, plus `kit` (comparators, iterators, types) and `env` (feature detection)                                             |
| **runtime** | `mem` sections and the heap door · `ipc` frame, ring, stream, change log · `sched` quantum, tick, pass, task, edge · `abi` handles, fetch · `clock` · `global`                                                | `mem` the reservation and the transferable buffers · `ipc` frame, ring, stream, change log, **doorbell** · `sched` quantum, tick, pass, **strobe** · `abi` handles, fetch · `clock` |

The two rows differ in what they ask of a program. `core` asks nothing: link it and call it. `runtime` asks a program to run xpute's way — memory granted once, work in turns, I/O by credits.

---

## The golden records

Under each crate's `golden/` are lines of `path<TAB>value`, produced once, checked once, and frozen. They are read by splitting each line and nothing more (`core golden.rs`), so the records cannot drift because a parser changed.

There are two kinds, and the difference decides whether a record may ever be re-recorded.

- **Across a protocol** — `wire/xtp`, `wire/tlv`. Both languages must produce these bytes, to the last one, and tests on both sides read the same files.
- **Inside a language** — everything else. A record holds one implementation to itself. Where Rust's `libm` and JavaScript's `Math` round differently, neither is wrong, and each record follows its own implementation.

The rule in one line: inside a language, each follows its own; across a protocol, the results must match exactly.
