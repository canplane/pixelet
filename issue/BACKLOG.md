# Backlog

What is queued, in priority order, so that work follows the list rather than
whatever was last noticed. One line of context each; the reasoning lives in
the discussion the line points to. A finished item is removed, not ticked —
its record is the discussion and the commit.

Priorities: **P0** now, **P1** next, **P2** when nearby, **P3** recorded so
it is not rediscovered, **parked** decided-not-now with the reason on file.

## P0

- **A view's level from the eye, in 3D, by one rule.** A node splits while
  its nearest distance from the camera is under `r_L = (P / τ) · 2^L`, one
  rule for every layer, so a cell's pixels and a view's node count are held
  between constants whatever the tilt or position; the window's rings
  around the map center go
  ([20260925-what-a-view-costs-is-the-eye-s.md](./20260925-what-a-view-costs-is-the-eye-s.md)).
  Built for terrain and places. Settled 2026-09-25: the rule is shared and
  chooses a level — a zoom — for each piece of ground by its distance from
  the eye, below z16 too (z17 to z19 near the eye); what a layer draws at
  that level, and in how much detail, is the layer's own, a tuning inside
  the perspective rather than a second rule. At a level L terrain draws its
  cells averaged 2 × 2 (detail +1) — blended by coverage for the smoother
  ground it draws, not level L + 1's majority page, though that would fetch
  a quarter of the pages — places count in a lattice sixteen times
  the ground's cell (+4) and still stand a lone place on its z16 cell, and
  objects draw their own cells (0), so an object's cube and the ground's are
  the same size wherever both have data. Where the rule asks finer than a
  layer's data, the layer draws its finest: terrain z16, objects their
  sub-cell tiers to z19. z16 is terrain's atom and nothing more. What is
  left:
  - **The split distance from the real screen.** `SPLIT` is the reference
    1080 px and 50° (pxl-render look/lod.rs); `P` is to come from the
    canvas's device pixels and the field of view, and where a screen needs
    more than the tables are sized for, τ rises rather than the rule
    breaking. Planned 2026-09-25, held. Measured on an iPhone 13 mini
    2026-09-27 (a debug build, the ratio fixed at 2): the GPU's time
    follows the triangles and not the pixels — 6 ms at a quarter of a
    million, 14 at half, 26.6 at the 2.07 million the view drew for most of
    the window — so a frame fits under half a million, and 78% of frames
    ran past 16.6 ms. A turn past 16 ms came in a frame the GPU ran long in
    532 times of 537: the kernel waits for the layer's next drawable, and
    on a device the JavaScript thread waits with it.
  - **What a crossfade leaves.** A change of what a chunk draws dissolves
    over 300 ms of frames (chunk/table.rs `Fade`), a cube for another and a
    stale chunk for what replaced it, its decor with it on the same half of
    the pattern (pxl-render dissolve.wgsl). The 300 ms is a number chosen by
    eye, to be measured. No hysteresis: the rule is a function of where the
    eye is, so a resting camera changes nothing, and the budget's pressure,
    the one input that moves on its own, steps on timers of its own. Above
    z16 a kind's footing and a transport-aligned kind's heading are read off
    the level's cells, so an instance can appear, go or turn at a level
    change, as the ground under it does.
- **What is drawn is what the screen covers: to main, and closed.** Built
  2026-09-28 on `observation-paging`, pushed, not on main
  ([20260928-what-is-drawn-is-what-the-screen-covers.md](./20260928-what-is-drawn-is-what-the-screen-covers.md);
  ARCHITECTURE.md "The view"). The client asks for terrain v7, so main
  waits for its publish. In order:
  - **Before main.** The dev tile server's 500 on some pages under its top
    level (a missing file in the demand fold) fixed, and the offline bake
    checked for the same; terrain v7 baked and published after
    `deno task usage` (about 227,000 Class A); a device build run; then
    merged with `--no-ff` and deployed.
  - **Closing the model.** The domain's reading by column becomes a query
    of a 3D box, so height is nowhere special above a layer. The query's
    walk skips by the index's extent a level at a time, so a ray costs a
    logarithm, and an `Observer` on the GPU can cast every tile at once.
    Record demand from hits rather than bounds: from street level
    `ByBounds` asks for about 37 pages where 3 are seen. Places and decor
    stop reading heights off the mesh (`Scheduler::surface_at`). Residency's
    table, and what the domain, the frontier and the observer allocate per
    rebuild, move off the heap. The arena is sized to the smaller goal.
    Objects get records a level and an index of their own beyond the
    store's reaches. The hard-held chain's test returns with the first
    consumer that leases.
- **How the samples are drawn.** Raster against casting into the records,
  on the iPhone 13 mini, with today's box format (36 unindexed vertices)
  against faces alone; seams where levels that are not neighbours meet, and
  a blend on arrival; decor as its own solver — instances at the asset's
  level of detail, an impostor far away, its lifetime off the chunk slot.
- **A blank screen after a far search from a zoomed-out view.** Location
  pending, 30 ticks of zoom out, search "아주대학교", the first result:
  nothing is drawn and the frame count stops (2 of 2 under dev:local,
  2026-09-25). The first requests after the jump go to the world origin
  (`eye()` on the stale origin). A lead not yet followed: `serve_tiles`
  answers nothing past `--top-level` (3 by default; z11 and coarser are 503
  for terrain and POI alike), so a view that draws nothing finer than
  level 4 has no page it can get.
- **One order out from the eye, the rest.** Every scheduler takes work in
  one order out from the eye, a chunk is a cube of its level holding
  something, and a placed object lives in the cube its band is in
  ([20260926-the-view-is-cubes-about-the-eye.md](./20260926-the-view-is-cubes-about-the-eye.md)).
  Left: occluded chunks are split, fetched and built like seen ones; a cut
  ceiling flashes on a cold load while a coarser page stands in; and a
  column crossing a cube's floor or ceiling is two chunks — 1.5 a column
  near Gangnam and 1.7 on Bukhansan, 2026-09-26 — each scanning the whole
  column's runs again, where the runs could be the column's once and each
  chunk a cut of them.
- **A seam while a chunk is drawn from a page over it.** Since
  2026-09-26 a chunk whose page has not come is drawn in its place from the
  nearest resident page over it, upsampled (world/pyramid.rs `drawn_from`),
  and its apron is read from that same page. A neighbor already built from
  its own page reads its apron from its own, so the two sides of their edge
  are two surfaces, the coarser one smoothed, and at a cliff or a coast they
  part by more than the one cell a skirt hides. Measured after a jump to a
  coast: brown walls about the character for a second, gone by the second
  after as the pages landed. What would close it: an edge read from one
  source while either side is borrowed — the apron from the neighbor's own
  source, and a neighbor's source among a chunk's build sources.
- **The move to a Rust core in WASM, drawing through WebGPU.** Scheduled
  2026-09-13 ([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md))
  and through by 2026-09-15: the kernel (`crates/pxl/kernel`, built by
  `crates/pxl/wasm` into one module whose exports are `boot` and
  `interrupt`) builds and schedules the chunks, holds the world, the camera,
  the map, the stores and the fetch scheduler, and lists what to draw; the
  shell forwards events, runs fetches and loads, and executes the draw list
  through WebGPU
  ([20260914-what-stays-in-javascript.md](./20260914-what-stays-in-javascript.md)).
  How it got there is that document,
  [the audit after the move](./20260914-the-audit-after-the-move.md),
  [the safety valves](./20260914-the-safety-valves-the-module-did-not-have.md)
  and the commits. What is left of it:
  - **The frame-based performance pass.** `BakeContext::step` itself (face
    masks and the mesh write), and a large-block region if refusals show
    again. At least considered: work that is data-parallel per element and
    whose result only goes to the screen, moved into compute — culling into
    an indirect draw buffer (`drawIndirect`) in place of a listed draw a
    chunk, decor instances filtered by tier and range, and a chunk's face
    culling. What the CPU has to read back stays the module's: a readback
    arrives frames late, and WGSL has no 64-bit type for the coordinates, no
    allocation and no I/O. The mesher's face culling is the open question,
    since the raycast and the caches read what it produces.
  - **Open, minor:** one POI page (6362873910329344) read as a truncated
    packet on one machine under dev:remote, the fetched bytes whole in the
    console — into_core's view the suspect, not reproduced elsewhere.

- **The last TypeScript that was not the shell.** 2026-09-19
  ([the doc](./20260919-the-last-typescript-that-was-not-the-shell.md)). In:
  the five Overture ingests into `pxl-geo-bake`, beside the bakes that read
  what they write, verified against the TypeScript at one bbox over nine
  tables with the geometry as WKB — which is how three float bugs came out
  that reading could not have found; the asset tree's tooling as
  `pxl-asset-bake`, where the WebP sheet turned out byte-identical and the
  sixty tiles were re-normalized and re-uploaded; and `@pxl/shared` dissolved
  to nothing, its thirteen surviving files moved beside the shell that draws
  with them. `packages/` is `@xpute/core` alone and `tool/` is bash and GDAL.
  The line that decided each piece: what a thing **is** is generated from
  `spec/` into both languages, how the UI **arranges** it is hand-written
  beside the list it is about.

- **CI/CD, and what is left of `apps/api`.** 2026-09-18
  ([the gate that deploys](./20260918-the-gate-that-deploys.md)). In: one
  workflow with a gate (`check`, `test`, `lint`) and two deploy jobs behind it,
  gated by what each costs to get wrong — the edge on every green push to main,
  the VM only when `crates/`, `infra/prod/` or the Dockerfile changed; a smoke
  check that builds its URLs from the generated constants rather than a path
  list; the monthly refresh moved onto the Rust bakes, which it had never left.
  Done: the secrets are set, the whole workflow ran green end to end, and the
  monthly refresh was triggered by hand rather than left to the cron.
  `apps/api` is gone — 38 dead files deleted, the survivors moved out, each
  Dockerfile beside its own crate.

- **The server to Rust, and `apps/api` into three.** Decided and begun
  2026-09-18
  ([the server in Rust](./20260918-the-server-in-rust-and-the-line-already-drawn.md)).
  In: socket.io out; `crates/pxl/geo` answering `/geo/search` off
  `DB_GEO_URL` alone and agreeing with the TypeScript over 311 results against
  the same database; `crates/pxl/geo-bake` carrying the terrain pipeline and
  baking 114 pages byte for byte what the TypeScript bakes, coastal pages
  among them, and the same 114 again end to end once it read its own source;
  `bake_tiles`, writing the same tile set as the TypeScript's at 45 ms a tile
  against 250; `serve_tiles`, the four dev tile routes over the same
  pipeline, answering what the TypeScript answers; `pxl-geo-spec` for what
  both servers know about Overture's Places and Divisions; and
  `[profile.server]`; and `bake_poi_tiles` and `bake_geocode_tiles` over one
  shared walk, writing what the TypeScript writes; and the cutover — a 51 MB
  distroless image of the Rust binary, db-main out of the server's
  environment, and `deno task dev` starting the servers with the proxy
  splitting on a prefix the way the Worker does; and the frame — three roots
  (`/geo`, `/api`, `/asset`) written from `spec/pxl/spec/route.json` into the client,
  the Worker and the kernel, with `wrangler.jsonc` and the deploy guard
  checked against it rather than generated, and `crates/pxl/api` on db-main
  answering nothing behind a gated `/api/internal/*`, the shape the
  TypeScript already had. Deployed 09-18 as b15, server then client, every
  route verified at the edge and against the origin. Left, and not part of the
  move: removing apps/api's TypeScript server once the Rust has served for a
  while. The reason is not throughput: `pxl-spec` already carries the
  encoders only the server uses, so one source can hold both ends of the
  wire format instead of two. `apps/api` is three workloads wearing one
  process, and they split along a line `infra/` had already drawn — one
  database each: `pxl-geo` (search, db-geo's three tables), `pxl-geo-bake`
  (the pipeline, the tool binaries, the dev tile server; db-geo whole,
  offline), and `pxl-api` (db-main, sessions and the realtime half).
  That the line was never crossed is not a guess — `main_pool` is created,
  probed with `select 1`, and queried by nothing. In order: socket.io out
  (dead on both ends); `pxl-geo`; the pipeline, verified by baking real
  pages through both implementations and diffing the bytes, since the
  offline tool and the live endpoint are required to agree byte for byte;
  the tool binaries, which is where the move pays — the full terrain set
  extrapolates to about 32m against about 3h; the dev tile server;
  `[profile.server]` for `panic = "unwind"`, since the workspace's `abort`
  would end a bake on one bad page and there is no catching it. The
  Overture and asset ingests stay in Deno, and so does the TypeScript pair
  — the byte diff needs both sides alive. Two standing constraints out of
  it: external dependencies stay out of `pxl-spec` and `xpute-core`, which
  have none and one, and `pxl-kernel` takes no WASM-only assumption, which
  is what keeps a server running the kernel as the authority possible
  later.

## P1

- **The move to React Native, the rest.** Built for iOS 2026-09-26
  (apps/native, crates/pxl/web/native); what is left of the move itself —
  what the device costs to run is the budgets' items, not this one:
  - **Android and Quest.** The Kotlin half of `modules/platform` — the
    surface on a Vulkan layer, the kernel's memory and doorbell, storage,
    text through Android's own engine, the compass — and the Android
    targets in `build-native.ts`; the kernel, the renderer and the shell
    take them as they are. Once the shell has settled.

- **One byte budget over everything drawn, paid for in LOD.** Its
  pressure is to go (P0, the view's cost derived from its rule): a resident
  byte derived from the view leaves no refusal to pay for. What stays is
  the device's own ceiling, the arena and wgpu's allocations. The phone's
  page dies because the working set exceeds a budget of about two
  gigabytes, and a WebView (Capacitor) runs in the same WebKit content
  process under the same limit — reported by others as a 2048 MB jetsam
  ceiling on iPhone, consistent with the mini's 1.6–2.1 GB. Today's
  `RETAIN_BUDGET_BYTES`
  ([20260912-what-to-render-when.md](./20260912-what-to-render-when.md))
  covers only what is held off screen; what is on screen has no ceiling.
  The precedent is CesiumJS: a tileset has `cacheBytes` and
  `maximumCacheOverflowBytes`, and when the tiles a view needs exceed both
  it raises the screen-space error until they fit — the view gets coarser,
  the page does not die
  ([Cesium3DTileset](https://cesium.com/learn/cesiumjs/ref-doc/Cesium3DTileset.html)).
  On a device the same ceiling is the app's: a debug build on the mini
  still took the system's critical memory warning once, seconds after
  start, with the kernel's region mapped rather than written (2026-09-27),
  and a device's heap row counts wgpu's own allocations beside the
  kernel's (crates/pxl/web/native/src/memory.rs).
  MapLibre sizes its tile cache from the viewport rather than a count
  ([MapOptions](https://maplibre.org/maplibre-gl-js/docs/API/type-aliases/MapOptions/)).
  The same lever exists here: the one LOD criterion's distance thresholds.
  First seen on the desktop, 2026-09-14, after a zoom through a city:
  on-screen work 158 MB against the 128 MB budget, the mesh section 181 of
  256 MB live and the rest lost to the buddy pool's rounding (a request
  takes the power of two above it), off screen nothing left to reclaim,
  1,091 chunks pending on tiers whose blocks the pool refused, for the rest
  of the session — the bake slot held by one of them, so nothing else baked
  either, and the tick spinning a hundred milliseconds a frame over the
  refusals. Built the same day: a refused block ends the tick's builds and
  commits, the refused chunk gives up its slot and is not tried again until
  the pool gives a block back (`PoolStat::releases`), and the budget is paid
  in LOD — the ladder's pressure (lod.rs): a step in when a block is refused
  and reclaim found nothing, at most every half second, up to three tiers; a
  step out after five seconds without a refusal with the section off its
  mark; the profiler's `lod ... pressure`. Still open: what the rounding costs
  the section (about a third at the sizes tiers come in) — size classes
  between the powers of two, or a large-block region — once the pressure
  has been seen doing its work.

- **XTP: a string-sequence leaf.** A `str` sequence leaf — one UTF-8 blob
  plus a u32 offsets lane — in place of one `str` node per string. A new
  type code, so existing packets still read. Per string today is an 8-byte
  header, a NUL and word padding (the POI repack: 60 MB gzipped against
  51 MB of JSON) and one node object at encode.
- **XTP encoder without per-leaf objects.** A node object per leaf and a
  closure per scalar at encode; mechanical, format-unchanged. The bake is not
  a beneficiary and should not be used to justify it: packing a page's source
  is 2.0 ms in TypeScript against 1.8 in Rust
  ([where the bake spends its time](./experiment/20260918-where-the-bake-spends-its-time.md)),
  because a page is four large typed arrays and a few thousand short tuples —
  the per-leaf cost is real where the packets are tiny and the call rate is
  millions, which is the rings. Measured on
  the rings (2026-09-14,
  [experiment/20260914-xtp-by-fixed-shape.md](./experiment/20260914-xtp-by-fixed-shape.md)):
  a ray query is 709 ns through TreeView and TreeReader and 215 ns with the
  request written by shape; the request's encode is 492 ns against 54, a
  reply's read 303 ns against 5. The shape of it is a writer and reader
  per command generated from `@pxl/shared/spec/cmd.ts`, over XTP, for both
  sides — the schema on the format, which also checks the packets' types.
  The pools raised it (2026-09-15): a block acquired and released is
  1,752 ns through the rings against 202 through the exports, a three-word
  packet 488 ns of it each way, on a path that runs per block.

- **Placed objects come back moved after a reload.** Found 2026-09-14: a
  wall of fifteen cubes (five along X, three up Z) reloads as thirteen
  cells on one Z layer spread along Y. In since v7.

  The cause first written here — that `local_world_to_canonical` is not the
  inverse of the page-local offset `encode_objects` writes — does not hold
  against the code as it stands (read 2026-09-17, by reading only; the
  symptom has not been reproduced since). Five links of that path were
  checked and each is symmetric:

  - the page base is `FROM_L3_IDX(page_idx)` on both sides, and `page_idx`
    is written and read as the same `u64`;
  - the fixed-point identity holds. `pvf3::to_real(pvf3::X(v))` is
    `(i * ONE + f) / ONE`, `ONE` is `2^FRAC_W` and an fvec lane is an
    unsigned residue in `[0, ONE)`, so it equals
    `projection::unproject_comp(i, f)` — which `project` inverts;
  - `ivec::sub` is per-lane with sign extension (`as_int_n`), not a packed
    subtract that could borrow across lanes;
  - the anchor is the same PVF3 on both sides: the encoder writes
    `entry.corner()` and `write_object_value` takes the corner as given;
  - `lower` and `corner` are inverses — `pvf3::sh` by `-OBJECT_LAYOUT.min`
    one way and `+min` the other.

  What is not ruled out: `x`/`y`/`z` are `f32` lanes in the saved form
  (`object/store.rs`), so a coordinate's precision is f32's at page-local
  magnitudes; and a chunk's entry lanes hold one entry per cell for a leaf
  larger than one, each carrying `entry.size`, so what a large leaf is on the
  way back out is worth checking. Fifteen becoming thirteen is two placements
  landing on one cell, which is what either of those would do.

- **Strings out of the kernel's records.** Most of the kernel's `String`s
  name something rather than hold text — a task's key is a category and a
  page, its error a status — and a record holding one cannot sit in a lane of
  a section, so the profiler's task ring stays on the module heap while its
  frame ring moved out (2026-09-15,
  [the heap all of it runs in](./20260915-the-audit-against-the-rings-checkpoint.md)).
  A name becomes numbers (a category index, a page index, `fnv1a64` of what
  is not one); text that is data — a query, a place's name, an error's detail
  — goes as bytes into a UTF-8 lane with an offset lane beside it, the shape of
  a string-sequence leaf; what is built
  to be handed to the shell once (a report, a log line, the snapshot) stays
  on the heap.

  Done 09-16/09-17, in the records the shell reads: a `DemandKey` is a
  category and a `u64` subject, a failure is `FetchFail`'s number with its
  status beside it, a `Message` and a `CameraTerrain` are numbers, a
  placeable cube is a sum type over the catalog's own numbering, and the
  profiler's report, trace and live stream are documents of numbers the shell
  writes the words for. A color is a number too — the catalog's swatches and
  the themes' lighting are packed by the generator, and nothing in the kernel
  parses `#rrggbb` any more.

  Not to be done, checked 09-17: the panic messages. They looked like the
  same kind of thing and are not. 200 runtime sites carry 6,287 bytes of
  prose, and 188 of them assert an internal invariant — "the cell is
  indexed", "the direct writer left open" — with runtime values interpolated
  into a good many ("pipeline {id}: location {location} fed twice"). An errno
  cannot carry either. The split exists for a consumer that routes on the
  code, as the shell routes on `FetchFail`; nothing routes on a panic, because
  the module traps and the client is over. Twelve sites name a resource
  condition, and of those only four can be reached on a device
  (`render/gpu/renderer.rs` twice, `render/gpu/api.rs`, the outbox's full
  spill) — what those want is to refuse rather than trap, the way the decor
  scratch and the mount pass were made to, not a number nobody reads.

- **Compression, chosen by traffic.** Recorded 2026-09-13, not scheduled:
  [20260913-compression-by-traffic.md](./20260913-compression-by-traffic.md).
  Measured again 2026-09-22 at 512 cells: zstd -22 is within 3% of Brotli
  q11 and decodes twice as fast, 46% under the bake's gzip -6. Safari takes
  zstd from 26.3, not on macOS before Tahoe; whether the host or the kernel
  decodes a stored page is open, since a React Native host would not decode
  what a browser does. Decided later that day: zstd -19 from the next bake,
  decoded by the transport, for what the bucket holds; what is published now
  stays gzip. Live
  responses want a fast level, realtime messages want an encoding and a
  stream more than a codec, and the codec stays in the transport, never in
  the command frame.

- **Decor: the impostor tier.** Decided 2026-09-13
  ([20260911-decor-identity-orientation-and-one-cell-format.md](./20260911-decor-identity-orientation-and-one-cell-format.md),
  "Closed"): each variant rendered once to a sprite, far instances drawn
  as a camera-facing quad through the billboard field. The criterion is
  in and every shipped kind is on the nearest tier only; the impostor is
  what would draw them further for two triangles each. Not a phone fix —
  build it when a wider decor range is wanted.
- **Terrain page blocks with the page.** `terrain/resident.rs` keeps the
  pool blocks a page owns in a table of its own; they belong in the page
  table's handle beside the views.

## P3

- **A staging file named by what it stages.** `pxl-geo-bake util.rs`
  `write_atomic` stages every write through `<name>.part`, so it is atomic
  only while one writer at a time writes a name. The dev tile server's
  cache and the offline bake keep that today — the shared ocean page is
  written once when a generation opens, since two folds re-publishing it
  raced on the rename and left pages a 500 for the generation (2026-09-29)
  — but the next writer outside the cache's `once` would race again. A
  staging name unique to its writer takes the assumption away.
- **A terrain read that hung.** Seen once on the simulator, 2026-09-27:
  every terrain page's read went unanswered for a minute and more while
  assets, geocode pages and the site's root answered at once, and a minute
  later the same pages came back in under two seconds. Not reproduced. The
  JavaScript was being reloaded under the kernel at the time, and a reload
  then left every credit with reads no shell would answer — the kernel now
  lets them go when a shell attaches (door.rs `host_attached`); whether that
  was all of it is not known.

- **The address label by the zoom.** Asked 2026-09-23, not now: it wants a
  rebake. The label is one baked answer a leaf page
  (`use-reverse-geocode.ts`, `geo-bake/geocode/provider/overture.rs`) — region,
  county, the fine polygon and a nearby point — shown whole at every zoom, so
  zoomed out to a province it still names a 동. The prior art is Nominatim's
  reverse `zoom`, which the client used before the bake: 5 a state, 8 a
  county, 10–13 a city or suburb, 14 on a neighborhood. Here, with level `k`
  at zoom 15 − k, something like the whole chain to level 2, region and county
  to level 5, region alone above.
  - Cut the leaf page's chain rather than bake one per level. A level page's
    center is not the map's: at level 4 a page is some 16 km, and its label
    would name the next county over. The leaf page is about a kilometer, so
    its chain is right and only needs shortening.
  - Each part carries its `subtype`, which means rebaking the geocode set
    and a new published version. Cutting by position is wrong where a level
    is missing: Sejong has no county, so "the first two" is not "region and
    county".
  - The kernel sends the window's base level in the snapshot beside
    `map.page` (`terrain/level.rs`'s `window()`); the client cuts the chain
    it holds, so a zoom does not fetch again.
  - Zoomed out, a pan crosses leaf pages faster and asks more often; the
    500 ms debounce holds that to two a second, nothing against the Worker's
    100k a day.

- **What the sky left open.** Recorded 2026-09-22
  ([the sky is the air](./20260922-the-sky-is-the-air.md)), each small and
  none blocking:
  - The terrain stands at its elevation and the planet's ground past it at sea
    level, so from high up a far piece of terrain floats over it with its
    chunk walls showing. Setting the ground at the map center's height, or
    skirting the coarse levels down to it, would close it.
  - Clouds with a thickness: a slab a kilometer or two deep, sampled a few
    times a pixel, for the volume a side view misses — as a quality setting,
    since it is several times today's cost. Volumetric clouds proper are out
    while the client draws at rest every two seconds.
  - The Milky Way, which a naked-eye catalog does not make (a low-resolution
    all-sky texture, whose licenses are attribution ones), and the bright
    planets, which the sun's and the moon's low-precision formulas extend to.
  - The themes' remaining fill — ambient, hemisphere, the floor — from the
    sky's irradiance, the way the sunlight now comes from the air. The floor
    is there for a phone's night and would resist it.
  - Not yet seen on a screen: the clouds at sunset, and the moonlit sky
    toward the moon itself.

- **A changelog, when `xpute` is published and not before.** Asked 2026-09-19
  and answered no: a changelog's reader is someone deciding whether to move
  between versions, and its load-bearing lines are the ones that change what
  that reader must do — removed, changed, deprecated, upgrade now. Continuous
  deployment has no such reader, since nobody chooses a build. What was wanted
  instead — which commits a build number carries — is a build log, and
  `check-and-deploy.yml`'s `record` job appends one line per published build to
  `_meta/build-log.jsonl`, subjects and all, which `./script/build-log.ts`
  reads back. Subjects rather than tags or ids because main's history is
  rewritten here routinely, and a name for a commit outlives the commit only if
  it is not a reference to one. The trigger that
  would make a real changelog earn its place is
  [the spin-out](./20260708-vision-and-scope-pivot.md): the day `@xpute/core`
  and `xpute-core` ship as packages, their consumers have neither this git log
  nor `issue/`, and the changelog belongs inside the package with its semver —
  generated from the commits at that point, not accumulated by hand until
  then. An installable build through Capacitor
  ([20260810-mobile-and-perf-targets.md](./20260810-mobile-and-perf-targets.md))
  is the same trigger from the other side, since a store submission wants
  release notes per version. Until one of those, `issue/` is the history and it
  records why, which a changelog never does.

- **The TypeScript's private marker, ported as a leading underscore.** 74
  places outside the tests: 53 functions and 21 fields, concentrated in
  `wire/xtp/cursor.rs` (25), `wire/tlv.rs` (23), `collection/arena.rs`
  (8) and `wire/xtp/encode.rs` (6), with a few each in
  `ipc/change.rs` and `wire/xtp/view.rs`,
  `pxl-spec`'s `sparse_tree/mem.rs` and its three `cell/` files. Rust has one
  meaning for a leading underscore and it is not privacy — privacy is the
  absence of `pub` — it silences the unused warning, which `rustc` confirms
  for items as well as bindings: `fn _never_called()` draws nothing where
  `fn never_called()` draws `never used`. So the marker turns the dead-code
  lint off for exactly the functions most likely to go dead. Two kinds:
  - Forced, where a private helper shares a name with the public method that
    calls it — `tlv.rs`'s `_u8`/`_i64`/`_f64`, `cursor.rs`'s
    `_bool`/`_f32_array`, about twenty in all. These need some
    disambiguator; `heap.rs`'s `build_of` is the suffix precedent.
  - Free, where nothing collides: `_grow`, `_need`, `_put`, `_take`,
    `_check_unsealed`, `_prune`, `_walk`, and every field — `_cap`, `_len`,
    `_size`, `_head`, `_buf`, `_off`, `_sealed`, `_factory`, `_mark`.
    `pxl-spec`'s `cell/type.rs` carries `pub _meta`, which says both at once
    and wants reading before it is renamed. `collection/heap.rs`'s `build_of` is the
    precedent for the forced kind, and the free kind just loses the underscore.

- **The TypeScript's numeric model, still named in the Rust.** `Number` and
  `BigInt` are the only numeric types TypeScript has, so the widths were
  typedef'd on top of them (`u64 = bigint`, `u8 = number`), and
  `TypedArray`/`BigTypedArray`/`ArrayBuffer`/`SharedArrayBuffer` do not union
  cleanly, so they were merged as far as they would go and the rest told
  apart by name. Rust needs none of that, and nothing here was ever meant to
  correspond to `Number` or `BigInt`: the port is not allowed to carry the
  other language's constraints across. `@xpute/core` and `xpute-runtime` are
  clear as of 2026-09-19; what the same sweep found and left alone:
  - `crates/pxl/spec/src/space/pvf3/` carries `BigInt` prose in twelve places
    (`core.rs`, `ivec.rs`, `fvec.rs`, `mod.rs`, `projection.rs`, `pvf3.rs`):
    "uses BigInt for bit-exact operations", "BigInt has no `>>>`",
    "`BigInt.asIntN(w, v)`". `pxl-spec` has no TypeScript twin at all, so
    these describe a file that does not exist.
  - `crates/pxl/web/kernel/src/shell/setting/`'s `SettingKind::Number` and
    `SettingValue::Number` (`schema.rs`, `store.rs`, `shell.rs`) — code
    rather than prose, so read first whether the schema's value kinds are
    JSON's or JavaScript's.
  - `ArrayBuffer` in `xpute/runtime/src/mem.rs`,
    `pxl/web/render/src/asset/atlas.rs` and
    `pxl/web/kernel/src/mem/mem.spec.rs`, where the module's `memory.buffer`
    really is one on the host's side. The boundary's name, kept unless the
    sentence is about the Rust.
  - `geo-bake/src/wkb.rs`'s "the TypeScript's `DataView` throws".
    `codec/json.rs`'s `JsonKind::Number` stays: that is JSON's own type name.
    `i128` is not part of this either — `xpute` has none, and `pvf3.rs`'s seven
    are the 64x64 intermediate of a fixed-point multiply rather than a width of
    the model.

- **Decor culling by page.** Decor is drawn for the whole loaded world
  in one instanced draw per variant; with the threshold it exists only in
  the nearest ring, so culling saves the half behind the camera. If the
  phone re-measurement still puts decor first, split into one draw per
  page and variant and cull by page bounds.
- **First load on a phone: the fetch queue, not the network.** Terrain
  and POI pages p50 9.7 s / 13 s from request to adopt, of which the
  network was 1.4–1.9 s; the rest was queue wait behind `MAX_INFLIGHT`
  6 with 64 pages asked at once, 4.2 MB per terrain page. That was
  measured before the 512-cell page, which is 1 MB, and before the
  levels, which ask a ring of pages a level, coarse first; what is worth asking
  is whether the quota and the page size are right on a phone's
  connection, measured again. On the device app over Wi-Fi, 2026-09-27: a
  terrain read that completed ran p50 1.6 s, and 28 of 48 were stopped
  before they did as the view moved.
- **The run set's per-axis inset has no writer.** `sub_sx`/`sub_sy` are
  always a copy of `sub_sz` (the scan, the coarse pack and the object
  adapter all set the three equal), so the per-axis design column_run.ts
  documents — a pillar narrow in XY and full in Z — costs two bytes a run
  for nothing today. Either the coarse bake learns to fit XY and Z
  separately, or the lanes go; decided with the Rust mesher, since the
  format is its input.
- **The scheduler's port, read against the TypeScript it replaced.**
  Read line for line on 2026-09-14 after the move (crates/pxl/kernel/src/world/sched.rs
  against 9ff57fe's chunk_node.ts, chunk_stage.tsx, chunk_table.ts,
  priority.ts). Fixed: the reclaim walked the order backward and gave back
  the nearest off-screen chunk first (core_sched.test.ts now pins farthest
  first); a layout the node re-attached on its own — decor changing — lost
  the blend the module does not repeat. Brought back to the TypeScript's
  behavior: builds move at most eight chunks a frame, counting only chunks
  that made progress; the pending lists are taken before the recheck; a
  rebuild on the spot trims the tier cache. Hardened: a layout naming a tier
  the node does not hold is warned and the chunk rebuilt from its sources,
  outside the ring read, instead of throwing inside the frame. What has no
  equivalence test is this layer: the pieces under it (mesher, scan, volume,
  terrain, object store, build) are held to the TypeScript byte for byte,
  the scheduler only by its order's unit tests and core_sched.test.ts's
  cases — worth a trace-driven comparison if its behavior is ever in doubt.

- **The const enum pass, once.** Asked 2026-09-27, not now. Vite
  (`apps/web/vite.config.ts` `const_enum`) and Metro
  (`apps/native/babel.config.js` `inline_enums`) each read every
  `export const enum` into a table of their own, a near copy, and already
  drift: one holds values as source text, the other as values. The reading
  goes to one file under `apps/web/script/`, plain JavaScript, since Metro
  `require`s the Babel config under Deno — whether CommonJS there can
  `require` an ES module is to be tried first. Vite's replacement is text:
  a member that prefixes another (`SENSOR_ENABLE` under a table read before
  it became `SENSOR_ENABLE_LOCATION`) left `7444_LOCATION` for the parser
  until the match was held to whole names. Replacing on the syntax tree, as
  Babel does, takes strings, comments and prefixes out of it as a class, for
  a parse per module on Vite's path.

## Parked, with the reason on file

- **`ds/treap_set` and `kit/comp`'s Rust side, removed from the pair.**
  Their consumers and what would bring them back are in
  [20260919-the-pair-audited-what-left.md](./20260919-the-pair-audited-what-left.md).
- **Coarse tiers: draw calls, a wider unit.** Not a bottleneck after the
  direct path; not without a measurement —
  [20260912-coarse-tiers-what-the-field-does.md](./20260912-coarse-tiers-what-the-field-does.md).
- **Characters and the rest without a record format.** Not before the format.
