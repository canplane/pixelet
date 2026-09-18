# The server in Rust, and the line the infrastructure had already drawn

**2026-09-18** — a design conversation, not a code change. It began as "move
the Hono server to Rust, the dependencies resolve through `pxl-spec` and
`xpute-core`" and spent most of its length on two questions the phrasing hid:
what, exactly, is the server, and where does it split. The answer to the
second turned out to be written down already — in `infra/`, not in the code.

This records the arguments, what was rejected and why, and the order the
pieces go in. The first steps were taken the same day — what they turned up is
at the end.

## What `apps/api` actually is

`deno task dev` runs it as one thing. It is three, and only the first is a
server:

|                             |  lines | what it is                                                                      |
| :-------------------------- | -----: | :------------------------------------------------------------------------------ |
| the live server             |   ~350 | `/api/search`, plus `main.ts`'s node-http↔fetch bridge, config and pools        |
| dev-only tile routes        |   ~440 | terrain/poi/geocode/asset, behind `__IS_DEV__`; production serves these from R2 |
| the offline bake and ingest | ~4,400 | CLI programs against the geo DB                                                 |

Two things fell out of listing them.

**In production `/api/internal/*` has no routes at all.** All three cache
invalidation endpoints are registered inside the dev-gated tile module
(`module/tile/mod.ts`), so a deployed build carries `admin_auth` and nothing
behind it.

**socket.io is dead code.** `main.ts` constructs a `Server`, binds it to the
HTTP server, and no handler is ever registered on either side; `apps/web` has
no client for it. The production edge does not proxy `/io` either
([one server root](./20260910-one-server-root.md)). It is removed rather than
ported: its wire protocol — its own handshake, reconnection and packet
encoding — shares nothing with WebTransport or with a raw WebSocket fallback,
so keeping it would not be reuse.

## Why Rust here is not why Rust elsewhere

The comparison raised was Figma, which rewrote its multiplayer server from
TypeScript to Rust and left the rest of its backend on Node (_Rust in
production at Figma_, 2018; _How Figma's multiplayer technology works_, 2019 —
cited from memory, not re-read). Two claims that came attached to it — that it
runs on Tokio, and that it persists through a low-level file I/O layer — are
not in those writeups and were not verified; they also decide nothing here.

What transfers is the structural fact: **Figma drew the boundary by workload,
not by language.** What went to Rust was the one part that is stateful,
long-lived and memory-hot.

The reason here is a different one, and it changes where the boundary falls:

|                      | Figma                                        | pixelet                                                           |
| :------------------- | :------------------------------------------- | :---------------------------------------------------------------- |
| the reason           | throughput and memory on a hot stateful path | **the domain model is already Rust, and it is the wire contract** |
| the boundary follows | the hot path                                 | `pxl-spec`                                                        |
| the result           | two languages in the backend, permanently    | one, or the point is lost                                         |

`pxl-spec` already carries the half of `@pxl/shared` that only the server
uses: `encode_terrain`, `encode_poi_page` and `encode_empty_poi_page` are in
`crates/pxl/spec/src/bake/format.rs` today, because the client needed the
decode side and the pair is kept whole. So the encoders exist in Rust before
the server does.

Which means Figma's own conclusion — keep the ordinary API on the old runtime
— must not be copied. What stays in TypeScript here stays because it does not
touch `pxl-spec`, runs a few times a year, and gains nothing; not because it is
"a normal web API".

`@xpute/core/kit/word.ts` and `kit/lower.ts` have no Rust counterpart and need
none: branded integers, and a tuple↔object conversion the Rust does
structurally.

## The two premises the crates impose

**The crates are single-threaded by construction.** `xpute-core`'s
`ArrayBuffer` is an `Rc<Storage>` (`kit/array.rs`), so every `U8Array`/`Bytes`
is `!Send`; and `sys/global.rs` carries `unsafe impl<T> Sync for Global<T>`
justified by "one thread, and no turn holds a reference across another" —
sound in a WASM module, a data race in a worker pool. `pxl-spec` itself uses no
`Global` (only `sched::task`'s executor does), so the live constraint is `Rc`:

- an xpute or `pxl-spec` value is born and dies inside one synchronous call;
- the boundary type is `Vec<u8>`, never `U8Array`;
- `spawn_blocking` is fine — the closure captures `Send` input, returns
  `Vec<u8>`, and the `Rc`s live and die inside it.

The temptation when the compiler refuses is to reach for `Arc`. That breaks
the premise rather than satisfying it, and the premise is the same one
Figma's per-document single-writer model has and the client's turn already
runs on. The right shape for the eventual realtime half is therefore one owner
per shard with a channel in, not a work-stealing pool over shared state.

**`panic = "abort"` is workspace-wide.** `[profile.*]` is honored only in the
root manifest, so any member inherits it. For the WASM module it is correct —
unwinding costs code size and the kernel's own design is that a trap ends the
client. For a server and a bake tool it is not, and the cost is concrete:

- `bake_tiles.ts` wraps each page in `try/catch`, counts `failed++` and
  continues. Under `abort` there is nothing to catch, so one bad page out of
  tens of thousands ends the run.
- The failure mode of a TS→Rust port of the raster path is _exactly_ a panic.
  Measured this session: `new Uint8Array(4)[99] = 5` is silently ignored in
  JavaScript and panics in Rust; `Math.floor(NaN)` is `NaN` so a loop bounded
  by it runs zero times, while `f64::NAN.floor() as i32` saturates to `0` and
  the same loop runs once. `ground_baker.ts` indexes a channel millions of
  times per page.
- A web framework wraps each request in `catch_unwind`; under `abort` that
  machinery is inert and one request takes the process down.

A second workspace was considered and is not needed. A **custom profile** in
the same root manifest does it, verified in a scratch workspace of the same
shape:

```
[profile.server]          cargo run --profile release  → exit 134, no Drop, nothing caught
inherits = "release"      cargo run --profile server   → "drop ran", caught = true, exit 0
panic = "unwind"
```

`apps/web/script/build_wasm.ts` builds with `--release`, so the module is
untouched, and the two binaries never link together.

## Server-side WASM, and why not

Raised as a symmetry argument — both sides being WASM would be beautiful — and
separately as an industry question. Both are answered no, for different
reasons.

**The symmetry is already had, one layer down.** What this repo keeps in pairs
is _source_: `@xpute/core` ↔ `xpute-core`, `@pxl/shared` ↔ `pxl-spec`, pinned
to the bit by the vectors ([where the pair differs](./20260914-where-the-pair-differs.md)).
That pair is not "both TypeScript" or "both Rust" — it is one set of
definitions expressed for two runtimes. Extending that logic gives "compile
`pxl-spec` for the host", which is what this move is.

Two WASM artifacts would share the file extension and nothing else. `pxl-wasm`
exports `boot` and `interrupt`, links against an **imported** memory it cannot
grow, assumes one thread and a turn, is `core::arch::wasm32` throughout and
has no host build at all. A server module would export a request handler, link
against WASI, and need sockets. Making both WASM does not make them symmetric;
it makes the server inherit the client's constraints on a machine that has
none of them.

**The adoption picture, as of this conversation** (knowledge to 2026-05, not
searched): server-side WASM won where the code is _someone else's_ — untrusted
or user-supplied code (Shopify Functions, Zed extensions, Envoy/Istio
proxy-wasm filters, Redpanda transforms, Extism) and multi-tenant edge
(Fastly Compute is WASM-native, Fermyon Spin). It lost the category it was
loudest about, "WASM replaces Docker": Krustlet archived, Docker+Wasm niche,
containers unmoved. WASI took years to grow sockets and async, and by then
general server workloads had nothing to gain by switching.

None of the five usual reasons applies here. The server is resident
(`restart: unless-stopped`), so there is no cold start; it runs only our own
code, so there is nothing to sandbox; everything is already Rust, so language
independence buys nothing; native is the alternative, so WASM is the slower
one; and there is no plugin surface.

**The sharpest objection is the product's, not the runtime's.** The
destination is realtime multiplayer, which means the server initiates —
it speaks first. A serverless or worker model is request-scoped: the runtime
may evict the instance between requests, so there is no "our process" to hold
a connection list in. Cloudflare shipping Durable Objects to patch exactly
that hole is the admission that plain workers cannot. A layer that cannot push
is not a candidate for a server whose job will be pushing.

**The symmetry worth having is a different one, later.** The server running
`pxl-kernel` natively as the authority — the same simulation on both sides,
each compiled for what it actually is — is Figma's shape and a headless game
server's. The door is open and should stay open: only `pxl-wasm` is
WASM-only, and `pxl-kernel` already builds on the host in `check:rust`. Do not
introduce WASM-only assumptions into `pxl-kernel`.

## Where the boundaries fall

The proposal was to split the GIS-flavoured serving from the ordinary
application serving. The boundary is harder than that, and it is already
provisioned: **there are two databases.**

And the code has never crossed between them. `main_pool` is created in
`app/resource.ts`, probed once with `select 1`, and **never queried by
anything** — no service, no provider, no tool. A Postgres instance runs in
production whose only job is answering that probe at startup. Meanwhile
`infra/README.md` records that production's db-geo holds _only_
`overture_places`, `overture_divisions` and `overture_division_points`; the
terrain tables are not there, so the bake pipeline could not run on that box
even if it were shipped to it.

So the split does not create a boundary. It removes a join that was never
real.

```
              ┌── pxl-geo-bake   db-geo (all tables)   local / offline
pxl-spec ─────┼── pxl-geo        db-geo (three)        resident
              └── pxl-api        db-main               resident — when it has code
```

No edges between the three; each sees only `pxl-spec`. One database each.

**`pxl-geo-bake` rather than `pxl-bake`:** it says what it bakes, and it says
that `geo` and `geo-bake` are the resident and offline halves of one data
domain. The asset ingest is also a bake and is not geo — it stays in Deno, so
the name never has to stretch over it.

**The dev tile routes belong to the bake, not to the API.** They exist to serve
what the bake produces before it is published, so they are a `pxl-geo-bake`
binary. That is what removes the last would-be edge from `pxl-geo` to the
pipeline, and it keeps the deployed image to a static binary and a Postgres
client with no pipeline in it. `dev:remote` already serves published tiles for
ordinary work, so this binary is what one runs _while working on the bake_ —
`deno.json` already draws that line ("Not usable for bake work: a published
tile prefix is frozen").

**`pxl-api` for the db-main half**, decided after `pxl-app` was rejected:
`app` already means four things here, and one of them is the client —
`wrangler.jsonc` contrasts `/api/*` with "the app". `pxl-api` carries its own
risk, that `/api/` is the root _everything_ answers under; the crate's own doc
comment says so, so that a later reader does not conclude it owns the prefix.
The crate is not created until it has code.

## What the framework choice turns on

Not benchmarks. `/api/search` is a trigram-indexed Postgres query with JSON
around it; the framework is orders of magnitude off the critical path, and the
actix-vs-axum gap that existed years ago has closed. Choosing on throughput
here would be choosing on the wrong axis.

It turns on **who owns the process**, because the realtime half is next and
`pxl-geo` is an HTTP surface rather than "the server":

|           | owns the runtime                                                            | fits                                |
| :-------- | :-------------------------------------------------------------------------- | :---------------------------------- |
| **axum**  | no — `Router` is a `tower::Service`, handed to hyper or `axum::serve` by us | ✓                                   |
| actix-web | yes — its own `actix-rt` System                                             | binds the realtime half to actix-rt |
| Rocket    | yes — `#[launch]`                                                           | routing policy into attributes      |

Recorded in fairness: actix-web's per-worker single-threaded arbiter maps
unusually well onto the `Rc`/one-thread premise above, and that is a real point
nobody else makes for it. It still loses — the premise is satisfied by
`Vec<u8>` at the boundary, CPU-bound bake work would block an arbiter anyway
and need `web::block` (which is `spawn_blocking` by another name), and a shard
owner wants a dedicated thread regardless.

**The DB layer matters more, and the usual advice is wrong here.** diesel is an
ORM over a schema DSL and every query in this repo is raw PostGIS
(`ST_Resample`, `ST_DumpValues`, `ST_Difference`) — nothing to gain. sqlx's
headline feature is compile-time-checked queries, which needs a reachable
database at build time or a checked-in offline cache; `check:rust` is a gate
that must pass with no geo DB up, and the Docker build has none either, so
sqlx would be used with its selling point switched off. **tokio-postgres plus
deadpool-postgres**: raw SQL is what exists, and per-connection prepared
statement caching is native — which is precisely what replaces
`repository.ts`'s `PREPARE`/`EXECUTE`-with-literal-args workaround and its
`lit()` guard.

**One boundary to hold:** `xpute-core` has no external dependencies and
`pxl-spec` has exactly one (`xpute-core`). That is not an accident — they
compile to WASM and every byte ships. serde, axum, tokio and the Postgres
client go in `pxl-geo` and `pxl-geo-bake` only. Adding serde to `pxl-spec`
because a response type is convenient is the erosion AGENTS.md warns about,
billed to the client's bundle.

## The URL roots

`/api` alone, with two origins behind it, forces the routing split into Caddy —
and Caddy would then have to name `/api/search`, so adding a geo route means
editing a relay. That is the thing
[one server root](./20260910-one-server-root.md) set out to stop.

Two roots restore it:

```
Worker:  /geo/tiles/* → R2,  /geo/* → GEO_ORIGIN,  /api/* → API_ORIGIN
Caddy:   /geo/* → pxl-geo,   /api/* → pxl-api
```

No route names in any relay; adding a route is a change to one server and
nothing else.

This does not contradict that document. What it rejected was splitting on a
_resource property_ — static versus live — which is a routing detail one level
down. Splitting on **which origin answers** is a deployment fact, and it is
what a relay has to know to relay at all. Both prefixes are still reserved on
the server's side, so neither reaches an address bar.

Deferred, deliberately: with one origin there is nothing to route between, and
moving published tile URLs costs a cache generation. R2 keys do not change —
only the prefix stripped to reach them — so there is no re-bake and no
re-upload, but the Worker and the client must deploy together. Four files:
`config/route.ts` (two constants), `worker/index.ts`, `wrangler.jsonc`,
`Caddyfile`.

One question to settle when it happens: **`asset/` is not geo.** Models and
textures are content-addressed ingest output, unrelated to Overture and DEM,
and `asset_url()` builds them under `TILE_ROOT` today. Whether the
bucket-served things stay a `tiles` branch under each root
(`/geo/tiles/terrain/…`, `/api/tiles/asset/…`) or move somewhere of their own
is open.

## What is not moving

The Overture ingests (`ingest_{base,buildings,divisions,places,transportation}.ts`),
the asset ingest and the brand renderer stay in Deno. They run a handful of
times a year, they touch no `pxl-spec` format, and `sharp`→`image` buys
nothing while risking a visual regression on shipped assets. Porting them for
uniformity is the cost without the reason.

The TypeScript pair stays too — `packages/pxl/shared/src/bake/*`,
`model/feature.ts` and the encoders are not deleted. `apps/web` still imports
constants and types from them, `kernel.test.ts` builds its fixture packets with
`encode_terrain`/`encode_poi_page`, and — the larger reason — the byte-for-byte
verification below needs both sides alive to compare.

## How the bake is verified

`source.ts` states the requirement: the offline tool and the live endpoint must
produce **byte identical** tiles. A port has to prove that against current
output, not look right.

The mechanism exists — `crates/*/vector/*.tsv`, written by the TypeScript and
read back by the Rust test beside the module. `crates/pxl/spec/vector/bake/format.tsv`
is already there. Note that the `deno task vector` and `check:pair` tasks that
[where the pair differs](./20260914-where-the-pair-differs.md) names are no
longer in `deno.json`; the vectors survived the migration and their emitters
did not.

For this pipeline the coarser check is the better one anyway: bake the same
sample of real `page_idx` through both implementations and diff the bytes. It
covers the whole path — WKB decode, adapters, policies, normalizer,
rasterizer, encoder — rather than one function at a time.

## The order

1. **socket.io removed**, with `IO_PATH` and its `DEV_PROXY_ROUTES` entry.
2. **`crates/pxl/geo`** — axum, `/api/search`, the admin middleware, config,
   deadpool-postgres. `DB_GEO_URL` only; `main_pool` and `DB_MAIN_URL` are not
   carried over. `infra/prod/docker-compose.yml` and `infra/README.md` are
   updated when the deploy actually switches.
3. **`crates/pxl/geo-bake`** — the pipeline: WKB, the Overture and coast
   adapters, the two policies, the normalizer, the ground baker, the encode.
   Verified by byte diff against the TypeScript.
4. **The tool binaries** — `bake_tiles`, `bake_poi_tiles`,
   `bake_geocode_tiles`. This is where the move pays: the terrain bake walks
   the whole coverage single-threaded today and wants rayon with a connection
   per worker.
5. **The dev tile server binary**, taking over `module/tile/*`.
6. **`[profile.server]`** in `crates/Cargo.toml`.
7. **`pxl-api`** — when it has code. Its name is settled; nothing else about it
   is.
8. **The URL root split** — when there are two origins to route between.

Standing constraints for all of it: external dependencies stay out of
`pxl-spec` and `xpute-core`; no WASM-only assumptions in `pxl-kernel`.

## What steps 1 and 2 turned up

**The two servers answer the same, and the one difference was dead.** Both were
run against the same local geo DB and asked the same 15 queries — 311 results,
covering the plain and the `near`-biased ranking, division matches, the
name dedupe, the empty result and the 400s. Every field agreed: names,
categories, the resolved context arrays, ordering, and the coordinates to the
digit. The only difference was `address: null` on division results, which the
TypeScript still emitted and the client's own `SearchResult` has never
declared; the comment beside it already said the field's job had disappeared
rather than moved, so the port drops it.

One query differed on ordering and turned out not to be a difference at all.
`q=공원` matches three rows literally named 공원, so `ORDER BY
similarity(name, $1) DESC, name` leaves them exactly tied and Postgres returns
them in scan order — the _same_ server shuffles them between calls. Worth
knowing rather than fixing here: adding `gers_id` as a last tiebreaker would
make the order stable, and would also be the first deliberate behaviour change
of the port, so it is a decision rather than a repair.

**The framework is off the critical path, as expected, and now measured.**
Medians over 15 warm calls each, TypeScript against Rust: 46.3/42.1 ms,
51.0/47.4 ms, 555.8/590.0 ms, 618.6/584.1 ms, 563.4/599.9 ms. Both are
DB-bound and the spread is noise, which is the number behind choosing axum on
process ownership rather than throughput.

**A gate over an empty router gates nothing.** The admin middleware is
registered on the `/api/internal/*` prefix rather than on a list of routes, so
that whatever is mounted there later is behind the token by construction —
and axum's `.layer()` over a router with no routes of its own wraps nothing,
so the request fell through to the outer 404 without the middleware ever
running. An explicit fallback gives the layer something to wrap. Nothing is
mounted under that prefix today, which is exactly the condition under which
this is invisible; it was found by a test written to pin the claim, not by
reading.

## What step 3 turned up

**The pipeline agrees byte for byte, over 114 pages.** Both implementations
were fed the _same_ source packet rather than each querying for itself: the
source statements carry no ORDER BY, so two runs can see the same rows in a
different order, and a difference in the tile would then say nothing about the
pipeline. `dump_page_source.ts` writes a page's packet and the TypeScript's
tile; `cargo run -p pxl-geo-bake --example bake_from_packet` bakes the same
packet; the tiles are compared whole. 4,203,672 bytes each, identical for a
spread of fourteen across the coverage, sixty at random, and forty coastal —
the last picked deliberately, by running the coast query alone over 2,500 land
pages and keeping the 355 whose ocean does not cover the whole envelope, since
without them the sea fill, the carve and the land/water mask never run at all.

**A `double precision` column reaches the packet as a string.** The Deno
driver decodes those as text — the same thing forward search had to undo on
its own coordinates — so `overture_buildings.height` is a `str` node where the
row type says `f64`, while `num_floors`, an `integer`, is a number. The
TypeScript never noticed, because the only thing it does with the value is
`Math.max(0, x)`, and that coerces. Reading it strictly is what turned it up:
the first fourteen pages happened to carry no populated height, and twenty-two
of the next sixty failed outright. Nothing downstream reads the result —
`FeatureComputed::height` reaches no rasterizer — so this was a decode that had
to accept what the packet holds, not a difference in any tile.

**Two pieces of TypeScript machinery did not come across, and neither changes
a byte.** The normalizer's module-level `V3` arena exists to keep a per-page
burst of small objects off JavaScript's heap; a `V3` here is `[f64; 3]` by
value. The rasterizer's `XS_SCRATCH` is the same thing for its scanline
crossings. What did have to come across exactly is subtler: that the rank sort
is stable, so equal-ranked features keep the order their rows arrived in and
paint over each other the same way; that a typed array's `sort` is numeric
where a plain array's is lexicographic; and that `j` trails `i` by one around a
ring, which `j = i++` says and a first port gets wrong.

## What the source half turned up

**The same 114 pages come out identical end to end.** Step 3 compared the
pipeline on a packet handed to both; this is Rust running its own five
statements, packing its own packet and baking it — repository, source and
pipeline together — against the TypeScript's tile for the same page. Identical
for all 114, which also settles the row-order worry that made the earlier
comparison feed both the same packet: the statements carry no ORDER BY, but
two drivers against the same static data and the same plan see the same rows
in the same order.

**The packet itself is not byte-identical, and does not need to be.** A
`double precision` column reaches the Deno driver as text and this one as a
number, so `height` is a `str` node in one packet and an `f64` node in the
other; the typed absences a Rust view writes are not the untyped nils
`put(null)` lowers to either. What has to match is the tile, and the tile is
what the encoders both write from the same `Ground`.

**`PREPARE`/`EXECUTE` and its literal argument list did not come across.** The
TypeScript reaches for them because its driver re-parsed and re-planned an
ordinary parameterized query every call — ~34 ms of a bbox lookup's ~44 ms in
planning, cut to ~2–3 ms by preparing once per physical connection. A prepared
statement here is that, so the workaround and the `lit()` guard that made it
safe are gone rather than ported.

**A synchronous client, not tokio-postgres.** pxl-geo takes the async one
because it serves requests; a bake is five queries, a pack and a rasterize,
and the pack and the rasterize are `Rc`-backed throughout. On a blocking
client the page is one call that nothing has to be `Send` across, and the
concurrency is a thread per worker with a connection of its own — where an
async client would have wanted the whole `Rc` half inside `spawn_blocking`
anyway. (This replaces the earlier note about rayon: the shape is the same,
the reason is the client rather than the CPU.)

**107 ms a page against 263.** Twenty pages, one connection, no concurrency on
either side: the TypeScript takes 5.3 s and this takes 2.1 s. The tool's own
win is still ahead of it — walking the coverage is what wants workers — but
the per-page figure is what a worker multiplies.

## What the tool turned up

**The two tools write the same tile set.** Both were run over the same 200
land pages at the same concurrency: 201 objects each — the tiles and the
shared `_ocean` — and every one identical after decompression. The manifests
carry the same keys and the same counts (123,735 candidate pages, 43,505 land,
80,230 ocean, the same source catalogs and the same git sha).

**The gzip bytes are not identical, and should not be expected to be.** Two
encoders at their own default levels produce different streams from the same
input; what the client sees is the decompressed bytes, which agree. flate2's
default happens to come out 3.4% smaller than `CompressionStream`, so a
re-bake shrinks the set slightly rather than growing it.

**250 ms a tile against 45**, and the first figures taken for it were too
kind. `--limit n` takes the first n of a page list ordered by index, so a
sample drawn that way is one contiguous patch of the country; the first 200
land pages averaged 15.5 kB gzipped against 29.9 kB for 200 drawn at random,
which is to say they were half the work. On the random set, cold: 50.0 s
against 8.9 s, so over the 43,505 land pages about 3h against about 32m.

Run again with the pages warm in Postgres it is 6.5 s, and raising the workers
past 8 does not move it — 35.7, 32.3, 33.5 and 34.0 ms a tile at 4, 8, 16 and
24. Past that point the bake is waiting on the database, not on itself, which
is also why the single-connection gap (263 ms against 107) is smaller than
this one: a thread owning its connection and its prepared statements for the
whole run is most of what the rest buys.

Both numbers are extrapolations from 200 of 43,505 pages, which is 0.46% of
the set, and a real walk goes in index order rather than at random — spatial
locality should help it and a dense city should hurt. Only a full run says.

Where the time goes was then measured rather than assumed
([where the bake spends its time](./experiment/20260918-where-the-bake-spends-its-time.md)),
and it is not the rasterizer. `bake_terrain` is 64 ms a page against 29, a
factor of 2.2 and the smaller half. The `base` statement is 167 of the
TypeScript's 277, and 149 of that 167 is 3.2 MB of WKB arriving — the server
hands both clients the same rows in the same time, and a pure-TypeScript
driver then takes eight times as long to turn them into arrays. XTP costs 2.0
ms against 1.8, which is to say nothing. The rest of the end-to-end gap is
that JavaScript is one thread, so eight workers overlap the database waits and
serialize every byte of decode and every cell of fill.

**The ocean tile is still page-independent, and the tool still checks.** The
client keeps one decoded copy of `_ocean` for every open-ocean page, which
holds only if baking the same empty source at different pages — different
bbox, different `Z_SCALE` by latitude — gives identical bytes. The assertion
came across with the tool, and passes.

## What the dev tile server turned up

**All four routes answer what the TypeScript answers.** Both servers against
the same database, twelve pages each: terrain and POI byte for byte, geocode
equal as JSON, the same 400 for a page index that is not one, the same 404 for
an asset key that is not there, the same 401/200 on the invalidate route. The
cached second request was compared too, not only the first.

**The asset route in the TypeScript is broken in the built server, and has
been.** `module/tile/asset/service.ts` resolves `out/asset/` as six `../`
from `import.meta.url`, which is right for the file's own path under
`src/module/tile/asset/` and wrong for what actually runs: `deno task dev`
serves `dist/main/server.js`, where the same six steps land two directories
above the repository. An ingested model that exists on disk 404s. The port
takes the directory as a flag defaulting to `./out/asset`, relative to the
working directory the tasks already run from, which is how every other tool
here finds `out/`.

That bug is what `__IS_DEV__` costs, and it is worth separating from what it
buys. What it buys works: the production bundle is 268.8 kB against the dev
build's 313.4, and every string only the tile path carries —
`ST_MakeEmptyRaster`, `overture_buildings`, `api/tiles` — is absent from it
(`overture_places` survives, because forward search really does query it). The
branch is dead and the bundler drops what only it referenced.

What it costs is that the dev-only code still has to live inside the
production server: its source tree, its dependency set, its build, and — the
part that bit — its runtime file layout. `out/asset/` was written as six `../`
from a module under `src/module/tile/asset/`, and that module only ever runs
from inside `dist/main/server.js`. The code was eliminated; the coupling was
not. A separate binary in a separate crate has neither: production does not
link it, there is nothing for a bundler to prove dead, no flag that can be set
wrong, and no shared layout to resolve against — and it can take axum and
tokio, which a module gated inside the API's own bundle never could.

**`pxl-overture` exists now, and the category map's home is settled.** Forward
search and the POI bake both translate an Overture category into one of ours,
and a badge on the map and a row in the search list have to draw the same icon
— so the table cannot be duplicated. `pxl-spec` was refused for it: the
TypeScript keeps it on the provider side rather than in `@pxl/shared`, and an
Overture vocabulary is not the spec. The crate turned out to hold more than
the one table: `GeocodePart` and the two subtype ladders were also the same
lists in two files, once the reverse geocode came across.

## Where it stands

**Steps 1–6 built** 09-18, verified against a live geo DB as above.
`crates/pxl/geo` is 954 lines including its tests, takes `DB_GEO_URL` alone,
and `apps/api` still serves — the Dockerfile, the compose file and
infra/README change when the deploy cuts over, not before. `check:rust` is
three passes now: the client's crates for wasm32 by name, the server's for the
host, then the host check with every target.

What is left of the move: `bake_poi_tiles` and `bake_geocode_tiles`, which now
have their providers in Rust and want only the walk around them, the way
`bake_tiles` did; and then the cutover itself — the Dockerfile, the compose
file and infra/README, which change when the deploy switches and not before.
7 and 8 wait on a second server existing.

The dev tasks still start the Deno server. `deno task dev` is `dev:api` and
`dev:web`; pointing it at `pxl-geo` and `serve_tiles` is part of the cutover,
because until then the TypeScript is what the byte comparisons run against.

Still open, and now to settle in step 4: the Overture category map. It is
search's and the POI bake's, and the terrain pipeline never touches it, so step
3 did not force the question after all — the second consumer arrives with
`bake_poi_tiles`. `pxl-spec` is out: the TypeScript keeps the table on the
provider side rather than in `@pxl/shared`, and an Overture vocabulary is not
the spec. That leaves a small crate of its own, decided when there are two.
