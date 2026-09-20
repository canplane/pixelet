# Architecture — xpute

xpute runs one program inside another. The outer one, the **host**, owns the machine: the clock, the memory, the I/O. The inner one, the **guest**, computes and owns nothing. It cannot take memory from the system, cannot open a connection, and cannot decide when it runs. Everything it has, the host gave it, and the host gives each thing a slice at a time — the memory once at boot, a quota of time on every turn, credits for the I/O it asks for.

That is the arrangement an operating system has with a process, without the hardware that enforces it: nothing stops a guest from spending a whole quota inside one function.

xpute is that arrangement, in two layers. `core` is generic code — collections, codecs, a wire format, an errno table, a libm — that any program can link and call, host or no host. `runtime` is the arrangement itself: the memory, the messages, the turn, the credits. `runtime` is written against `core`, and `core` knows nothing of `runtime`. Both layers exist twice, once in Rust and once in TypeScript; what that means for each of them is the last section, once the pieces have names.

Neither layer knows what a guest computes. No domain type of any kind reaches them, which is what lets the arrangement carry from the program it was written for to the next one.

---

## One memory, placed once

The two sides share one linear memory and nothing else. The host reserves it at boot and never grows it, so a view taken into that memory stays valid for as long as the guest lives. Everything below rests on that: a message queue can keep its words at a fixed address, and a payload can be read where it lies rather than copied out of the way first.

What goes in the memory is the guest's business; placing it is the runtime's. The guest hands in a base, a unit and a list of sizes, and gets back where each range landed (`mem/section.rs`). A linker script does the same thing with output sections, for the same reason: whatever places ranges should not need to know what any of them hold.

The last range is the heap, and blocks come out of it through a single call (`mem/heap.rs`). What answers that call is the guest's choice. `core` offers two allocators that fit together. The lower one is a buddy allocator, which hands out fixed pages over a given span (`alloc/buddy_tree.rs`, ported from evanw/buddy-malloc). Above it sits a slab allocator holding size classes (`alloc/slab.rs`), so that a small request takes a slot of its own class instead of a whole page. The slab keeps its bookkeeping inside the blocks it has not handed out, so what it costs does not grow with the span it covers.

One kind of buffer cannot live in that memory at all. Work sent off the thread needs something transferable, and a `WebAssembly.Memory` is not. The host keeps a small pool of fixed-size transferable buffers for that case (`mem/transfer.ts`), made on first need and reused when they come back. The pool is capped, and the cap is the backpressure: when every buffer is out, whatever wanted one waits.

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

A ring has one producer and one consumer, and that is what lets it run without a lock. The producer writes the tail and its own payloads; the consumer writes the head; neither writes what the other writes. Positions only ever grow, so `tail - head` is how many messages are waiting. Message _n_ owns slot _n_, which means a slot is free exactly when its message has been consumed — there is no allocator under the ring and nothing to free.

A push can fail in two ways and no others. `EAGAIN` means the ring is full: the producer waits, and nothing is dropped. `EMSGSIZE` means the packet is bigger than a slot, and a packet that big crosses by address instead, in a heap block the guest hands out and the host writes into.

Two rings make a pair: one for what the host submits, one for what the guest completes. Driving that pair from the host end is the **doorbell** (`ipc/doorbell.ts`). It pushes the commands, rings the guest with a quota, and then reads the completion ring dry, handing each message either to the call that was waiting for it or to whoever listens for signals. One ring of the bell is one whole turn of the guest.

---

## The packet

The address in a message points at a packet, and every packet on the rings has the same shape. That shape is **XTP**, the Xpute Tree Packet (`core wire/xtp`): a tree of scalar leaves, typed-array sequences and branch nodes, laid out on 8-byte words, little-endian, with every offset written relative to where it sits rather than absolute.

Relative offsets buy two things. A packet can be moved, or spliced into a larger one, without rewriting a single pointer inside it. And a typed-array leaf can be handed out as a view straight into the packet, with no copy, because nothing in the leaf depends on where the packet ended up.

Two more properties come from the layout itself. A subtree encoded earlier grafts in with the same layout an inline node would have, and an optional node that is absent still carries its type.

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
2. **Tick** (`sched/tick`) is the guest's, and it divides the quota. The guest runs its phases in a fixed order against one budget, counted from the rising edge. Which phases exist and what share each gets is the guest's; the tick knows the quota and the clock and nothing else. A quota is how long a turn should take, not a deadline anything enforces. Nothing here is real time.
3. **Pass** (`sched/frame_budget`) is one phase's share spent on one ordered list of steps, until a wall-time budget or a step cap says stop. It is not a registry — what is eligible, and in what order, is the caller's. What it does guarantee is forward progress: the first step that has work always runs, so nothing can be starved by whatever sits ahead of it in the list.

Work too long for one turn is a **task**: a plain future written against `sched/edge`. That module hands a task the turn's quota as a deadline on the host clock, plus `yield_if_spent` to stop when the quota is gone and `next_turn` to park until the next turn begins. Nothing in that interface names an executor. Whatever module assembles the guest picks one, marks the two edges by calling `rise` and `fall`, and polls the executor between them.

On the host side, something has to decide when to ring at all. That is the **strobe** (`sched/strobe.ts`): one turn on an animation frame, and only when something asked for a turn. Two things can ask — the host, when input arrives or a fetch lands, and the guest, on its last falling edge. A guest that asks for nothing is rung for nothing, and no turn ever runs on a timer of the host's own.

---

## I/O by credits

The guest cannot do I/O, because it does not own the connections; the host does, the way an operating system owns its devices. What the guest can do is say what it wants and in what order. It keeps a queue of requests (`io/queue.rs`), and the host starts what is at the front of that queue — as far as the credits it granted for this turn will reach.

That leaves two budgets, and they never touch. A tick spends a quota of time on the guest's own work. The queue spends a grant of credits on work the host does for the guest.

---

## Handles

The two sides need to talk about the same object without either one being able to hold the other's pointers. So one side keeps the object and hands out a number for it, exactly as an operating system keeps a file and hands out a descriptor. That number is a **handle** (`abi/handle`), and it is two fields in one word: `slot | generation << 16`.

The side holding the handle reads the slot out of it and nothing more. Only the table that issued the handle looks at the generation, so a handle kept past its release names a generation the slot no longer has and cannot name whatever took the slot next. A table is fixed at N slots and never grows. Slot 0 is never handed out, so 0 is never a valid handle and can be used for "none".

Handles are in `runtime` rather than `core` because a handle is one side of a boundary, not a container.

---

## Written twice

Everything above is implemented in both Rust and TypeScript, and the two implementations are not the same kind of pair.

`core` is symmetric. Each language implements the same kit, and recorded output holds the two to each other wherever they must produce identical bytes. `runtime` is not symmetric and is not meant to be: its two halves are the two ends of one arrangement. The host end is TypeScript — the doorbell, the strobe, the reservation of the memory — and the guest end is Rust — the executor, the tick, the tasks. A module with nothing facing it across the boundary is not a gap waiting to be filled. What the two ends must agree on is not their module lists but their formats: the frame's four words, the ring's layout, the memory's sections.

|             | Rust (`crates/xpute/`)                                                                                                                                                                                                    | TypeScript (`packages/xpute/`)                                                                                                                                                  |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **core**    | `abi` words and the command number · `alloc` slab and buddy · `codec` JSON · `collection` arena, deque, heap · `math` scalar, vec3, mat4, rng · `status` errno, error, log · `wire` XTP, TLV · `golden` the record reader | `abi` `codec` `collection` `math` `status` `wire` as the Rust, plus `kit` (comparators, iterators, types) and `env` (feature detection)                                         |
| **runtime** | `mem` sections and the heap door · `ipc` frame, ring, change log · `sched` quantum, tick, pass, task, edge · `io` the credit queue · `abi` handles, fetch · `clock` · `global`                                            | `mem` the reservation and the transferable buffers · `ipc` frame, ring, change log, **doorbell** · `sched` quantum, tick, pass, io, **strobe** · `abi` handles, fetch · `clock` |

The two rows differ in what they ask of a program. `core` asks nothing: link it and call it. `runtime` asks a program to run xpute's way — memory granted once, work in turns, I/O by credits.

---

## The golden records

What holds the two implementations to each other is a set of recorded outputs. Under each crate's `golden/` are lines of `path<TAB>value`, produced once, checked once, and frozen. They are read by splitting each line and nothing more (`core golden.rs`), so the records cannot drift because a parser changed.

There are two kinds, and the difference decides whether a record may ever be re-recorded.

- **Across a protocol** — `wire/xtp`, `wire/tlv`. Both languages must produce these bytes, to the last one, and tests on both sides read the same files.
- **Inside a language** — everything else. A record holds one implementation to itself. Where Rust's `libm` and JavaScript's `Math` round differently, neither is wrong, and each record follows its own implementation.

The rule in one line: inside a language, each follows its own; across a protocol, the results must match exactly.
