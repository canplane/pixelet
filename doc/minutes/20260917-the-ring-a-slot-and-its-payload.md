# A slot and its payload: what following the ring's own precedents cost

**2026-09-17** — kept up to what the code holds.
[The ring and its precedents](./archive/20260914-the-ring-and-its-precedents.md)
was written three days ago and already said what the ring should be. Under
io_uring it says the closest precedent is "the split between a fixed entry and a
separately held payload", and under the questions already answered: "registered
buffers instead of copies … the packet carries the address, not the bytes." The
ring did not do either. This records what doing them meant in bytes, and the two
things the earlier document did not reach: where the rings sit, and what happens
when one fills.

Nothing here is new prior art. It is the prior art on file, applied.

The rings' depths and addresses are `spec/client/kernel/sys/mem/mem.json`'s,
with the map in ARCHITECTURE.md's "The memory map".

## What was wrong, three faces of one thing

A ring was one block: a header, `cap` messages, and a bump area for the packets,
all in 1 MiB taken from the heap.

**The shared area was not a ring.** Its cursor rewound only when the producer
found the ring empty:

```rust
if w[2] == w[1] { w[4] = 0; }
```

Consuming a message returned no space, so what the area had to hold was not what
was in flight but everything pushed between two empty moments. And
`push_in_place` handed the writer whatever was left of it, so the same packet
went through or did not depending on what had been sent before it — a full area
answered EAGAIN, an empty one EMSGSIZE, and neither answer was about the packet.

**Two failure modes, disagreeing.** Depth (4,096) and payload area (983,008 B)
were two numbers that did not know about each other. Which ran out first turned
on the average packet: `983,008 / 4,096 = 240 B`. Under that the depth bound,
over it the area, and nothing in the code said so.

**The addresses moved.** `ef2d6e2` had the rings take their blocks from the
heap, so they landed wherever the allocator happened to put them — which is to
say wherever the code above them last changed.

## A slot and its payload

Message `n` takes slot `n & (cap - 1)` and writes into that slot's payload. A
slot holds one live message, so its payload is free exactly when the slot is;
`entry_at` and `slot_at` answer the same `position`.

That is AF_XDP's correspondence — a descriptor and the buffer it names, the UMEM
cut into `NUM_FRAMES` chunks of `FRAME_SIZE` with one ring entry each. There is
no cursor, no rewind, and no moment the producer has to wait for.

**The word is not AF_XDP's.** It calls a chunk a _frame_, because in its trade
one holds one Ethernet frame; here one holds an XTP packet, and this tree
already spends `frame` on a message frame (`ipc/frame.rs`), a rendered frame and
a coordinate frame. A fourth would have been three too many. A slot is what the
descriptor already numbered, so the payload is the slot's and needs no name of
its own.

What it buys is **one failure mode**. A push fails when the ring is full and for
no other reason. A packet past a slot is EMSGSIZE whatever the ring holds, so
what fits is a property of the packet and is settled where it is written rather
than where it is pushed.

## A slot in two places

The submission ring's descriptors are 12 KiB and its payloads 2 MiB, the
completion ring's 20 KiB and 4 MiB (pxl-kernel door/layout.rs `rings`), so
keeping them in one block made the small one as coarse as the large one. virtio
keeps its descriptor table, avail ring and used ring in three separate areas;
AF_XDP keeps its rings in one mapping and its UMEM in another; NVMe programs its
queue base addresses and its data goes by PRP/SGL. The ratio is the tell:

```
AF_XDP      rings 32–64 KiB  :  UMEM 8 MiB        1 : 128
here, was   65,536 B         :  983,008 B         1 : 15
here, now   12 KiB           :  2 MiB             1 : 170
```

The header carries where a ring's payloads begin (word 4,
xpute/spec/ipc/ring.json `SLOT_BASE`), so a `Ring` is still made from one
address (xpute-guest ipc/ring.rs `Ring::new`).

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
and `XSK_UMEM__DEFAULT_FRAME_SIZE` is 4,096. What settles it here is not either
of those: 4 KiB is the page this kernel already counts in
([the page table the pool did not need](./archive/20260916-the-page-table-the-pool-did-not-need.md))
and the buddy's smallest block
([the slab over the buddy](./archive/20260917-the-slab-over-the-buddy.md)), so a
payload is one block wherever one has to come from, and "a packet is at most a
page, and past that it is bulk" is a sentence that can be said without looking
at any command.

**There are two rings and no spill, and the completion ring is twice the
submission's.** `SQ_CAP` is 512 and `CQ_CAP` 1,024
(spec/client/kernel/sys/mem/mem.json). io_uring's default is CQ twice SQ, and
its own manual page says why that is not a universal ratio — "adequate for
regular file or storage workloads, but may be too small for networked
workloads". The ratio here is not taken from io_uring's and is no claim about
the traffic: a turn answers what it was sent and raises its own signals besides.

`512 × 4 KiB = 2 MiB` of payloads for the submission ring, and
`1,024 × 4 KiB = 4 MiB` for the completion ring.

## What was considered and not taken

- **A bip buffer.** Simon Cooke's two-region circular buffer is the standard
  answer to variable-length records in a ring, and it returns space as the
  consumer advances. It buys nothing here: consumption is turn-based, so no
  consumer interleaves inside a turn and there is no space for it to hand back.
  The turn boundary is already the reset point.
- **A shared buffer pool, split adaptively.** AF_XDP's UMEM _is_ shared between
  RX and TX, so this is the conventional adaptive answer. Two things stop it.
  The free list would have two pushers and two poppers, and splitting it back
  into two SPSC lists fixes the division again; and AF_XDP does not balance
  either — the application moves buffers, so the split stays a decision somebody
  makes.
- **Runtime resize.** `io_uring_resize_rings()` exists. It would cost the one
  property the addresses were made constant for.
- **Keeping one shared area and sizing it against the traffic.** Measuring which
  packets are big and cutting those is how the 240 B break-even happened in the
  first place.

## Where they sit, and why the linker has to be told

Constant addresses, because a block of the heap moves whenever the code above it
changes, and the address is published to the host at boot, in the directory
(xpute-guest ipc/directory.rs `write_directory`), the way virtio writes its
queue addresses into device registers and AF_XDP registers its UMEM. A
published address that moves per build is a discovered address wearing a
constant's clothes.

The linker owns the bottom of the memory, so it has to be kept out of the range.
`--global-base` is how it is told where its own data may begin, and the same
arithmetic has to produce that flag and the addresses the code uses — which is
why both come out of one file of numbers, `spec/client/kernel/sys/mem/mem.json`:
it is generated as the kernel's `sys/mem/mem.spec.rs`, which
`rust/client/kernel/src/door/layout.rs` lays the rings from, and as the
`mem.spec.rs` beside `rust/client-bracket/wasm/build.rs`, which hands it to the
linker through xpute-guest wasm/link.rs `link`. There is no second place to get
them wrong.

The map as the code holds it (mem.json, door/layout.rs; a descriptor range is
`ring_bytes` rounded up to a page):

```
addr                          bytes          what
────────────────────────────────────────────────────────────────────────
0            1,048,576          1.00 MiB     stack, descending  STACK_BYTES
1,048,576    1,060,864         12 KiB        sq descriptors     RING_DESC_AT
1,060,864    1,081,344         20 KiB        cq descriptors
1,572,864    3,670,016          2 MiB        sq payloads  512 × 4 KiB  RING_SLOT_AT
3,670,016    7,864,320          4 MiB        cq payloads  1,024 × 4 KiB
8,384,512    8,388,608          4 KiB        the directory      DIRECTORY_AT
────────────────────────────────────────────────────────────────────────
8,388,608                                    .rodata .data .bss   ← --global-base
────────────────────────────────────────────────────────────────────────
page past the data                           .fixed — the terrain and the rest
past .fixed                    end           heap
```

Two ranges that grow without moving each other: the rings have everything under
8 MiB (mem.json `GLOBAL_BASE`) and the module's data everything over it.
`.fixed` begins at the first page past the module's data (pxl-kernel
sys/mem/section.rs `fixed_at`) and the heap past it.

**The slack under the directory, about 508 KiB, costs address space, not RAM.**
Linear memory is backed by the OS on first touch, so an untouched gap is not
resident. The exception this repository has already met is accounting rather
than residency: JavaScriptCore reports the whole reservation as the page's
JavaScript memory — a Safari heap timeline read it as one `ArrayBuffer` — which
is why a phone is given 512 MiB and not 1 GiB (mem.json `PHONE_BYTES`,
`ts/client/src/abi/boot.ts`). The reservation did not change here, so neither
does that.

**The stack stays at the bottom, and that is the only guard there is.** rustc
passes `--stack-first` and `-z stack-size=1048576` to wasm-ld for
wasm32-unknown-unknown by default, and build.rs passes both too, from
`STACK_BYTES` through xpute-guest wasm/link.rs `link`; the linked module's
`__stack_pointer` initializes to 1,048,576. With the stack first, an overflow
goes below zero, wraps as an unsigned 32-bit address, lands outside the memory
and traps. With the default layout it would
write over `.data` silently instead, which is the reason Rust and Zig both pass
the flag and why LLVM has an open issue to make it wasm-ld's default. There is
no MMU here and so no guard page: the wraparound is the whole of the protection,
and moving the stack anywhere else gives it up.

## What crosses by address now

`WORLD_SNAPSHOT`, which was the UI's JSON as a string, is a block the kernel
holds, named by its address and length, `at` and `len`
(spec/client/kernel/domain/control/cmd.json `WORLD_SNAPSHOT`).

A reply is the completion's result word (xpute-guest ipc/door.rs `Door::drain`),
so there is no reply scratch. `REPLY_BYTES` was 256 KiB — a single constant that
alone decided how big the shared area had to be.

What stays inline is what a slot is for: the commands' packets, which the kernel
reads in place in their slots without copying (xpute-guest ipc/ring.rs
`Ring::packet`). That is the slot doing UMEM's job already, and moving them to
blocks would add a round trip to remove a copy that is not happening.

## A full ring is a wait, not a panic

`Outbox::post` tried the completion ring, then the spill, and asserted. The
spill was io_uring's backlog — "never dropping events but providing
backpressure on submits" — but io_uring has the second half too: a submission
is refused `-EBUSY` while the backlog stands, and the application must reap
completions and try again. There was no second half here.

There is no spill now. The host reads the completion ring dry before it rings
again, so overrunning it is a sizing bug: a completion that does not fit is
ENOSPC (xpute-guest ipc/door.rs `Door::post`). `drain` stops before a command
when the completion ring has no room for what one command may leave, its
signals and its reply (`Door::has_room`), and what is left stays in the
submission ring in order. The submission side answers EAGAIN, and the host
rings until there is room (@xpute/host ipc/doorbell.ts `Doorbell` `submit`), so
both directions wait.

io_uring needs a backlog at all because completions arrive from hardware and
cannot be refused mid-operation. Here everything inside a drain is synchronous,
so the stop is enough.

## What this leaves open

- The kernel's bound is `SIGNALS` (pxl-kernel door.rs), what a turn raises
  after its drain.
- Nothing was run. This is `cargo clippy -D warnings` and `deno task check`,
  both green, and no more than that.
