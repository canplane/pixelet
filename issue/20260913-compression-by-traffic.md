# Compression, by what is travelling

**2026-09-13** — asked whether anything beats gzip. For the baked pages,
yes, and by a margin; but the tiles are one kind of traffic among several,
and the codec that suits a page baked once is the wrong one for a small
message sent a hundred times a second. This records what was measured, what
each kind of traffic wants, and the order it is worth doing in. Not
scheduled: it is later work, and nothing here is built.

## Measured on our own pages

Twenty-five published terrain pages (v2, fetched from the deployment and
decoded) and forty local POI pages (v3), recompressed whole. The CLI tools,
one run.

|              | raw    | gzip -9 | brotli -q11      | zstd -19     | zstd -9       |
| ------------ | ------ | ------- | ---------------- | ------------ | ------------- |
| terrain × 25 | 100 MB | 684 KB  | **453 KB (66%)** | 533 KB (77%) | 642 KB (93%)  |
| POI × 40     | 302 KB | 131 KB  | **109 KB (82%)** | 121 KB (91%) | 133 KB (101%) |

Brotli at its top level takes a third off the terrain pages and a fifth off
POI. Decode time was not measured (the timing run broke on shell quoting).

## What a browser takes

- **Brotli** — `Content-Encoding: br` over HTTPS in every browser the floor
  (iOS 26) includes.
- **Zstandard** — `Content-Encoding: zstd` is recent in Safari: partial on
  iOS 26.1, full from 26.3 ([caniuse](https://caniuse.com/zstd),
  [browser-compat-data#28948](https://github.com/mdn/browser-compat-data/issues/28948)).
  Below the floor's 26.0, so zstd needs a fallback negotiated per request.
- **Shared dictionaries** (Compression Dictionary Transport) — large wins for
  many small objects of one schema, Chromium only.
- **In our own code** — `CompressionStream` and `DecompressionStream` do
  gzip and deflate natively and nothing better; a Brotli or zstd decoder of
  our own is WASM we ship.

## By traffic

| Traffic                                                                  | Who compresses, when                | Wants                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------------------------------------------ | ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Baked pages** — terrain, POI, geocode                                  | the bake, once; the monthly refresh | the best ratio at any compression cost: **Brotli q11**, stored pre-compressed, served with `Content-Encoding: br` through the Worker as gzip is now                                                                                                                                                                                                                                           |
| **Assets** — glb, png, webp                                              | ingest, once                        | png and webp are compressed already — nothing. A glb is mostly geometry: a mesh codec (meshopt) is the lever, not a byte codec                                                                                                                                                                                                                                                                |
| **The app bundle** — JS, CSS, HTML                                       | the build, once, or the edge        | pre-compressed Brotli at build, or the edge's own; which one the deployment does today is not checked                                                                                                                                                                                                                                                                                         |
| **Live API responses** — forward search                                  | per request, on the server          | a fast level: zstd where the client accepts it, Brotli or gzip otherwise; small, so the edge's default is likely enough                                                                                                                                                                                                                                                                       |
| **Realtime messages** — commands, state sync, chat, once a server exists | per message, both directions        | tiny and frequent: a generic codec per message barely helps and WebSocket's permessage-deflate is deflate only. The lever is the **encoding** — fixed frames and XTP, delta against the last state — then, if still needed, one compression stream per connection that keeps its window across messages, or zstd with a dictionary trained on our frames, decoded by the core once it is WASM |
| **Uploads** — a user's model, placements                                 | per upload, in the browser          | `CompressionStream("gzip")` is what a browser has; a glb goes as the mesh codec left it                                                                                                                                                                                                                                                                                                       |
| **Local persistence** — placements, a future OPFS tile cache             | on write                            | store what arrived as it arrived; placements are small enough that encoding matters more than a codec                                                                                                                                                                                                                                                                                         |
| **Inside the page** — worker slabs, the change log                       | never                               | memory is moved or read in place; compression would only add a copy                                                                                                                                                                                                                                                                                                                           |

## What follows from it

- **Encoding before codec.** What a codec can take is set by the layout it
  is given: quantised positions, columns rather than rows, deltas. XTP's
  column layer and the string-sequence leaf (BACKLOG) do more for realtime
  traffic than any codec swap.
- **The codec belongs to the transport, not the protocol.** A command frame
  stays the same bytes whether it crosses a slab, a ring or a socket; a
  boundary that compresses does so around the frame, and says so in its own
  header, never inside the command.
- **Chosen per boundary**, like reliability: offline and immutable takes the
  slowest best ratio, per request takes a fast level, per message takes an
  encoding and a stream.

## When

The baked pages first, when it is taken up: the bake tools compress with
Brotli (`node:zlib`'s `brotliCompressSync`), the uploads and the monthly
workflow say `Content-Encoding: br`, the publish-bucket skill follows, and
terrain moves to a new version because a published one is immutable. The
realtime row is designed with the server and the command channel, not
before.
