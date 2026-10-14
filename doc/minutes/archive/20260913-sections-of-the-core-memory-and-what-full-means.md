# Sections of the core memory, and what "full" means in each

**2026-09-13** — BACKLOG P0-1 at the time asked for spill past the reservation to become
a budget signal before a WASM kernel reads the pool. Working out what that
signal is showed the pool has no map: one buddy pool over the whole core
memory, every kind of thing carved from it in arrival order, so where a
structure lands and which kind of thing overflowed are accidents of
sequence. This records the map that replaces it, and — the part that
started as a question about April's transaction controller — what each
part of the map does when it is full. Built the same day (d8db67e): steps
1–4 of the order below; step 5 comes with the ring.

## What there is today

- The core memory is one fixed `WebAssembly.Memory`, 8 regions of 64 MB on
  the desktop and 4 on a phone (`wasm/core_pool.ts`).
- One buddy pool carves it. Chunk meshes, run sets, LOD tier caches,
  terrain pages, POI pages, the change log's journal and tables, the
  scheduler's lanes, the mesher's scratch — all from the same pool, in the
  order they were first asked for.
- When the backing is used up the pool opens plain `ArrayBuffer` regions
  and says nothing; the ledger overlay shows a warning nothing acts on. A
  WASM kernel cannot see those regions.
- The one lever that frees memory is `RETAIN_BUDGET_BYTES`
  ([20260912-what-to-render-when.md](../20260912-what-to-render-when.md)):
  off-screen chunks are reclaimed, farthest first, once what they hold is
  over a quarter of the reservation.
- The change log's ring sits inside the backing because it is allocated at
  startup. Nothing guarantees it; a WASM core that expects its rings at a
  known place in linear memory would be relying on load order.

## The map: sections with a fixed start and a fixed size

Like a process's address space, the core memory is cut into sections, each a
range with a start offset and a size fixed for the life of the page, each
its own buddy pool over that range. No section may borrow from another —
that is the rule that costs something (a section with room cannot lend it)
and buys the rest (a burst in one kind can never evict another kind, and
"full" always names which kind).

| Section     | Holds                                                                                                                       | Lifetime                                    | Size                                                                                 |
| ----------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------ |
| **fixed**   | the command and completion rings, the change log's journal and revision tables, the scheduler's lanes, the mesher's scratch | allocated once at startup, never freed      | small; a shortfall fails at startup, never later                                     |
| **page**    | terrain pages, POI pages                                                                                                    | long — resident while near                  | a fixed cap; ~4 MB a terrain page, up to the count the cleanup keeps                 |
| **mesh**    | chunk meshes, run sets, LOD tier caches                                                                                     | churn — built and dropped as the view moves | a fixed cap; the ledger's `biggest` is 1 MB, `reserved` peaked at 320 MB in one pool |
| **scratch** | what one tick uses and drops                                                                                                | one tick; rewound every frame               | fixed; overflow is a bug in that tick                                                |

Sizes are set from what the ledger reports today and adjusted after one
phone reading. A WASM core is handed a section's start and size, and the
fixed section's layout is the contract between the glue and the core:
the rings are at known offsets, not wherever the allocator put them.

## What "full" means, and what April did

April's `TxController` (`checkpoint/20260405-xpute`, `core/ipc/tx.ts`) is
the last time this repository answered the question, and it answered it
three ways at once: a cap on open transactions past which `send` returned
`EAGAIN`; caps on reply mailboxes past which new replies were dropped
("backpressure via loss"); and a timeout as the end of every transaction.
Refusing the request was right. Dropping the reply was not, because a reply
carried its value — a dropped reply was lost work. The three policies did
not come from one rule.

The references converge on one:

| Where                                                                     | When full                                                                                                                                                                                                                             |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| credit-based flow control — PCIe, InfiniBand, a TCP window, a virtio ring | the receiver grants credits; with none, the sender does not send. The slabs are this already (`SLAB_CAP` 4 is the credit, `submit` returns null without one), and the transfer-ring experiment showed the buffers held are the credit |
| a submission ring — io_uring, NVMe                                        | submit fails (`EAGAIN`); the submitter retries. Nothing is lost                                                                                                                                                                       |
| a completion ring — io_uring                                              | overflow is flagged and completions kept aside; in practice sized so submissions cannot outrun it                                                                                                                                     |
| a network queue — a router, RED                                           | drop, and a higher layer retransmits; only where loss is allowed                                                                                                                                                                      |
| the change log here                                                       | an overwritten cursor reads as lost and the reader re-reads its topics — possible only because a change carries no value                                                                                                              |

**Drop what can be re-read; refuse at the sender what cannot.** April's reply
mailboxes dropped what could not be re-read. The change log may overwrite
because it never carried the value in the first place.

## The policy, per section

| Section     | Full means                               | Then                                                                                                                                                                                            |
| ----------- | ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **fixed**   | the submission ring has no slot          | `EAGAIN`: the submitter (the scheduler) retries next tick. The completion ring is sized so that one submission guarantees one completion slot, and cannot overflow                              |
| **page**    | the cap is reached                       | drop the farthest page — what the cleanup count already does, with the section's cap as a second trigger                                                                                        |
| **mesh**    | use past a threshold                     | reclaim off-screen chunks, farthest first, until under. If nothing off-screen is left, `acquire` returns null and the chunk stays pending and asks again next tick — the same shape as `EAGAIN` |
| **scratch** | the tick asked for more than the section | a bug; fail loudly                                                                                                                                                                              |

The credits already exist: `MAX_INFLIGHT` 6 for the network, `SLAB_CAP` 4
for the worker, the tick's budget for CPU. A ring's capacity is one more,
and checking a credit before submitting is what the fetch scheduler's pump
already does.

## Not this

- **Not a byte budget over what is drawn.** When the on-screen working set
  itself exceeds the memory — a wide zoom on a phone — no reclaim of
  off-screen work helps; the answer is coarser LOD, the item in BACKLOG P1
  with CesiumJS as its precedent. The sections make that condition
  visible (the mesh section full with nothing off-screen) rather than
  solving it.
- **Not a router or a transaction table.** One producer and one consumer per
  ring needs neither. A router is for several queues; a transaction table
  is for replies that are waited on. Both are April's, and both are looked
  at when a second worker or a server makes a second queue exist
  ([20260913-boundaries-commands-and-what-comes-back-from-april.md](./20260913-boundaries-commands-and-what-comes-back-from-april.md)).

## Order

1. `core_pool.ts` becomes the map: section pools over `CORE_MEMORY` at fixed
   offsets; today's `CORE_POOL` is the mesh section. — done
2. The fixed section takes the change log, the waker's lanes, the bake
   context's scratch and the scheduler's lanes, through an acquire that
   fails rather than spills. — done
3. The page section takes terrain and POI pages. — done
4. Per-section use in the ledger, and the mesh section's threshold as a
   second trigger for the reclaim pass; an acquire that returns null, and
   chunk_node treating null as "next tick". — done; the mesh section still
   spills today, so the null path is exercised by its test until a kernel
   makes the section refuse
5. The submission ring's `EAGAIN` comes with the ring, at the Rust mesher.

## Status 2026-09-15

The rings are the first two blocks of the fixed section, at fixed offsets
(`pxl_core::memory`'s `RING_SQ_AT` and `RING_CQ_AT`), taken at boot and
checked there; the boot block names them. No scratch section was cut.
