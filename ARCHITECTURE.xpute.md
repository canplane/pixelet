# Architecture — xpute

xpute is the system pixelet runs on: a kit of generic code, and a runtime that runs a guest program in turns inside a host. The host owns the clock, the memory and the I/O, the way an operating system owns the machine; the guest is given one memory at boot, a quota of time on each turn, and credits for the I/O it wants. Nothing in xpute knows what the guest does — no coordinates, no terrain, no tiles — and that is what keeps it portable to another program.

It is written twice, in Rust and TypeScript, and the two halves are not the same kind of pair:

- **`core` is symmetric.** Both languages implement the same kit, and golden records hold them to each other where bytes cross.
- **`runtime` is two ends of one arrangement.** The host end is TypeScript — the doorbell (the one call into the guest), the strobe (the clock that decides when to make it), the reservation (the memory handed over at boot). The guest end is Rust — the executor, the tick, the tasks. A module that exists on one side only is not a gap on the other. What both must agree on is the format: the frame's four words, the ring's layout, the memory's sections.

[ARCHITECTURE.pixelet.md](./ARCHITECTURE.pixelet.md) is what pixelet builds over this.

---

## What is in it

|             | Rust (`crates/xpute/`)                                                                                                                                                                                                    | TypeScript (`packages/xpute/`)                                                                                                                                                                  |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **core**    | `abi` words and the command number · `alloc` slab and buddy · `codec` JSON · `collection` arena, deque, heap · `math` scalar, vec3, mat4, rng · `status` errno, error, log · `wire` XTP, TLV · `golden` the record reader | `abi` `codec` `collection` `math` `status` `wire` as the Rust, plus `kit` (comparators, iterators, types) and `env` (feature detection)                                                         |
| **runtime** | `mem` sections and the heap door · `ipc` frame, ring, change log · `sched` quantum, tick, pass, task, edge · `io` the credit queue · `abi` handles, fetch · `clock` · `global`                                            | `mem` the reservation and the little memory (transferable buffers) · `ipc` frame, ring, change log, **doorbell** · `sched` quantum, tick, pass, io, **strobe** · `abi` handles, fetch · `clock` |

The kit is taken as it is. The runtime asks a program to run its way: memory granted once, work in turns, I/O by credits.

---

## The memory

A guest gets one linear memory, sized once and never grown. The runtime lays it out as ranges end to end, the way a linker script places output sections (`mem/section.rs`): it is told a base, a unit and a list of sizes, and answers where each range landed. Which ranges there are and what they hold is the guest's map, not the runtime's.

Heap blocks come and go through one door (`mem/heap.rs`); the allocator behind it is the guest's choice. The kit offers two that fit together: a **buddy** of fixed pages over a given span (`alloc/buddy_tree.rs`, evanw/buddy-malloc ported), and a **slab** of size classes over it (`alloc/slab.rs`) so that a small request takes a slot of its class rather than a whole page. The slab keeps its bookkeeping inside the free blocks, so nothing grows with the span.

On the host side, `mem/transfer.ts` is the little memory: fixed-size transferable buffers for work that must leave the thread, since a `WebAssembly.Memory` cannot. They are made when first needed, reused when they come back, and capped — the cap is the backpressure.

---

## The frame and the rings

Every boundary carries the same four words (`ipc/frame.rs`): who asked (`tag`), what is asked (`cmd` in the low 16 bits, flags in the high 16), what came back (`result`, a value or `-errno`), and where the packet is. A command number is `major << 8 | minor` (`core abi/cmd`). Two flags matter: `ACKREQ` on a submission asks for a reply, and `RES` on a completion marks one — a reply to a call that asked, or to one that failed. Everything else a guest posts is a signal it raised on its own.

A **ring** (`ipc/ring.rs`) lays frames out as descriptors, with the payloads in a separate range:

```text
descriptors                                   payloads, elsewhere in memory
┌──────────────────────────────────────────┐  ┌────────┬────────┬───┬──────────┐
│ header: capacity · head · tail ·         │  │ slot 0 │ slot 1 │ … │ slot n-1 │
│         slot_bytes · slot_base · 3 rsv   │  └────▲───┴────────┴───┴──────────┘
├──────────────────────────────────────────┤       │
│ message n: tag                           │       │
│            cmd (low 16) | flags (high 16)│       │
│            result (value or -errno)      │       │
│            packet address ───────────────┼───────┘  (slot n & (capacity-1))
└──────────────────────────────────────────┘
```

One producer, one consumer. The producer writes only the tail and its payloads, the consumer only the head, so there is no lock; positions only grow, and `tail - head` is how many wait. Message _n_ owns slot _n_, so a slot is free exactly when its message has been consumed and no allocator sits under the ring. A push fails in two ways only: `EAGAIN` when the ring is full — the producer waits, nothing is dropped — and `EMSGSIZE` when the packet exceeds a slot. Anything larger crosses by address, in a heap block the guest hands out. Descriptors apart from payloads is virtio's descriptor table and AF_XDP's UMEM: the descriptors are kilobytes, the payloads megabytes.

The host's end is the **doorbell** (`ipc/doorbell.ts`): commands pushed into the submission ring, the guest rung with a quota, the completion ring read dry — replies to the calls waiting on them, signals to whoever listens. Each call of the bell is one whole turn.

---

## The change log

The read direction is not a message. A change is three numbers (`ipc/change.rs`): a topic (`u16`, a kind of state the guest numbers), a key (`u64`, the unit within it) and a revision (`u32`). A writer calls `bump`; the topic's tick becomes the key's revision, written to a revision table and to one fixed journal ring. A reader either compares a key's revision with the one it remembers, or walks the journal from its cursor — and if the journal has lapped it, re-reads its topics. A change carries no value; the value is read where it lives. How a reader is woken — polled from a tick, a listener (`ipc/change_waker`), a React hook — is built over this and is not part of it.

---

## The turn

The host and the guest alternate like a clock's two edges. On the rising edge the host calls `interrupt(quota_ms)`; the guest drains its submission ring, spends the quota on its own tasks, posts to its completion ring, and on the falling edge returns `wake_ms`: `0` for the next frame, `n` for a time, `NO_WAKE` for nothing. The host knows nothing of what ran inside.

Scheduling is three levels, from the outside in:

1. **Quantum** (`sched/quantum`, the host's). One turn's quota, sized from a frame interval the host measures rather than assumes. A guest declares its policy — a margin share for interactive turns, a number of frames for batch turns, a settle time after input — and what its falling edge answers with; the host applies it. An overrun is subtracted from the guest's next quota, down to nothing.
2. **Tick** (`sched/tick`, the guest's). Inside the quota, the guest runs its phases in a fixed order against one budget, counted from the rising edge. The phases and their shares are the guest's; the tick knows the quota and the clock and nothing of the host. The quota is about how long a turn should take, not a deadline — nothing here is real time.
3. **Pass** (`sched/frame_budget`). One phase's share spent on one ordered list of steps until a wall-time budget or a step cap says stop. It is not a registry: what is eligible and in what order is the caller's policy. What it guarantees is forward progress — the first step that does work always runs, so nothing can be starved by what is ahead of it.

Longer work is **tasks**: plain futures written against `sched/edge` — the rising edge's quota as a deadline on the host clock, `yield_if_spent` and `next_turn` to park until the next rising edge. Nothing in the interface names an executor; the module that assembles the guest picks one and polls it between `rise` and `fall`.

The host's clock for one guest is the **strobe** (`sched/strobe.ts`): a turn on an animation frame, and only when something asked for one — the host for an input or an arrival, or the guest on its last falling edge. A guest that asks for nothing is rung for nothing; no turn runs on a timer of the host's own.

---

## I/O by credits

The guest does no I/O. It keeps a queue (`io/queue.rs`) of what it wants run and in what order, and the host — which owns the connections the way an OS owns its devices — runs what the queue starts, within the credits it grants per turn. A tick spends a quota of time on the guest's own work; the queue spends a grant of credits on what the host runs for it. That is all the two share.

---

## Handles

What one side keeps and the other names is a **handle** (`abi/handle`): `slot | generation << 16`. The table hands out the number and the holder keeps the thing, the way an OS hands out a descriptor and keeps the file. The other side reads the slot off the handle and nothing else; the table alone checks the generation, so a handle kept past its release does not name whatever took the slot after it. A table is fixed at N slots and never grows, slot 0 is never handed out, and 0 is never a handle. It lives in the runtime rather than the kit because it is one side of a boundary, not a container.

---

## XTP and TLV

**XTP** (Xpute Tree Packet, `core wire/xtp`) is the wire format for anything that crosses: a tree of scalar leaves, typed-array sequences and branch nodes, on 8-byte words, little-endian, with every offset relative. A packet can therefore be moved or spliced without rewriting a pointer, and a typed-array leaf is read as a view into the packet without a copy. A pre-encoded subtree grafts in with the same layout as an inline node, and an absent optional node keeps its type. Every packet on the rings is XTP.

**TLV** (`core wire/tlv`) is the flat, self-delimiting form for stream arguments. Both live in `core` because neither knows anything of the program using them.

---

## The golden records

Under each crate's `golden/` are lines of `path<TAB>value`: outputs checked once and frozen, read by a split per line and no parser (`core golden.rs`). They are of two kinds, and only one may be re-recorded:

- **Across a protocol** — `wire/xtp`, `wire/tlv` — both languages must produce these bytes, to the last one, and the records are read by tests on both sides.
- **Inside a language** — everything else — a record holds one implementation to itself. Where Rust's `libm` and JavaScript's `Math` round differently, neither is wrong, and the record follows the implementation.

The rule: inside a language, each follows its own; across a protocol, the results must match exactly.
