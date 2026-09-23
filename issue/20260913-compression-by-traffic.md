# Compression, by what is travelling

**2026-09-13** — asked whether anything beats gzip. For the baked pages,
yes, and by a margin; but the tiles are one kind of traffic among several,
and the codec that suits a page baked once is the wrong one for a small
message sent a hundred times a second. This records what was measured, what
each kind of traffic wants, and the order it is worth doing in. Not
scheduled: it is later work, and nothing here is built. Measured again on
2026-09-22 at the 512-cell page, with a different answer on the codec and an
open question on where it is decoded — see the last two sections.

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

## 2026-09-22: measured again, at 512 cells

Asked again once terrain v4 was baked — 512-cell pages with the level
pyramid on them, gzip as before. Three things have moved since the first
round: the page, Safari, and whether the kernel stays in a browser.

### Size

Five hundred v4 leaves sampled across the set, 525.6 MB raw, each page
recompressed on its own. The baseline is gzip -6 because that is what the
bake writes (`flate2`'s default); the first round's baseline was gzip -9.

| Codec             | Per page | Against gzip -6 |
| ----------------- | -------- | --------------- |
| gzip -6 (bake)    | 8.0 KB   | 1.000           |
| gzip -9           | 6.6 KB   | 0.827           |
| pigz -9           | 6.4 KB   | 0.803           |
| pigz -11 (zopfli) | 5.6 KB   | 0.695           |
| zstd -19          | 4.9 KB   | 0.607           |
| zstd -22          | 4.3 KB   | 0.541           |
| brotli -q11       | 4.2 KB   | 0.526           |
| xz -9e            | 4.2 KB   | 0.527           |
| lz4 -12           | 10.5 KB  | 1.312           |

zstd -22 is now within 3% of Brotli, where the first round had zstd -19 a
sixth behind it. Compressing costs 0.4 s a page at -22 and 0.07 s at -19,
once, in the bake. pigz is gzip's format and stays inside gzip's limit
however hard it is pushed; lz4 trades size for speed, which is the wrong
trade for a page that crosses a network.

### Decode

Timed on the same 525.6 MB compressed as one stream, so a process start is
spread over all of it; the best of three.

| Codec       | Decode   | One 1 MB page |
| ----------- | -------- | ------------- |
| gzip -9     | 4.1 GB/s | 0.26 ms       |
| gzip -6     | 3.7 GB/s | 0.28 ms       |
| zstd -19    | 3.5 GB/s | 0.30 ms       |
| lz4 -12     | 3.2 GB/s | 0.32 ms       |
| zstd -22    | 3.1 GB/s | 0.34 ms       |
| brotli -q11 | 1.5 GB/s | 0.69 ms       |
| xz -9e      | 0.6 GB/s | 1.81 ms       |

zstd's usual lead in decode speed does not show here. A page is mostly long
runs of one value and compresses to under 1% of itself, so every decoder
spends its time copying runs at close to memory speed. On these pages zstd
is worth its size, not its speed: about Brotli's ratio at twice Brotli's
decode rate. These are native `libzstd` numbers; a decoder in the kernel's
WASM would be slower by an amount not yet measured.

### What a browser takes, again

Safari decodes `Content-Encoding: zstd` from 26.3 on iOS, iPadOS and macOS
Tahoe, and not in Safari 26.3 on an earlier macOS
([WebKit, Safari 26.3](https://webkit.org/blog/17798/webkit-features-for-safari-26-3/)).
The browsers that count are Chrome, Safari and Firefox; Samsung Internet,
which takes no zstd at all, is not one of them. Against the WebGPU floor
that leaves two gaps, iOS 26.0–26.2 and Safari 26 on a macOS before Tahoe,
so zstd through the transport still needs a fallback negotiated per request.

### Where it is decoded: open

The first round put the codec in the transport, and for a command frame that
still holds. For a stored page it is open again, because the host may not
stay a browser. A React Native host would run the kernel as native Rust and
fetch through the platform's HTTP stack, and there `Content-Encoding` is
whatever that stack decodes: OkHttp takes gzip and nothing else without
added interceptors. A decoder in the kernel goes wherever the kernel does.

|          | The host decodes                                              | The kernel decodes                                                       |
| -------- | ------------------------------------------------------------- | ------------------------------------------------------------------------ |
| Costs    | nothing in the kernel; native code, outside the frame's quota | the decode, inside the kernel's quota; about 11 KB of WASM over the wire |
| Coverage | per host: the two Safari gaps above, Android's HTTP stack     | every host alike; the Worker passes opaque bytes and sets no encoding    |
| Reaches  | no dictionary outside Chromium                                | a dictionary trained on our pages, which no transport here can carry     |

The 11 KB is measured: `ruzstd` 0.8.3 alone in a `cdylib` built as the
module is (`opt-level = "z"`, LTO, `panic = "abort"`, `wasm-opt -Oz`) is
26.0 KB, 11.6 KB gzipped and 10.6 KB under Brotli, against a 1,050.6 KB
kernel. It is the pure-Rust decoder, because `libzstd` is C and building C
for `wasm32-unknown-unknown` wants a C toolchain and a libc shim. The two
write and read one format (RFC 8878), so the bake compresses with `libzstd`
(the `zstd` crate), which alone has the high levels, and whichever decoder
the host builds with reads it; a native kernel can take `libzstd` too.
Streaming decode buys nothing either way: the kernel reads a body whole
(`body_read`).

### Decided for now

gzip stays. The switch is a batch job whenever it is taken up: every baked
object is gunzipped and recompressed, which is lossless and needs no
database, so the bake need not run again for it — terrain v4's 227,310
objects are 2.11 GB under gzip (1.30 GB of leaves, 0.81 GB of levels above),
about half an hour at zstd -19 and three hours at -22 on eight threads.
What moves with it: the bake's `util::gzip` and the pyramid's `gunzip`, the
kernel's decode or the Worker's encoding (whichever the open question
settles), the dev tile server, the uploads' metadata, the manifest's
`content_encoding` and the publish-bucket skill.

## 2026-09-22, later: zstd from the next bake, decoded by the transport

Decided the same day. What is published now — terrain v4, POI v5, geocode
v2 — stays gzip as it was baked, and every bake after it packs zstd.

**The transport decodes, not the kernel.** The objects carry
`Content-Encoding: zstd` and whatever fetches them decodes them: a browser
here, an app's HTTP stack once the mobile client is a React Native app, where
decoding is that host's job. The kernel and the shell see a page's packet and
never its codec, which is the line the first round drew. What decided it was
the layering; a dictionary, the one thing only the kernel could carry, was
measured and does not pay: trained on 1,726 leaves, it takes 5% off a leaf
and nothing off levels 1–3 — the pages are a megabyte raw and already
repetitive enough inside — for 110 KB of dictionary the client would load.

**Only what the bucket holds.** Terrain, POI and geocode pages are baked once
and fetched many times, and are zstd. The app bundle and live responses are
left to the edge. Geocode goes with the other two, which a kernel-side decoder
could not have done: the shell fetches it.

**Who cannot decode it.** Safari decodes zstd from 26.3 on iOS and on macOS
Tahoe, so Safari 26.0–26.2 and Safari on an earlier macOS are left out; they
are pointed at another browser rather than served a second copy. The
mobile half of that gap closes when the mobile client is an app.

**The level is the transport's too.** A bake stores level 19, the top of the
regular levels: on v4's leaves 39% under gzip -6 at four times its
compression time and the same decode. The dev tile server packs what it
makes while someone waits at zstd's default, 3. The two packings of one page
unpack to the same packet — measured at levels 0 to 3 against v4 — so the
dev server is checked against a bake by packet, not by object.

**One label per object, and the bytes decide it.** An object's encoding is
metadata its upload sets, so gzip and zstd versions can share the bucket and
a prefix rewritten in place can hold both mid-sync. The upload therefore takes
the encoding from the bake's manifest (`content_encoding`), never from a
constant, and a bake refuses to resume into a directory whose objects are in
the other encoding. Readers inside the bake — the pyramid reading the level
below, `decode-tile.ts` — tell the two apart by their first bytes.
