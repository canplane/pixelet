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

The proposal was to split the GIS-flavored serving from the ordinary
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
`deno.jsonc` already draws that line ("Not usable for bake work: a published
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

Both were settled the same day, in the other direction — see _The frame_ below.

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
longer in `deno.jsonc`; the vectors survived the migration and their emitters
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
make the order stable, and would also be the first deliberate behavior change
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

**`pxl-geo-spec` exists now, and the category map's home is settled.** Forward
search and the POI bake both translate an Overture category into one of ours,
and a badge on the map and a row in the search list have to draw the same icon
— so the table cannot be duplicated. `pxl-spec` was refused for it: the
TypeScript keeps it on the provider side rather than in `@pxl/shared`, and an
Overture vocabulary is not the spec. The crate turned out to hold more than
the one table — the two Divisions subtype ladders were also the same lists in
two files, once the reverse geocode came across.

It briefly held `GeocodePart` as well, and that was wrong. A crate named for a
provider holds what the provider's values mean; `GeocodePart` is a shape _we_
answer in, and swapping the provider would keep it while changing every ladder
beside it. It was there because the crate was the only shared place allowed a
serde dependency, which is a reason about the build and not about the
boundary. Each server defines its own now, and they are not the same type:
the reverse geocode's carries `approx` for a part that came from a nearby
point, and search's never does. Nothing is lost by the duplication that the
client would not see immediately — the shape is three optional fields — while
what a divergence in the category table or the ladders would cost is a place
drawing different icons in two parts of the UI, which is why those are still
shared.

## The category table is generated now, and checked against the theme

Porting it left two hand-maintained copies of the same 61 entries, one in
TypeScript and one in Rust. They agreed — checked, key for key — but that is a
thing that stays true until it does not, and the repository's own model is that
a name is written from JSON rather than decided in a language.

So `spec/overture/category.json` is the table, and `script/spec.ts` writes both
sides from it. The Rust is a static sorted by the Overture name, looked up by
binary search; the TypeScript is the same object it was. Neither can drift from
the other any more, because `check:spec` already fails on a generated file that
is not what its JSON writes.

The generator also checks the thing nothing was checking: every target the map
names has to be a key of `spec/kernel/layer/poi/theme.json`, and every key of
that theme has to be named by at least one entry. They were exactly 36 and 36
already. Both directions were broken deliberately to confirm the gate fires:

    spec/overture/category.json maps to 1 category the POI theme does not know:
    hospitaal. A POI in one of them draws the default badge and says nothing —
    add it to spec/kernel/layer/poi/theme.json or map to a key that is there.

    spec/kernel/layer/poi/theme.json has 1 category nothing maps to: aquarium.
    A theme entry exists because some Overture category resolves to it — add the
    mapping in spec/overture/category.json or drop the entry.

The first is the one worth a gate: a POI whose category the theme does not know
falls back to the default badge and reports nothing, so the only way to notice
was to look at the map and recognize the wrong icon.

## The POI and geocode bakes, and what they did not gain

Both write what the TypeScript writes. Over the same 300 random land pages:
the same file set including the `.empty` markers, 158 POI objects byte for byte
after decompression, 300 geocode objects equal as JSON, and a resumed run
skipping all 300.

**They are barely faster, and that is the interesting part.** POI 0.48 s
against 0.26, geocode 0.56 against 0.37 — 1.8x and 1.5x, where the terrain bake
was 5.6x. The reason is the same one the phase timings gave: terrain's per-page
cost is 3.2 MB of WKB arriving and a rasterizer running over it, and these two
have neither. A POI page is one indexed point query and a few thousand short
strings; a label is two or three. Almost all of what is left is the database
answering, which is the same database either way. Over the full coverage both
come to about a minute.

So the move buys these two nothing on the clock. What it buys is that they stop
being the reason Deno has to be installed to publish a tile set — which is the
whole of why they were ported.

**One walk, not three.** The two are the same shape — terrain's land-page list,
one object per page, an `.empty` marker where there is nothing, resume by what
is on disk — so that is `walk.rs` and each tool is about 120 lines of what is
its own. `bake_tiles` deliberately does not go through it: its walk is over a
candidate grid it classifies itself, it has the shared ocean tile, and it has no
`.empty`. Folding it in would have bent all three.

Two deliberate differences from the TypeScript's manifests, both additions: the
page manifests now carry `git_sha` and an `objects`/`gz_bytes` measured from
disk, which only terrain's had. The count excludes the `.empty` markers, since
they are local to a resume and never uploaded.

## The cutover

What a deployment runs is `crates/pxl/geo` now, and what `deno task dev` starts
is that plus `serve_tiles`.

**The image is the Rust binary and 51 MB.** `apps/api/Dockerfile` keeps its path
— that is where the compose file points and where the deploy procedure names it
— and builds none of apps/api. Two stages: `cargo build --profile server -p
pxl-geo` over the whole `crates/` tree (the workspace has to resolve, so it is
copied whole), and a distroless runtime with glibc and nothing else. No shell,
no package manager, no CA bundle: the only thing it connects to is db-geo, over
the compose file's own network and without TLS, so there is no certificate to
verify. Built and run for real against the live database before this was
written — 160 MB on `debian:trixie-slim` with ca-certificates, 51 MB without
either.

**db-main is out of the server's environment.** It stays provisioned and
running, because the application server will own it; nothing reads it until
then. `main_pool` had been created, probed with `select 1` and queried by
nothing since before this started.

**The dev proxy splits where the edge already splits.** Two local servers under
one root — `pxl-geo` on 3000 for search, `serve_tiles` on 3001 for the tiles —
and the proxy sends `/api/tiles/` to the second and the rest of `/api/` to the
first. That is the Worker's own split with the bucket's side baked on demand
instead of read, so dev mirrors production rather than inventing a shape. It
reads a prefix and never a route, which is what
[one server root](./20260910-one-server-root.md) asks of a relay.

The `/api` + `/geo` root split stays where it was: deferred. With one origin in
production there is nothing to route between, and moving published tile URLs
costs a cache generation for nothing.

**`cargo run` does not read `.env`**, where `deno run --env=.env` did, so the
tasks load it and forward their arguments. `bake:tiles` now runs the Rust
binary, and `bake:poi` and `bake:geocode` exist beside it — the two had no tasks
before and were invoked by full path.

**Nothing was deleted.** `apps/api`'s TypeScript server still builds, still
type-checks and still runs if started directly; it is what every byte
comparison in this document was made against, and removing it is a separate
commit for after the Rust has served for a while. What that leaves in the tree
is dead code with a reason, which is worth saying out loud so it does not read
as an oversight.

## The dev tile server behind a feature, and what it was actually worth

`serve_tiles` is `--features serve` now, and its axum and tokio are optional
dependencies, so `cargo build -p pxl-geo-bake --bins` builds the three bake
tools and skips the server. A build that asks for it without the feature says
so — `target serve_tiles in package pxl-geo-bake requires the features: serve`
— rather than failing obscurely.

The saving is real and smaller than the figure that motivated it. Clean builds
into an empty target directory:

|                      | crates |   wall |
| :------------------- | -----: | -----: |
| bake tools alone     |     74 | 23.7 s |
| plus the tile server |     98 | 32.0 s |

24 crates and 8.3 s, not the 51 crates axum's subtree has: 25 of those 51 are
already pulled in by `postgres`, which every bake tool needs anyway. Quoting
the subtree's own size overstated the marginal cost by about half, which is
what measuring it was for.

It costs something too. `check:rust` now lints the bake crate twice, once in
each configuration, because a lint run under only one of them leaves the
other's code unlinted until it breaks — the failure mode a feature gate has
that a separate binary does not. Warm, the whole Rust gate is still under a
second.

Worth keeping, narrowly: what it buys is a bake-only build not compiling a web
framework, and the machines that would feel that are a CI job or an image that
carries the tools alone. Neither exists yet.

## The frame

The split above was argued as something to do at deploy time. It was built
instead, and the reason is that a deployment is the worst moment to first
discover what the addresses are: the Worker, the client bundle, the Caddyfile
and two servers all have to agree, and the ones that disagree fail in
production rather than in a check. Meet in the middle — put the frame in place
while nothing depends on it, so the pieces that arrive later arrive into
something.

**Three roots, not two.** `asset/` came out as its own. Models and textures are
content-addressed ingest output; under `/geo` they would be claiming to be
geography, and under `/api` they would be claiming to be state. They are
neither — they are bytes named by their own sha256, which is as immutable as a
URL gets. `asset_url()` returns `/<key>` now because the key already begins
`asset/`.

```
Worker:  /geo/tiles/* → R2   /asset/* → R2   /geo/* → GEO_ORIGIN   /api/* → API_ORIGIN
Caddy:   /geo/* → pxl-geo    /api/* → pxl-api
dev:     /geo/tiles/* → 3001 /asset/* → 3001 /geo/* → 3000         /api/* → 3002
```

**The addresses are generated.** `spec/route.json` is the one place the roots
and the branches are written, and `script/spec.ts` writes three files from it —
`apps/web/src/config/route-spec.ts`, `apps/web/worker/route-spec.ts` and
`crates/pxl/kernel/src/shell/config/route_spec.rs`. They had been five
hand-kept copies before, which is four too many for a constant that only ever
fails at the edge.

Two of them cannot be generated, so they are checked instead. `wrangler.jsonc`'s
`run_worker_first` is Cloudflare's own config format and `check_bundle.ts`'s
sentinels are the deploy guard's; the generator compares both against the spec
and throws when they drift. It earned that on its first run, before any of this
was wired:

```
apps/web/wrangler.jsonc's run_worker_first is ["/api/*"]
where spec/route.json's roots are ["/geo/*","/api/*","/asset/*"]
```

What is deliberately _not_ generated is a server's own routes. `pxl-geo` spells
`/geo/search` and `pxl-api` spells `/api/internal` in its own source, because
adding a route is a change to one server and to nothing else — that is
[one server root](./20260910-one-server-root.md)'s whole point, and reading
route names out of a shared file would undo it. Only the relays are written
from the spec, since a relay is the thing that must agree with everyone.

**pxl-api exists, and answers nothing.** It is the same frame the TypeScript
had: a process that connects to db-main, probes it with `select 1` at boot, and
reserves `/api/internal/*` behind the admin token with a `.fallback()` under
the gate so the prefix is covered rather than a list of routes. `main_pool` had
been created and queried by nothing in the Deno server since before this
started; dropping it here would have meant putting it back — along with the
root, the origin, the container and the secret — on the day the first route is
written. What does not belong in it yet is a command schema or a payload shape;
the order those come in is
[point density and the order to multiplayer](./20260910-point-density-and-the-order-to-multiplayer.md)'s,
and an address space is not a carrier.

**Each server reads its own root's variables.** `GEO_HOST`/`GEO_PORT` and
`API_HOST`/`API_PORT`, defaulting to 3000 and 3002. Sharing `API_PORT` between
them, which is what the first cut did, collides the moment both run locally; in
a deployment each has a container to itself and both listen on 3000 again.

**A misroute worth recording, because it was not in the code.** `/api/*` kept
reaching 3000 after every file said 3002. The cause was `apps/web/.env`'s
`VITE_API_BASE_URL`, a single origin from when there was a single server, which
the dev proxy read in preference to its default. It is three variables now —
`VITE_GEO_BASE_URL`, `VITE_TILE_BASE_URL`, `VITE_API_BASE_URL` — one per thing
that can be pointed somewhere else. A constant moved out of the source into a
gitignored file is a constant no gate can check, which is the general form of
what went wrong.

Verified end to end through vite with all four processes up:

```
/geo/search?q=강남                    200   7,693 B
/geo/tiles/terrain/6361774393589760   200   4,203,672 B
/geo/tiles/poi/6361774393589760       200   184 B
/geo/tiles/geocode/6361774393589760   200   248 B
/asset/0831346f…                      200   148,892 B
/geo/internal/x                       401
/api/internal/x                       401
/api/anything                         404
```

## Where it stands

**The move is through, and the frame is in.** `crates/pxl/geo` answers `/geo/*`
against db-geo, `crates/pxl/api` answers `/api/*` against db-main and has no
routes, `serve_tiles` bakes `/geo/tiles/*` and serves `/asset/*` on demand
behind `--features serve`, and `deno task dev` starts all four processes.
`check:rust` is four passes — the client's crates for wasm32 by name, the
server's for the host, the bake crate again with its feature, then the host
check with every target — and `check:spec` is what keeps the addresses honest.

**Deployed 09-18**, server then client, as build b15. Published tile URLs moved
from `/api/tiles/…` to `/geo/tiles/…` and `/asset/…`; R2 keys did not change —
only the prefix stripped to reach them — so there was no re-bake and no
re-upload, and the cache went cold once. Every route answered afterward, both
at the edge and against the origin directly:

```
/                                       200   3,193 B (the app)
/geo/search?q=분당                       200   7,593 B
/geo/tiles/{terrain,poi,geocode}/…      200
/asset/e8d81b97…                        200  18,188 B  model/gltf-binary
/geo/internal/x                         401  (pxl-geo's gate)
/api/internal/x                         401  (pxl-api's)
/api/anything                           404
/api/tiles/…, /api/search               404  (the old addresses)
```

Two things the deploy itself taught, both worth more than the move:

**The bundle guard stopped the first attempt, and it was right.** `deno task
deploy` failed with `sentinel "/geo" missing from the bundle`. The old
`'"/api"'` sentinel had survived because `API_ROOT` was a constant the client
spelled on its own while `TILE_ROOT` was a template literal esbuild left
unfolded — the root reached the bundle and the branch never did. Generating the
branches folds them at spec time, which reverses it exactly: `"/geo/tiles"` and
`"/geo/search"` reach the bundle and nothing spells `"/geo"` alone. The spec
carries a `bundle` flag now, and the generator compares SENTINELS against it in
both directions, since a sentinel the build never emits fails every deploy and
one the build dropped passes a guard that is checking nothing.

**Caddy's config is a bind mount, and a bind mount is read at start.** After the
server rebuilt, `/geo/*` answered correctly while `/api/internal/x` returned 404
instead of 401 — the request was reaching pxl-geo, not pxl-api. `docker compose
up -d --build` had not recreated caddy, because from compose's side neither its
image nor its service definition had changed. `--force-recreate caddy` was what
made the Caddyfile current. A missing container would have been a 502; a 404
from the wrong server is what a stale relay looks like.

**Deleted 09-18, after this had served for a few hours:** `apps/api` entirely.
38 files — the server and the bake every byte comparison in this document was
made against — and the 14 that were never part of the move (the Overture and
asset ingests, the brand renderer, the texture-pack tool, the elevation
pipeline) went to `tool/`, a root of its own because nothing under it is
deployed. See [the gate that deploys](./20260918-the-gate-that-deploys.md). The Overture ingests, the
asset ingest and the brand renderer stay in Deno for the reasons under _What is
not moving_, and those are not waiting on anything.
