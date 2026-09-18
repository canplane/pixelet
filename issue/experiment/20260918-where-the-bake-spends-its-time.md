# Where the bake spends its time, and why the Rust is five times faster

**2026-09-18.** The terrain bake in Rust runs a page in 45 ms where the
TypeScript takes 250
([the server in Rust](../20260918-the-server-in-rust-and-the-line-already-drawn.md)).
The question was where that goes: the TypeScript had been worked over, and
nothing about the pipeline is obviously the kind of thing a language changes by
five. Measured, not guessed — and the answer is not the part that looks like the
hot loop.

## Method

24 land pages drawn at random from `out/tiles/land_pages.json`, one connection,
against the local `pixelet-db-geo`. Every figure is a per-page mean over those
24 after a full warm-up pass, so Postgres's buffer cache and V8's JIT are both
warm and neither side is paying for a cold start. The two implementations run
the same SQL against the same server, so anything the server does is common to
both and cancels.

The phases were timed separately by calling into each half directly rather than
by instrumenting the tool: `fetch_page_source` against `pack_source` against
`bake_terrain`, and then the five statements one at a time.

To separate what the server does from what crossing the wire costs, the `base`
statement was run in two forms: as it is, and with `ST_AsBinary(geom)` wrapped
in `octet_length(...)`. The second does identical work — the same scan, the
same clip, the same `ST_AsBinary` — and returns an integer instead of the
bytes, so the difference between them is transfer and decode and nothing else.

## Numbers

Per page, one connection, warm:

| Phase                                     | TypeScript | Rust   |
| ----------------------------------------- | ---------- | ------ |
| the five queries, into client memory      | 267 ms     | 51 ms  |
| pack into XTP                             | 2.0 ms     | 1.8 ms |
| bake (WKB decode → normalize → rasterize) | 64 ms      | 29 ms  |
| whole page                                | 277 ms     | 82 ms  |

The five queries, apart (TypeScript):

| Statement   | ms a page |
| ----------- | --------- |
| `base`      | 166.8     |
| `ele`       | 11.8      |
| `buildings` | 4.9       |
| `segments`  | 3.2       |
| `coast`     | 3.2       |

And `base` split into what the server does and what the bytes cost:

| `base`                           | TypeScript  | Rust       |
| -------------------------------- | ----------- | ---------- |
| `octet_length(...)` — bytes stay | 16.1 ms     | 12.9 ms    |
| as it is — bytes come back       | 165.4 ms    | 31.3 ms    |
| **the bytes**                    | **+149 ms** | **+18 ms** |

That page carries 3.2 MB of WKB, essentially all of it `base`'s: about
22 MB/s into JavaScript against about 175 MB/s into Rust.

## What it shows

**The rasterizer is not the story.** `bake_terrain` — the WKB decode, the
normalize, the scanline fill over 65,536 cells a channel, the thing that looks
like the hot loop — is 64 ms against 29, a factor of 2.2. It is the smaller
half of the page and the smaller of the two gaps.

**One query moving 3.2 MB is the story.** `base` is 167 of the TypeScript's
277 ms, and 149 of that 167 is the bytes arriving. The server hands the same
rows to both clients in the same time; a pure-TypeScript Postgres driver then
takes eight times as long to turn them into arrays as a Rust one does. That
single difference is larger than every other phase put together.

**XTP is not a cost on either side.** 2.0 ms against 1.8. The format is the
same algorithm in both languages and it is already cheap; the encoder's
per-leaf object churn that
[XTP by fixed shape](./20260914-xtp-by-fixed-shape.md) measured on the rings
does not show up at this scale, because a page is four big typed arrays and a
few thousand short tuples rather than millions of tiny calls.

**Two of the five-and-a-half times are JavaScript's one thread, and they are
recoverable.** A page is 277 ms against 82, a factor of 3.4, but the tools at
concurrency 8 are 250 ms a tile against 45. The difference is that
`Promise.all` over eight "workers" is one thread: it overlaps the database
waits and serializes every byte of decode and every cell of fill, so the
TypeScript goes from 277 to 250 ms a page — eight workers bought ten percent.

Run the TypeScript tool as several processes over disjoint page lists instead,
which is what a `Worker` would be, and it saturates at about 110 ms a tile on
this 10-core machine. The same 200 pages, all warm:

|                                                     | ms a tile |
| :-------------------------------------------------- | --------: |
| TypeScript, one process, concurrency 8 (as written) |       250 |
| TypeScript, 4 processes × concurrency 2             |       113 |
| TypeScript, 8 processes × concurrency 1             |       108 |
| TypeScript, 12 processes × concurrency 1            |       107 |
| Rust, one process, concurrency 8                    |        32 |
| Rust, one process, concurrency 10                   |        33 |

So the end-to-end 5.6 is 2.3 of it being on one thread and 3.4 of it being the
work a page costs — and that 3.4 is exactly the single-connection per-page
ratio above, which is where the driver's bytes and the rasterizer live. Both
sides then stop at the database.

The honest form of the comparison is therefore that the tool as it was written
left a factor of two on the floor, and that a `Worker` pool would have
collected it without any of this. What a Worker pool could not have reached is
the other 3.4.

## What it does not show

The 24 pages are a twentieth of a percent of the coverage and were drawn at
random, where a real bake walks in index order; spatial locality in Postgres's
cache should help that and a dense city should hurt it. The full-set figures in
the issue come from a 200-page sample and are extrapolations, marked as such
there.

Nothing in the TypeScript was changed for this. The one lever that was tried
— real parallelism, as processes — was tried because leaving it out would have
credited the language with a factor that belongs to the thread count. The
others were not: asking for the bytes in a different form, or holding the WKB
on the server and doing the clip differently, might well move the remaining
3.4 and were not attempted, because the pipeline is moving anyway and the
question was where the time goes rather than what the last of it could be
argued down to.

What the numbers do say is which lever would have mattered, and it was never
the rasterizer.
