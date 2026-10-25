# A slot and its payload: what following the ring's own precedents cost

**2026-09-17** — [The ring and its precedents](./archive/20260914-the-ring-and-its-precedents.md)
was written three days ago and already said what the ring should be. Under
io_uring it says the closest precedent is "the split between a fixed entry and
a separately held payload", and under the questions already answered:
"registered buffers instead of copies … the packet carries the address, not
the bytes." The ring did not do either. This records what doing them meant in
bytes, and the two things the earlier document did not reach: where the rings
sit, and what happens when one fills.

Nothing here is new prior art. It is the prior art on file, applied.

> Since resized: the numbers below are that day's. The rings' depths, the
> addresses and the map have changed since, and
> `spec/client/kernel/sys/mem/mem.json` holds the current ones, with the map
> in ARCHITECTURE.md's "The memory map". The shape this records, a slot and
> its payload, still holds.

## What was wrong, three faces of one thing

A ring was one block: a header, `capacity` messages, and a bump area for the
packets, all in 1 MiB taken from the heap.

**The shared area was not a ring.** Its cursor rewound only when the producer found
the ring empty:

```rust
if w[2] == w[1] { w[4] = 0; }
```

Consuming a message returned no space, so what the area had to hold was not
what was in flight but everything pushed between two empty moments. And
`push_in_place` handed the writer whatever was left of it, so the same packet
went through or did not depending on what had been sent before it — a full
area answered EAGAIN, an empty one EMSGSIZE, and neither answer was about the
packet.

**Two failure modes, disagreeing.** Depth (4,096) and payload area (983,008 B) were
two numbers that did not know about each other. Which ran out first turned on
the average packet: `983,008 / 4,096 = 240 B`. Under that the depth bound, over
it the area, and nothing in the code said so.

**The addresses moved.** `ef2d6e2` had the rings take their blocks from the
heap, so they landed wherever the allocator happened to put them — which is to
say wherever the code above them last changed.

## A slot and its payload

Message `n` takes slot `n & (capacity - 1)` and writes into that slot's
payload. A slot holds one live message, so its payload is free exactly when
the slot is; `entry_at` and `slot_at` answer the same `position`.

That is AF_XDP's correspondence — a descriptor and the buffer it names, the
UMEM cut into `NUM_FRAMES` chunks of `FRAME_SIZE` with one ring entry each.
There is no cursor, no rewind, and no moment the producer has to wait for.

**The word is not AF_XDP's.** It calls a chunk a _frame_, because in its trade
one holds one Ethernet frame; here one holds an XTP packet, and this tree
already spends `frame` on a message frame (`ipc/frame.rs`), a rendered frame
and a coordinate frame. A fourth would have been three too many. A slot is
what the descriptor already numbered, so the payload is the slot's and needs
no name of its own.

What it buys is **one failure mode**. A push fails when the ring is full and
for no other reason. A packet past a slot is EMSGSIZE whatever the ring holds,
so what fits is a property of the packet and is settled where it is written
rather than where it is pushed.

## A slot in two places

The descriptors are 36 KiB a ring and the payloads are 8 MiB (the submission
ring's; the completion ring's are now 68 KiB and 16 MiB), so keeping them in
one block made the small one as coarse as the large one. virtio keeps its
descriptor table, avail ring and used ring in three separate areas; AF_XDP
keeps its rings in one mapping and its UMEM in another; NVMe programs its
queue base addresses and its data goes by PRP/SGL. The ratio is the tell:

```
AF_XDP      rings 32–64 KiB  :  UMEM 8 MiB        1 : 128
here, was   65,536 B         :  983,008 B         1 : 15
here, now   36 KiB           :  8 MiB             1 : 227
```

The header carries where a ring's payloads begin (word 4), so a `Ring` is
still made from one address.

## Sizing, from the references rather than from the traffic

What these systems actually reserve:

|                                        | descriptors           | data                                           |
| -------------------------------------- | --------------------- | ---------------------------------------------- |
| AF_XDP (xdpsock default)               | 32–64 KiB             | **UMEM 8 MiB** — 2 KiB × 4,096                 |
| DPDK (per port)                        | pointers only         | **~18 MiB** — 8,191 mbufs × (2 KiB + headroom) |
| io_uring (`IORING_MAX_ENTRIES` 32,768) | SQ 2 MiB + CQ 1 MiB   | registered buffers, separate                   |
| virtio-net                             | 4–16 KiB              | skb/mbuf pool, separate                        |
| NVMe (depth 1,024)                     | SQ 64 KiB + CQ 16 KiB | host DMA buffers, separate                     |

The descriptor ring is kilobytes everywhere and the memory goes into the data
area, at MiB scale. 1 MiB a ring, payloads included, was small on both counts at
once.

**A slot's payload is 4 KiB.** AF_XDP's sample and DPDK's mbuf both use 2 KiB,
and `XSK_UMEM__DEFAULT_FRAME_SIZE` is 4,096. What settles it here is not
either of those: 4 KiB is the page this kernel already counts in
([the page table the pool did not need](./archive/20260916-the-page-table-the-pool-did-not-need.md))
and the buddy's smallest block
([the slab over the buddy](./archive/20260917-the-slab-over-the-buddy.md)), so a payload
is one block wherever one has to come from, and "a packet is at most a page,
and past that it is bulk" is a sentence that can be said without looking at any
command.

_Superseded: there are two rings and no spill, and the completion ring is
twice the submission's — `SUBMISSION_CAPACITY` 2,048, `COMPLETION_CAPACITY`
4,096 (spec/client/kernel/sys/mem/mem.json), since a turn answers what it was sent
and raises its own signals besides._

**The depth is 2,048 and the three are equal.** io_uring's default is CQ twice
SQ, and its own manual page says why that is not a universal ratio — "adequate
for regular file or storage workloads, but may be too small for networked
workloads". A ratio here would be a claim about this traffic, and this traffic
is moving: the renderer came into the module, so what used to go out as draw
lists no longer crosses, and what the shell still sends is on its way in behind
it. Equal is the choice that makes no claim.

`2,048 × 4 KiB = 8 MiB` a ring, which is xdpsock's UMEM exactly, by two numbers
neither of which was chosen to land there. (The completion ring's payloads
are now `4,096 × 4 KiB = 16 MiB`.)

## What was considered and not taken

- **A bip buffer.** Simon Cooke's two-region circular buffer is the standard
  answer to variable-length records in a ring, and it returns space as the
  consumer advances. It buys nothing here: consumption is turn-based, so no
  consumer interleaves inside a turn and there is no space for it to hand
  back. The turn boundary is already the reset point.
- **A shared buffer pool, split adaptively.** AF_XDP's UMEM _is_ shared between
  RX and TX, so this is the conventional adaptive answer. Two things stop it.
  The free list would have two pushers and two poppers, and splitting it back
  into two SPSC lists fixes the division again; and AF_XDP does not balance
  either — the application moves buffers, so the split stays a decision
  somebody makes. Against that, the thing being optimized is free: an
  untouched page of linear memory costs address space and not RAM, so
  over-provisioning both sides dominates.
- **Runtime resize.** `io_uring_resize_rings()` exists. It would cost the one
  property the addresses were made constant for.
- **Keeping one shared area and sizing it against the traffic.** Measuring which
  packets are big and cutting those is how the 240 B break-even happened in
  the first place.

## Where they sit, and why the linker has to be told

Constant addresses, because a block of the heap moves whenever the code above
it changes, and the address is published to the shell at boot the way virtio
writes its queue addresses into device registers and AF_XDP registers its
UMEM. A published address that moves per build is a discovered address wearing
a constant's clothes.

The linker owns the bottom of the memory, so it has to be kept out of the
range. `--global-base` is how it is told where its own data may begin, and the
same arithmetic has to produce that flag and the addresses the code uses —
which is why both come out of one file of numbers,
`spec/client/kernel/sys/mem/mem.json`: its generated `sys/mem/mem.spec.rs` is what
`crates/client-bracket/wasm/build.rs` emits as the link argument and what
`crates/client/kernel/src/door/layout.rs` lays the rings from. There is no
second place to get them wrong.

The map as the code now holds it (mem.json, door/layout.rs; a descriptor
range is `ring_bytes` rounded up to a page):

```
addr                          bytes          what
────────────────────────────────────────────────────────────────────────
0            1,048,576          1.00 MiB     stack, descending  STACK_BYTES
1,048,576    1,085,440         36 KiB        sq descriptors     RING_DESC_AT
1,085,440    1,155,072         68 KiB        cq descriptors
2,097,152   10,485,760          8 MiB        sq payloads  2,048 × 4 KiB  RING_SLOT_AT
10,485,760  27,262,976         16 MiB        cq payloads  4,096 × 4 KiB
33,550,336  33,554,432          4 KiB        the directory      DIRECTORY_AT
────────────────────────────────────────────────────────────────────────
33,554,432                                   .rodata .data .bss   ← --global-base
────────────────────────────────────────────────────────────────────────
page past the data                           .fixed — the terrain and the rest
past .fixed                    end           heap
```

Two ranges that grow without moving each other: the rings have everything
under 32 MiB and the module's data everything over it. `.fixed` begins at the
first page past the module's data (pxl-kernel sys/mem/section.rs `fixed_at`) and
the heap past it.

**The slack under the directory, about 6 MiB, costs address space, not RAM.**
Linear memory is backed by the OS on first touch, so an untouched gap is not
resident. The exception this repository has already met is accounting rather
than residency: JavaScriptCore reports the whole reservation as the page's
JavaScript memory — a Safari heap timeline read it as one `ArrayBuffer` —
which is why a phone is given 512 MiB and not 1 GiB (mem.json `PHONE_BYTES`,
`apps/client/src/abi/boot.ts`). The reservation did not change here, so neither does
that.

**The stack stays at the bottom, and that is the only guard there is.** rustc
passes `--stack-first` and `-z stack-size=1048576` to wasm-ld for
wasm32-unknown-unknown by default, and build.rs now passes both itself from
`STACK_BYTES`; the linked module's `__stack_pointer`
initializes to 1,048,576. With the stack first, an overflow goes below zero,
wraps as an unsigned 32-bit address, lands outside the memory and traps. With
the default layout it would write over `.data` silently instead, which is the
reason Rust and Zig both pass the flag and why LLVM has an open issue to make
it wasm-ld's default. There is no MMU here and so no guard page: the
wraparound is the whole of the protection, and moving the stack anywhere else
gives it up.

## What crosses by address now

_Superseded in its particulars: of the commands below only `WORLD_SNAPSHOT`
is left, a block the kernel holds named by address and length
(spec/client/kernel/domain/control/cmd.json); `FETCH_BODY`, `VOLUME_READ`,
`OBJECT_COLUMNS`, `LOAD_UPDATE` and `REPLY_BYTES` are gone, and a reply is the
completion's result word (xpute-guest ipc/door.rs `Door::drain`)._

`FETCH_BODY` already did it — the shell asks for room, writes the body, and
names the block — so terrain never touched a slot. The rest now matches:

|                  | before                           | now                                 |
| ---------------- | -------------------------------- | ----------------------------------- |
| `VOLUME_READ`    | band rows copied into the packet | a block the kernel holds, named     |
| `OBJECT_COLUMNS` | keys copied into the packet      | the same                            |
| `WORLD_SNAPSHOT` | the UI's JSON as a string        | the same                            |
| `LOAD_UPDATE`    | a model's bytes inline           | a block the shell asks the heap for |

The reply scratch is one slot, so a reply too large is refused where it is
written rather than where it is pushed. `REPLY_BYTES` was 256 KiB — a single
constant that alone decided how big the shared area had to be.

What stays inline is what a slot is for: the signals, the chunk paint's
1,536 bytes, the URLs, the zone and instance and keyframe lanes the kernel
reads in place without copying. Those last are the slot doing UMEM's job
already, and moving them to blocks would add a round trip to remove a copy
that is not happening.

## A full ring is a wait, not a panic

_Superseded: the spill is gone. The host reads the completion ring dry before
it rings again, so overrunning it is a sizing bug (`Door::post` answers
ENOSPC), and `drain` stops before a command when the completion ring has no
room for what one command may leave, its signals and its reply (xpute-guest
ipc/door.rs `Door::has_room`)._

`Outbox::post` tried the completion ring, then the spill, and asserted. The
spill is io_uring's backlog — "never dropping events but providing backpressure
on submits" — but io_uring has the second half too: a submission is refused
`-EBUSY` while the backlog stands, and the application must reap completions
and try again. There was no second half here.

There is now: `drain` stops before applying a command when the spill has no
room for what it might raise, and what is left stays in the submission ring in
order. The submission side already answered EAGAIN, so both directions wait.

io_uring needs a backlog at all because completions arrive from hardware and
cannot be refused mid-operation. Here everything inside a drain is synchronous,
so the stop is enough — the spill remains for the tick, which emits as the
builder walks and has no equally clean place to stop.

## What this leaves open

_`MOST_SIGNALS` is gone: the kernel's bound is `SIGNALS` (pxl-kernel door.rs),
what a turn raises after its drain._

- `MOST_SIGNALS` is 64: what one command may raise before `drain` checks again.
  It bounds a command, not a turn, because a tick is not a submission.
- Nothing was run. This is `cargo clippy -D warnings` and `deno task check`,
  both green, and no more than that.
