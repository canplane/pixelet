# Compression, by what is traveling

**2026-09-13** — asked whether anything beats gzip. For the baked pages,
yes, and by a margin; but the tiles are one kind of traffic among several,
and the codec that suits a page baked once is the wrong one for a small
message sent a hundred times a second. This records what was measured, what
each kind of traffic wants, and the order it is worth doing in. Measured
again on 2026-09-22 at the 512-cell page, with a different answer on the
codec and an open question on where it is decoded. The baked pages were
built on 2026-09-24 and are zstd; every other row here is still later work.

> The first half, Brotli q11 for the baked pages (the "By traffic" row and
> "When"), and then "gzip stays" ("Decided for now"), is superseded by the
> later dated sections: zstd, decoded by the transport, and zstd stays.
> What binds is `CONTENT_ENCODING` and `pack`/`unpack` in
> `crates/server/geo-bake/src/util.rs`, which are zstd alone.

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
  is given: quantized positions, columns rather than rows, deltas. XTP's
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

> Superseded by "2026-09-22, later" below: zstd, not Brotli.

The baked pages first, when it is taken up: the bake tools compress with
Brotli (`node:zlib`'s `brotliCompressSync`), the uploads and the monthly
workflow say `Content-Encoding: br`, the publish-bucket skill follows, and
terrain moves to a new version because a published one is immutable. The
realtime row is designed with the server and the command channel, not
before.

## 2026-09-22: measured again, at 512 cells

Asked again once terrain v4 was baked — 512-cell pages with the
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
(then `body_read`; now the host's `get` in `apps/client/src/kernel/io/read.ts`
reads it with `arrayBuffer()`).

### Decided for now

> Superseded the same day by the section below.

gzip stays. The switch is a batch job whenever it is taken up: every baked
object is gunzipped and recompressed, which is lossless and needs no
database, so the bake need not run again for it — terrain v4's 227,310
objects are 2.11 GB under gzip (1.30 GB of leaves, 0.81 GB of the zooms above),
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
and nothing off z15 to z13 — the pages are a megabyte raw and already
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
unpack to the same packet — measured at z16 to z13 against v4 — so the
dev server is checked against a bake by packet, not by object.

Measured again on 2026-09-22, asked whether 3 would serve a view better for
being faster: 500 v4 pages taken evenly across the bake, 502 MB unpacked,
through `zstd -b`. Level 3 packs them to 4.57 MB at 3,147 MB/s and 19 to
3.06 MB at 22 MB/s — a third smaller, about 45 ms a page against 0.3 — and
19 unpacks faster, 8,942 MB/s against 6,866. The level costs the bake alone,
once; the reader gets fewer bytes and a quicker decode. 19 stays, and 3
stays where a page is packed while someone waits for it.

> No longer applies: every bake writes zstd, and the bake reads zstd
> alone (`unpack` in `crates/server/geo-bake/src/util.rs`), so there is no
> mixed bucket to tell apart and no `encoding_of`. What stands is that the
> upload takes the encoding from the manifest (`script/publish.ts`).

**One label per object, and the bytes decide it.** An object's encoding is
metadata its upload sets, so gzip and zstd versions can share the bucket and
a prefix rewritten in place can hold both mid-sync. The upload therefore takes
the encoding from the bake's manifest (`content_encoding`), never from a
constant, and a bake refuses to resume into a directory whose objects are in
the other encoding. Readers inside the bake — the pyramid reading the zoom
finer, `decode-tile.ts` — tell the two apart by their first bytes.

## 2026-09-24: what the gap costs, and the codec asked again

The section above priced the zstd gap as something the excluded browsers pay:
"they are pointed at another browser rather than served a second copy". They
do not pay it alone. Left to itself the edge does serve the second copy — it
decodes the body for any client that did not offer zstd — and the decoded copy
is what the cache then holds.

Measured against the deployment, one object each, a zstd-accepting client
against one without:

| object                 | zstd   | without |
| ---------------------- | ------ | ------- |
| a terrain page         | 8,543  | 526,936 |
| the shared ocean page  | 101    | 526,936 |
| the geocode dictionary | 90,141 | 619,224 |
| a geocode group        | 2,746  | 12,192  |

A terrain page is a fixed-size raster, so the emptier it is the worse the
ratio — and the worst of them is the object the most requests are for, the
ocean page every water tile falls back to, at five thousand times its packed
size. Nothing failed, because a client is happy either way. It only cost.

The bill is not what it cost. R2 charges for requests and storage rather than
egress, and the Workers plan that fails closed counts requests
([20260816-deployment-hosting.md](./20260816-deployment-hosting.md)), so a
copy sixty-two times the size is billed the same. What it spends is the
reader's transfer and the room in the cache.

**The Worker cannot refuse such a client, and that was learned by deploying
it.** "An object goes out in the encoding it is stored in or not at all" reads
like a rule a Worker can hold, and it is not: Cloudflare rewrites
`accept-encoding` on the request it hands a Worker and negotiates with the real
client itself on the way out, so what the Worker reads is the edge's list and
never the browser's. A 406 written there refused every client — `zstd`, `*`,
`identity` and the header absent were all tried against the deployment, and all
four were refused. Whatever refuses a client for its encoding has to run where
the client's own header is still intact, which is not a Worker.

What the Worker does keep is the cache: the encoding stays parked on a header
the cache has no meaning for while the entry is stored, because an encoding a
cache can read is one it normalizes, and it stores the decoded body.

**So the reader finds it in the body instead.** A browser handed a
`Content-Encoding` it does not know passes the bytes through undecoded rather
than saying anything, so a body that still begins with a compression magic is
one the transport did not decode — a fact about the browser, found on the one
side that can see it (`packed` in `apps/client/src/kernel/io/read.ts`). The
bake once made the same judgment on the other side of the wire (`geo-bake
util.rs`'s `encoding_of`, since gone). Such a body is not handed to the
kernel, which would spend a heap block and fail in its decoder far from the
cause; the notice goes up and every later read is settled without being made.

The decoded copy the edge makes for a client that did not offer zstd is
therefore still made, and still cached. Nothing in a Worker prevents it. What
it costs is in the table above, and the only lever left is the codec.

**The codec, asked again now that the gap has a price.** The 09-22 table puts
brotli q11 at 4.2 KB against zstd -22's 4.3, so the gap buys nothing in size;
what it buys is decode, 0.34 ms a page against 0.69. Nothing in this tree
decodes, though — the transport does, in the browser's own network stack, off
the main thread, and at most sixteen reads are out at once (`MAX_CREDITS` in
`crates/client/kernel/src/param/io.rs`). The difference lands outside the
kernel's quota and outside the bill alike.

What keeps zstd is the host after this one. OkHttp takes gzip and nothing
else, so brotli moves the gap to the React Native client rather than closing
it. Only a gzip closes it everywhere — zopfli's 5.6 KB is a gzip stream and
every stack reads one — and that is 30% more bytes than zstd -22, paid by the
reader, since the bill does not count them. zstd stays.
