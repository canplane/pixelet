# Serverless Live-Demo Deployment

**2026-08-16** — how the demo gets deployed without a server, what it costs, and
which parts of the current architecture had to change to make that true.

> Mechanical details below (table names, route paths, env vars like
> `VITE_TILE_BASE_URL`, `apps/api`, `/tiles/v1`, the `_ocean` fallback,
> gzip) reflect this doc's own date and have since moved —
> `spec/spec/route.json`, `ts/server/edge/wrangler.jsonc`,
> `ts/server/edge/index.ts` and `infra/README.md` are current.
> The reasoning (pre-baked tiles, object storage, one-Worker-one-origin)
> still holds for what the bucket serves; this note exists so the paths
> below aren't mistaken for current.
>
> The deployment is no longer serverless. The Worker relays `/geo/*` and
> `/api/*` to `GEO_ORIGIN` and `API_ORIGIN` (`wrangler.jsonc` `vars`), where
> Caddy sends them to pxl-geo and pxl-api, each with its database
> (`infra/prod/Caddyfile`, `infra/prod/docker-compose.yml`). The bucket
> still serves the tiles, under `/tile/*`, and the assets, under `/asset/*`.

---

## The problem this started from

The engine has been runnable only on a developer machine, because a running
terrain tile depended on a live PostGIS instance: `GET /api/world/terrain/tiles/:page_idx`
ran three spatial queries and a full bake **on every request**, with no cache.
Deploying that meant paying for a database, a Node process, and egress on
multi-megabyte tile responses — enough to stop the demo from ever going up.

The observation that unlocks the whole thing: **`page_idx → tile bytes` is a pure
function.** OSM and DEM source data don't change underneath a running server, so
the same page always bakes to the same bytes. Anything that pure can be computed
once, offline, and served as a static file. That collapses the deployment from
"web app + API + PostGIS" down to "web app + a bucket of files".

## Decisions

### 1. Tiles are pre-baked to an object store, not served live

> Superseded in part: tiles are still baked and served from the bucket, now
> by the Rust bakes (`bake_terrain`, `bake_place`, `bake_division` in
> `rust/server/geo-bake/src/bin/`), but search and the application are
> served live behind the same Worker — see the note at the top.

The whole coverage area is baked once by an offline tool
(`apps/api/src/bake/terrain/tools/bake_tiles.ts`) and uploaded as static
objects. No API server and no database exist in the deployed request path.

Tile bytes come from `apps/api/src/modules/world/terrain/source.ts`, which was
extracted out of the live endpoint's closure specifically so both paths share
one implementation. A CDN tile and a live-API tile are therefore identical byte
for byte; they can't drift, because there is only one assembly path to drift
from.

The live API keeps working unchanged for development, where baking on demand is
what you actually want.

### 2. Open-ocean pages are not baked at all — one shared fallback covers them

> Superseded: there is no `_ocean` object now. A page a version leaves out
> is answered `204` by the Worker (`left()` in
> `ts/server/edge/index.ts`), and the client reads a 204 as no
> object (`ts/client/src/namespace/read.ts`); 404 stays for a key that
> names nothing.

Classifying the coverage bbox against `osm_coastline_polygons` (the same
predicate `repository.ts`'s `OSM_COAST_SQL` already uses, so "has land" means
exactly what it means to the live bake path) splits it three ways:

| Page kind                           | Land polygons touching its bbox | Count         |
| ----------------------------------- | ------------------------------- | ------------- |
| Land or coastal — must be baked     | ≥ 1                             | **43,505**    |
| Pure open ocean — nothing but water | 0                               | **80,230**    |
|                                     |                                 | 123,735 total |

**65% of the bbox is open sea.** Baking and uploading 80,230 copies of "flat
water" would triple the dataset for no information.

Instead the bucket holds one `_ocean` object. The client requests a tile,
gets a 404 for any page that wasn't baked, and resolves that to the shared
tile. A 404 here isn't an error — it's the signal for "nothing but water here".

This is only sound if the ocean tile is genuinely page-independent, which is
not obvious: `bake_terrain` takes a `page_idx` and derives both a bbox and a
latitude-dependent `Z_SCALE` from it. So the tool **verifies it rather than
assuming it** — it bakes a synthesized empty source (zero elevation grid, no
features, no coastline rows) at three different pages and refuses to proceed
unless all three produce identical bytes. They do.

Synthesizing the empty source, rather than sampling some far-offshore page from
the DB, matters: deep-ocean pages turn out to carry the occasional stray feature
row (buoys, EEZ boundary points), so a sampled tile would have baked whatever
happened to be floating in the page that got picked.

**Known loss:** those stray mid-ocean features disappear from the demo. Judged
irrelevant at demo scope — they are invisible dots in open water — but it is a
real difference from what the live API would serve for the same page.

The client only takes the 404 → ocean path when a static tile origin is
configured. Against the live API every page, ocean included, bakes to a real 200,
so a 404 there is a genuine failure and must keep surfacing as one.

### 3. Tile origin is a build-time env var, versioned by URL prefix

> Superseded: `__TILE_BASE_URL__` is gone, and `VITE_TILE_BASE_URL` now
> only points the dev server's proxy at a local tile server
> (`ts/app/web/vite.config.ts`). Tile URLs are `/tile/<kind>/<version>/<name>`
> (`tileUrl` in `ts/client/src/config/route.ts`), and which version is
> served is the bucket's `source` declaration (`spec/spec/bake/published.json`).
> Immutable versions, never overwritten in place, still hold.

`VITE_TILE_BASE_URL` is read in `vite.config.ts` and injected as the
`__TILE_BASE_URL__` build constant (matching the existing `__APP_ENV__` /
`__IS_DEV__` pattern rather than introducing a second mechanism).

- **Empty** → the client requests `/api/world/terrain/tiles/:page_idx` and the
  dev proxy forwards it to the live API. This is local development, unchanged.
- **Set** → the client requests `${TILE_BASE}/${page_idx}` directly. Absolute
  (`https://tiles.example.com/v1`) and relative (`/tiles/v1`) both work, which
  matters for the same-origin hosting option below.

The version prefix is part of the value, and **tiles are never overwritten in
place**. A re-bake publishes `v2` and the app build points at `v2`. This is the
cheap answer to a problem that otherwise needs per-layer cache purging: an
object store, a CDN edge, the browser cache, and any future OPFS cache all
invalidate for free when the URL changes, with no purge API call and no TTL to
wait out. Old prefixes just stop being requested.

Per-tile content hashing was considered and rejected as more machinery than a
dataset re-baked a handful of times a year needs.

### 4. Objects are stored pre-gzipped

> Superseded: objects are stored zstd now (`CONTENT_ENCODING` in
> `rust/server/geo-bake/src/util.rs`; see
> [compression by traffic](./20260913-compression-by-traffic.md)). The
> upload takes the encoding from the bake's manifest (`script/publish.py`).

Object stores don't compress on the fly. Each tile is gzipped at bake time and
uploaded with `content-type: application/octet-stream` +
`content-encoding: gzip`; `fetch()` decompresses transparently, so the client
needs no change. Since versioned tiles are immutable, they can also carry
`cache-control: public, max-age=31536000, immutable`.

This matters more than expected — see the numbers below.

## Measured numbers

Full bake against the local geo DB, concurrency 8, completed:

|                     | Value                                       |
| ------------------- | ------------------------------------------- |
| Land tiles baked    | **43,505**, zero failures                   |
| Ocean pages skipped | 80,230 (one shared `_ocean` object instead) |
| Objects to upload   | 43,506 (tiles + `_ocean`)                   |
| Raw, uncompressed   | 170.24 GB                                   |
| **Gzipped total**   | **892.5 MB**                                |
| Gzipped per tile    | 21.0 kB average — **0.5% of raw**           |
| Wall clock          | 36m58s                                      |
| App bundle          | 1.97 MB (655 kB gzipped)                    |

The earlier estimate of "100–150 kB/tile → 4–6 GB" was measuring **uncompressed**
tiles. These payloads are dense per-cell ground channels built from polygon
fills — mostly long uniform runs — so they compress by more than two orders of
magnitude. The real upload is an order of magnitude smaller than planned for,
which puts the whole dataset comfortably inside a free storage tier instead of
brushing against it.

### Known defect: 297 tiles on the outer boundary have flat elevation

297 of 43,505 tiles (0.68%) logged `invalid ele shape` and fell back to a
zero-filled elevation grid, so they render perfectly flat.

They are exactly the **outermost ring of the candidate grid** — the maximum-Y row
(124), the minimum-Y row (111), and the maximum-X column (62). The elevation
query pads its sampling window by `ELE_PAD` around each page, and at the very
edge of `esa_elevation_raster`'s coverage there is no source raster to fill that
padding, so the resample returns a short grid (2176 or 1836 cells instead of
68×68 = 4624) and `fetch_terrain_source` substitutes zeros.

This is a property of where the DEM coverage ends, not a bug in the bake.

**Kept rather than dropped from the upload set**, on evidence rather than
convenience. The alternative was shrinking the candidate grid by one ring so
these pages 404 into the ocean fallback. Decoding the affected tiles and their
inward neighbors shows why that would be worse:

- The neighbor one page inside the ring carries real elevation (96–344 in the
  sampled tile). So the geometric discontinuity at the boundary exists either
  way — dropping the ring does not remove the cliff, since the ocean fallback is
  also flat at zero.
- Dropping it _adds_ a second error on top: land recolored as sea. Composition
  across the ring is mixed — sampled tiles ran from 99.4% sea to 100% bare land —
  so the ocean fallback would assert open water over genuine land in part of it.
- Where a ring tile is ~100% sea already, the two options are indistinguishable.

Keeping is therefore better-or-equal in every case sampled, and costs about 6 MB.

The ring is also not a quality cliff. Tiles just inside it are equally
featureless (see below); the only thing the ring additionally loses is elevation.

The correct fix, if this ever matters, is neither option: clamp the padded
sampling window to the available raster instead of letting a short grid fall back
to zeros. Worth doing only if the coverage area is extended — the same ring
reappears at whatever the new boundary is.

### The periphery has no OSM features at all

Not a defect, but it shapes what the demo actually contains. Comparing decoded
tiles:

|                  | interior (Seoul area)  | periphery (toward Japan / China / North Korea) |
| ---------------- | ---------------------- | ---------------------------------------------- |
| `land` channel   | 15–99% landuse classes | **only SEA and BARE, 0% everything else**      |
| structure pixels | 184 – 184,000          | **0**                                          |
| transport pixels | 20,000 – 218,000       | **0**                                          |

`osm_coastline_polygons` is global, so any page touching a landmass classifies as
land and gets baked — but the OSM _feature_ extract is Geofabrik's South Korea
file. Those pages therefore bake into a bare land/sea silhouette and nothing
else.

The elevation data behaves the same way, and less visibly. The DEM is four
GeoTIFFs covering mainland South Korea, Jeju, the far western islands, and
Ulleungdo/Dokdo — they do not tile the full 124–132°E / 33–39°N rectangle. The
ingestion pipeline builds a VRT over their union bounding box, so the mosaic
_reports_ full-rectangle coverage while the gaps hold zeros that are
indistinguishable from sea level downstream. Verified by probing the raster
directly: Jeju returns 934 m, a point in one of the implied gaps returns 0.

So the periphery is flat as well as featureless, and nothing in the data marks
the difference between "sea level" and "no data here". Correct for a South Korea
demo — and the reason the four boxes were chosen — but worth knowing before
reading anything into terrain outside them.

Size distribution over the 43,505 tiles confirms the scale:

|                                      |                                                                      |
| ------------------------------------ | -------------------------------------------------------------------- |
| min / p10                            | 4,190 B — identical in size to the `_ocean` tile, i.e. fully uniform |
| median                               | 19,331 B                                                             |
| max                                  | 172,927 B                                                            |
| under 6 kB (effectively featureless) | **6,949 tiles (16.0%)**                                              |
| total bytes in those                 | **29 MB**                                                            |

**16% of the tile count, 3% of the bytes** — featureless tiles compress to almost
nothing, which is exactly why pruning them isn't worth doing. Panning east, south
or north out of Korea yields plain bare terrain rather than an error or a hard
world edge, which is the better failure mode anyway.

## Hosting comparison

**Precondition that decides most of it:** the app bundle is 655 kB gzipped, so
its bandwidth is negligible anywhere. Effectively all traffic is tiles, and
tiles go to object storage regardless of host. So the free-bandwidth numbers the
three providers advertise barely enter into it.

|                             | Cloudflare Pages | Vercel (Hobby)        | Netlify (Free)        |
| --------------------------- | ---------------- | --------------------- | --------------------- |
| Static bandwidth            | Unlimited        | 100 GB/mo             | 100 GB/mo             |
| Builds                      | 500/mo           | ample                 | 300 min/mo            |
| Files per deployment        | 20,000 cap       | effectively unbounded | effectively unbounded |
| Commercial use on free tier | allowed          | **prohibited**        | allowed               |
| Same account as R2          | **yes**          | no                    | no                    |
| CORS needed for tiles       | **avoidable**    | required              | required              |
| DX                          | fine             | best                  | good                  |

Three things actually differentiate them:

**CORS avoidance.** With the app on Vercel or Netlify, app origin ≠ tile origin,
so the bucket needs a CORS policy naming the app domain — extra state to keep in
sync per domain change, and its absence fails _only_ in production, never
locally. On Cloudflare the bucket can be bound to a path on the app's own
hostname (`pixelet.app/tiles/*`), making tile requests same-origin, so CORS never
enters the picture. `VITE_TILE_BASE_URL=/tiles/v1` then suffices.

**File-count ceiling.** Cloudflare Pages caps a deployment at 20,000 files, so
43,505 tiles can't live there even in principle. Vercel and Netlify permit it but
would make builds absurd and would meter tile traffic against the 100 GB
allowance — a session panning around pulls tens of megabytes, so that allowance
is a number that genuinely depletes. Tiles belong in object storage regardless
of which host is picked.

**Vercel's Hobby terms** prohibit commercial use. Irrelevant to a portfolio demo;
relevant if the demo ever becomes anything, at which point it's $20/mo.

**Decision: Cloudflare Pages for the app + R2 for the tiles.** Vercel has the
better developer experience by a clear margin, but that doesn't outweigh the
combination of ongoing CORS management, a second vendor, and the non-commercial
clause. Netlify shows no advantage over Cloudflare for this workload.

### Cost

R2's free tier (10 GB storage, 1M class-A writes/mo, 10M class-B reads/mo) and
zero egress fees cover this entirely: 892.5 MB of storage, 43,506 uploads, and
read volume that a demo won't come near. Cloudflare Pages adds no bandwidth cost.
**Expected running cost: $0.**

Even the pathological case stays trivial — going over 10 GB of storage costs
about $0.015/GB/month. Egress, the thing that stopped this deployment from
happening in the first place, is free on R2 no matter the volume.

### There is no spending cap, and that is fine here

R2 has **no native spend limit**. Cloudflare shipped spend limits in June 2026,
but for Workers AI only; for R2 it remains an open feature request. Nothing
stops service at a threshold, so exposure has to be reasoned about rather than
bounded by a switch.

Three of the four meters are effectively fixed and cannot drift:

| Meter               | This deployment                                    | Free tier               |
| ------------------- | -------------------------------------------------- | ----------------------- |
| Storage             | 892.5 MB, and the dataset does not grow on its own | 10 GB                   |
| Class A (writes)    | 43,506, one-time at publish                        | 1M/month                |
| Egress              | —                                                  | always free             |
| **Class B (reads)** | **scales with traffic**                            | 10M/month, then $0.36/M |

Only reads vary. One tile request is one class-B operation, and a session that
pans around pulls roughly 50–200 tiles, so the free tier covers on the order of
60,000 sessions a month. Past that the cost is small enough that reaching it
would be a different kind of problem.

**The effective control is caching, not billing configuration.** The Worker
reads R2 on every request; putting responses through the Cache API means edge
hits never touch R2 at all, which is sound precisely because tiles are
immutable and versioned by URL. That turns the one variable meter into a
fraction of itself.

That change touches the same content-encoding path as the double-compression
trap above, and it did in fact regress on the first attempt — see "the same
trap again" above for what broke and why `no-transform` alone was not the fix.
Verified after: six tiles across the dataset serve at exactly their stored size
on both MISS and HIT, and decode byte-identically to a local `gunzip`.

A dashboard usage notification is worth setting up as a backstop, but it
reports rather than prevents.

### What actually bounds the spend (verified 2026-08-17)

The paragraph above guessed that leaving no payment method on file was the
closest thing to a hard ceiling. That is wrong twice over: R2 cannot be enabled
without one, and there is no auto-stop once the free tier is passed — usage is
billed pay-as-you-go against the card. The real bound is somewhere else, and it
is stronger than a billing setting.

Three facts, each checked rather than assumed:

- **The bucket is private.** `wrangler r2 bucket dev-url get pixelet-tiles`
  reports public access via the r2.dev URL disabled, and no custom domain is
  attached to it. Every path to the bucket goes through the Worker.
- **Workers is on the free plan**, whose limit is 100k requests/day and whose
  behavior past it is to fail with a 1027 error, not to bill. `run_worker_first`
  routes `/tiles/*` through the Worker, so a tile read cannot happen without a
  Worker invocation.
- **The edge cache is real here.** A repeat request for `/tiles/v1/_ocean`
  answers `cf-cache-status: HIT` with an `age` in the hours. This is worth
  stating because the opposite is widely believed — that the Cache API is inert
  on `*.workers.dev` — and it is not what this deployment does.

Together those compose into a ceiling nobody configured: at most 100k Worker
invocations a day is at most ~3M class-B reads a month even at a 0% cache hit
rate, against a 10M free tier. Storage (941 MB) and class-A (one bake's worth of
uploads) are nowhere near their own limits. **R2 cannot bill while those three
hold.** Exactly three things break it, and all three are deliberate acts:
moving Workers to a paid plan, enabling the r2.dev URL, or attaching a custom
domain to the bucket.

The Worker also validates the object key shape before touching R2:

```
^[A-Za-z0-9][A-Za-z0-9._-]{0,31}/(?:_ocean|-?\d{1,20})$
```

A miss is a billed class-B operation, so without this anyone can walk
`/tiles/<anything>` and charge one read per request while every one of them
404s. It returns the same 404 a real miss does — a distinct status would tell a
prober which guesses had the right shape, and the client cannot act on the
difference anyway.

> Since then: the shape is `KEY_RE` in `ts/server/edge/index.ts`,
> and a well-shaped page key with no object answers 204 (`left()`), not 404.

### On its own domain — and what that took (2026-08-17)

Closed, by moving the app off `*.workers.dev` and onto `plei.me`. The
`wrangler.jsonc` half is two keys:

```jsonc
"routes": [{ "pattern": "plei.me", "custom_domain": true }],
"workers_dev": false,
```

`custom_domain: true` means wrangler owns the DNS record and the certificate
— neither is created by hand, and an existing record at the same name makes
the deploy fail rather than being silently replaced. `workers_dev: false` is
not optional bookkeeping: WAF rules attach to a zone, so leaving the old
hostname up means every rule written against `plei.me` is bypassable by
asking for `pixelet.canplane.workers.dev` instead. A firewall with the side
door propped open. It disabled Preview URLs as a side effect, which wrangler
warns about; nothing needed them.

`plei.me` rather than one of the `xpute.*` domains the account also holds.
`@xpute/kit` is a runtime kit that carries no pixelet knowledge and is meant
to stay portable, so spending that name on one of the things
built with it would be the same conflation the package boundary exists to
prevent.

**The rest of it lives only in the Cloudflare dashboard**, which is worth
writing down precisely because it is not in version control and cannot be
diffed, reviewed, or restored from this repository.

| Where                          | What                             | Why                                                                         |
| ------------------------------ | -------------------------------- | --------------------------------------------------------------------------- |
| DNS                            | `plei.me` — created by wrangler  | Do not hand-edit; deploys recreate it                                       |
| DNS                            | `www` A `192.0.2.1`, **proxied** | Placeholder so the redirect below can fire                                  |
| Rules → Redirect Rules         | `www -> apex (301)`              | Dynamic, `concat("https://plei.me", http.request.uri.path)`, preserve query |
| Security → WAF → Rate limiting | `Throttle /tiles/* per IP`       | 100 requests / 10 seconds, block                                            |

> Since then the tile root is `/tile` (`spec/spec/route.json`), so a rule
> on `/tiles/*` matches nothing; the dashboard rule has to name `/tile/*`.

Two of those have a reason that is not obvious from the setting itself.

**The dummy A record.** Cloudflare only receives traffic for hostnames it has
a DNS record for, so a Redirect Rule cannot fire for a name that does not
resolve — `www` has to exist and be proxied before any rule applies to it.
`192.0.2.1` is RFC 5737's documentation range: not routable, so nothing can
accidentally reach it, and the request terminates at the edge in the rule
before the address is ever used. Both records carry a comment saying as much,
since a stray A record pointing at a documentation IP is exactly the kind of
thing a future reader deletes as junk. (The 100-character comment limit is
why they are terse.)

This whole pattern is a consequence of having no origin. With a real server
the same redirect was three lines of nginx and the A record held that
server's own address; the placeholder exists only because the edge replaced
the machine.

**Ten seconds, not one minute.** The Free plan allows a single rate-limiting
rule and only a 10-second counting period; the longer windows are paid. So
the limit is expressed as 100 requests per 10 seconds rather than the 300 per
minute it would otherwise be — and deliberately loose. A moving client pulls
9 tiles per position and up to 6 more per page boundary crossed, and Korean
mobile carriers put many users behind one CGNAT address, so a tight limit
costs real sessions. At 100/10s a full sweep of all 43,506 objects takes
about 72 minutes, which is friction rather than prevention, and friction is
the right goal: **the actual ceiling is the Workers free plan's 100k/day**,
and this rule only stops one address from spending that budget alone.

### The other shape of "custom domain", which is not this one

Kept because the two are easy to confuse, and only one of them is safe here.
The move above put the domain on the **Worker**. Putting it on the **bucket**
instead is a different trade entirely:

|                                     | Custom domain on the **Worker** | Custom domain on the **bucket**  |
| ----------------------------------- | ------------------------------- | -------------------------------- |
| Tiles still pass through the Worker | yes                             | **no**                           |
| 100k/day fail-closed ceiling        | kept                            | **gone**                         |
| R2 billing exposure                 | none                            | real (cache absorbs most)        |
| WAF / rate limiting                 | available                       | available                        |
| CORS                                | none (same origin)              | reintroduced (separate hostname) |

The first adds WAF and changes nothing else, which is why it is what shipped.
The second is what you would do once 100k/day became the binding constraint
rather than the safety net — a good problem, and not this one. It also
reintroduces CORS, which the same-origin `/tiles/*` path was chosen to avoid
in the first place.

> Pricing figures are from model knowledge and should be re-checked against the
> providers' current pages before committing. Note also that Cloudflare has been
> steering new projects toward Workers-with-static-assets rather than Pages, so
> confirm which path the current documentation describes.

### Uploading

> Now `script/publish.py`: `aws s3 sync` against R2's S3 endpoint, with the
> `content-encoding` and `content-type` taken from the bake's manifest.

43,506 objects rules out `wrangler r2 object put`, which handles one object per
invocation. R2 is S3-compatible, so `rclone copy` or `aws s3 sync` is the
practical route, setting `content-encoding: gzip`,
`content-type: application/octet-stream`, and the immutable `cache-control` on
the whole prefix.

## Deliberately not addressed

> **Superseded by [20260908-overture-poi-adoption.md](./archive/20260908-overture-poi-adoption.md).**
> Reverse geocoding no longer calls Nominatim or any third party at request
> time — it's baked per L3 page from Overture's Divisions theme. The
> live-call risk and `VITE_GEOCODE_BASE_URL` escape hatch described below no
> longer apply; kept as the reasoning that was live at the time.

**Reverse geocoding still calls a third party from the browser — accepted, and
now disclosed.** `use_reverse_geocode.ts` hits `nominatim.openstreetmap.org`
directly. OSM's usage policy expects an identifying User-Agent, which a browser
cannot set — only a Referer goes out — so public demo traffic can be throttled
or blocked. If location labels stop appearing after launch, this is the first
place to look; nothing else degrades.

Self-hosting Nominatim was considered and rejected: it needs PostgreSQL/PostGIS,
an hours-long OSM import, several GB, and an always-on server — it would undo
the entire serverless deployment for a place-name label. The realistic fallback,
if the public instance does start refusing traffic, is a ~20-line Cloudflare
Worker that forwards the request with a proper User-Agent. That is a relay, not
a geocoder: no database, and it fits in the free tier of the account already
being used. `VITE_GEOCODE_BASE_URL` exists to make that swap a config change
rather than a code change — it is a door left open, not one to walk through
now.

> Placed objects are now the kernel's kept slot `kept::OBJECTS` below its
> `STORAGE` root (`rust/client/kernel/src/sys/io/host.spec.rs`); the page
> keeps each slot in `localStorage` under its key, `pxl:kept:objects` for
> this one (`kept_key` in `ts/client/src/namespace/storage.ts`), the
> device app in the platform's preferences.

**Placed objects stay in `localStorage`.** Nothing is shared between visitors;
each browser sees only its own edits. Correct for a demo, and it's what makes
"no server" possible at all. The 5–10 MB quota and synchronous main-thread I/O
are known and unaddressed.

**Client engine code is readable in the bundle.** Anything that runs in the
browser ships to the browser — scene code, coordinate math, LOD shaders, the
voxel decoder. Minification is not obfuscation. This is a constant of client-side
rendering, not something deployment changes: the same exposure exists whether the
demo goes up or not. What stays private is the part that matters more — the bake
pipeline itself (OSM policy, POI ranking, PostGIS SQL, quantization) lives in
`apps/api` and never leaves the server, and the baked binaries don't give it up.

> The bake pipeline no longer lives in `apps/api`, which is gone: it is
> `rust/server/geo-bake`, and still never deployed.

**API hardening remains parked.** Rate limiting, timeouts, and circuit breaking
were judged out of scope (see project notes). They're moot for the static
deployment, which has no API to harden. The in-process LRU cache and
single-flight added to the live tile endpoint stay useful for development and for
any future non-static path.

## Deploying it: what actually broke

> The Worker is `ts/server/edge/index.ts` now, and
> `run_worker_first` claims four roots, `["/geo/*", "/api/*", "/tile/*",
> "/asset/*"]` (`ts/server/edge/wrangler.jsonc`, written from
> `spec/spec/route.json`). The reasoning below holds.

The shape that shipped is one Cloudflare Worker serving both the built app and
the tiles (`ts/server/edge/wrangler.jsonc`, `apps/web/worker/index.ts`), so tiles live
under `/tiles/` on the app's own hostname and are same-origin by construction.
Two settings make that work together, and both matter:

```jsonc
"not_found_handling": "single-page-application",
"run_worker_first": ["/tiles/*"]
```

The first is needed so a deep link falls back to `index.html` instead of 404ing.
On its own it would also swallow every tile request and hand back `index.html`,
which the client would then try to decode as terrain — a failure with no
resemblance to its cause. The second routes `/tiles/*` to the Worker before the
asset fallback sees it.

### The trap: the runtime re-compresses an already-compressed body

Tiles are stored pre-gzipped, and the Worker replays R2's stored metadata onto
the response with `writeHttpMetadata()`, including `content-encoding: gzip`.
That is not enough. By default the Workers runtime assumes a `Response` body is
_not_ encoded and applies its own compression on top.

Measured against a stored 8,653-byte tile:

| Client sends                | Bytes received | `content-encoding` | Result                                                   |
| --------------------------- | -------------- | ------------------ | -------------------------------------------------------- |
| `Accept-Encoding: gzip`     | **4,343**      | `gzip`             | gzip inside gzip — one decode leaves it still compressed |
| `Accept-Encoding: identity` | 8,653          | **absent**         | raw gzip bytes, client never told to decode              |

Both paths end with `decode_terrain()` receiving compressed input, and neither
announces itself at the point of failure.

The fix is `encodeBody: "manual"` in the `ResponseInit`, which tells the runtime
the body is already in its final encoding. After it, the same request returns
exactly 8,653 bytes with `content-encoding: gzip`, and the decoded payload is
byte-identical to a local `gunzip` of the same object — 4,203,672 bytes opening
with the XTP magic.

**Uploading 50 tiles before the other 43,455 is what surfaced this.** The bytes
in R2 were correct the whole time; only the serving path was wrong, so no amount
of checking the upload would have found it — only checking what comes back over
the wire did.

### The same trap again, one layer out

> Since replaced: the cache stores the packed bytes with their encoding
> parked on `x-pxl-stored-encoding` (`STORED_ENCODING`), and `serve()` puts
> it back with `encodeBody: "manual"` on both paths out
> (`ts/server/edge/index.ts`).

Putting the Cache API in front of R2 — the cost control described below —
reintroduced the identical failure on cache hits alone. A tile stored at 34,261
bytes served at 34,261 on a MISS and **30,728 on a HIT**, and the decode broke.

`no-transform` on the cached response, which community reports suggest, did not
prevent it. The actual cause: **`encodeBody: "manual"` does not survive the
cache round trip.** A cached entry comes back as an ordinary `Response` with no
memory of that flag, so the runtime re-encodes the already-gzipped body on its
way out. Re-asserting it on the hit path fixes it:

```ts
const cached = await cache.match(request);
if (cached) {
  return new Response(cached.body, { headers: cached.headers, encodeBody: "manual" });
}
```

Worth noting how this failed: only on the second request, only through the
edge, and with a _smaller_ response — the sort of thing that looks like the
compression working rather than breaking. Comparing served size against stored
size is what catches it; nothing about the response is otherwise suspicious.

### Verified end to end

> As of this date. A page never baked now answers 204, and there is no
> `_ocean` (decision 2's note).

| Check                | Result                                                              |
| -------------------- | ------------------------------------------------------------------- |
| App loads            | 200, `text/html`                                                    |
| Uploaded tile        | 200, `content-encoding: gzip`, decodes byte-identical to local      |
| **Page never baked** | **404 `text/plain`** — not `index.html`, so the routing split holds |
| `_ocean` fallback    | 200, decodes to 100% SEA at elevation 0                             |
| Seoul tile content   | 218,976 building pixels, 283,752 road pixels, elevation 12–110 m    |

## Pre-deployment secret audit

Run against the built bundle rather than reasoned about, since the whole point
is what actually ships.

| Check                                                                                                                | Result                                                                           |
| -------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| All 21 root `.env` variable **names** in the bundle (`ADMIN_TOKEN`, `DB_*_PASSWORD`, `DB_*_URL`, `postgresql://`, …) | **0 hits**                                                                       |
| All root `.env` **values** matched literally against the bundle                                                      | only `DB_MAIN_HOST`/`DB_GEO_HOST`, whose value is `localhost` — a false positive |
| Source maps in `dist/`                                                                                               | **0** (`sourcemap: mode !== "production"`)                                       |
| Stray files in `dist/`                                                                                               | none — bundle, css, fonts, favicons, locales                                     |

Structurally this holds because Vite's `loadEnv(mode, cwd, "VITE_")` only reads
`VITE_`-prefixed variables, so the database credentials are never loaded into
the build at all. And since the API is not deployed, `ADMIN_TOKEN` and
`/api/internal/*` have no route to the internet in the first place.

Two secret-looking literals in the bundle were checked and are benign: React's
own list of input types (which contains `password`), and the deliberate
attribution link in the license section.

> Since superseded: Nominatim is not called; reverse geocoding is baked
> (the divisions bake), and the coordinate that leaves the client now is
> forward search's `near`, sent to the project's own `/geo/search`
> (`fetchSearch` in `ts/client/src/shell/search/repository.ts`). The
> settings this paragraph lists are now `pxl:lang`, `pxl:ui_hotbar` and the
> kernel's two kept slots, the settings and the placed objects
> (`pxl:kept:settings`, `pxl:kept:objects`;
> `ts/client/src/namespace/storage.ts`). pxl-api is deployed and its
> `/api/internal` is behind the admin token (`API_INTERNAL`,
> `rust/server/api/src/main.rs`).

**Location is the sensitive surface here.** The only personal data
leaving the client is the map-center coordinate sent to Nominatim — rounded to
four decimal places (~11 m) and debounced, never stored. That is now disclosed
in the settings license section, translated rather than hardcoded, because it is
a statement to the user rather than a notice a license demands verbatim.

Everything else stays on the device: `pxl:lang`, `pxl:render_settings`,
`pxl:debug_settings`, and `pxl:objects:v4` in `localStorage`.

If the API is ever deployed, `ADMIN_TOKEN` must be regenerated rather than
carried over from development. The bucket stays private and is read only
through the Worker ("What actually bounds the spend", above).

## Changes this produced

> History: `apps/api` and every file below it are gone, and the client is
> `ts/client`; none of these paths is current.

| File                                                     | Change                                                                    |
| -------------------------------------------------------- | ------------------------------------------------------------------------- |
| `apps/api/src/modules/world/terrain/source.ts`           | new — tile assembly extracted so offline bake and live API share one path |
| `apps/api/src/modules/world/terrain/service.ts`          | uses the extracted source instead of its own copy                         |
| `apps/api/src/bake/terrain/tools/bake_tiles.ts`          | new — classify, bake, gzip, resumable, writes a manifest                  |
| `ts/app/web/vite.config.ts`                              | reads `VITE_TILE_BASE_URL` → `__TILE_BASE_URL__`                          |
| `ts/app/web/@types/index.d.ts`                           | declares the new build constant                                           |
| `ts/app/web/.env.example`                                | documents the tile origin and its versioning rule                         |
| `apps/web/src/modules/world/terrain/use_tile_fetcher.ts` | tile-origin URL builder + 404 → shared ocean tile                         |
