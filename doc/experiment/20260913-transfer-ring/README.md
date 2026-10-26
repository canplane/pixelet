# The transfer ring

An experiment, not a component. Nothing imports it and nothing was decided on
it.

The question: without SharedArrayBuffer, can two workers share memory by handing
a fixed set of transferable `ArrayBuffer`s back and forth — each side holding
some, a sender only sending while it holds one, like a sliding window — and what
does that cost against the same window over shared memory?

Two workers joined directly by a `MessageChannel`, no main-thread hop. The
producer holds N buffers, fills one and transfers it while it has any; the
consumer reads it and transfers it back. The shared version is the same window
as N slots of one `SharedArrayBuffer` with a state per slot and `Atomics` to
hand over.

```sh
deno run -A decision/experiment/20260913-transfer-ring/bench.ts
```

## Results, 2026-09-13

Apple M1 Max (10 cores), Deno, 1.5 s per cell, one run each. Not a browser and
not a phone.

**Handover only** — a 64-byte header touched per buffer. Identical for 64 KB and
1 MB buffers: a transfer moves ownership, not bytes.

| N  | transfer, round trips/s | rtt      | shared, round trips/s | rtt      |
| -- | ----------------------- | -------- | --------------------- | -------- |
| 1  | 42k                     | 0.023 ms | 228k                  | 0.004 ms |
| 2  | 58k                     | 0.034 ms | 413k                  | 0.005 ms |
| 4  | 172k                    | 0.023 ms | 724k                  | 0.005 ms |
| 8  | 196k                    | 0.041 ms | 1.7M                  | 0.004 ms |
| 16 | 212k                    | 0.075 ms | 2.6M                  | 0.006 ms |

**1 MB copied in by the producer and out by the consumer:**

| N  | transfer  | rtt      | shared    | rtt      |
| -- | --------- | -------- | --------- | -------- |
| 1  | 12.3 GB/s | 0.053 ms | 10.7 GB/s | 0.050 ms |
| 4  | 31.0 GB/s | 0.107 ms | 23.2 GB/s | 0.130 ms |
| 16 | 30.3 GB/s | 0.503 ms | 23.2 GB/s | 0.645 ms |

## What it showed

- **The ring works.** A handover costs about 5 µs — one message and one turn of
  the receiver's event loop — and copies nothing.
- **N behaves as a window.** Throughput saturates near N = 4 with two threads
  doing the work; past that a larger N only queues, and the round trip grows
  with it. Memory is N × buffer size, fixed.
- **Flow control and ownership come free.** The buffers a producer holds are its
  credit, so a slow consumer stops the producer without a protocol; a sender's
  buffer is detached while away, so the two sides cannot race.
- **Shared memory hands over 5–12× faster**, with no event loop in the path. The
  shared version here polls with a 1 ms wait and keeps a core busy, so read that
  as an upper bound rather than a fair comparison.
- **Once the data is real, the handover stops mattering.** At 1 MB copied per
  round trip, memcpy is the cost and the two converge (transfer came out ahead
  here). The handover's cost is only visible for very small messages at hundreds
  of thousands a second.

## Not measured

- A browser, and Safari on a phone, where every number will be slower.
- Repetition: each cell ran once. The jump between N = 2 and N = 4 looks like
  worker wake-up scheduling and was not pursued.

Related: the slab pool that had this shape between the main thread and a worker
(`sched/slab.ts`, `SLAB_CAP = 4`, chosen without a measurement) went with the
worker path on 2026-09-14; and the in-page boundaries in
[decision/closed/20260913-boundaries-commands-and-what-comes-back-from-april.md](../../closed/20260913-boundaries-commands-and-what-comes-back-from-april.md).
