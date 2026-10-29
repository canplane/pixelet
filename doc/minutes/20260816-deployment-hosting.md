# Live-Demo Deployment

**2026-08-16** — how the demo gets deployed, what it costs, and which parts of
the architecture had to change to make that true; kept up to what the code
holds.

One Cloudflare Worker is the demo's one origin. The bucket serves the baked
tiles, under `/tile/*`, and the assets, under `/asset/*`; the Worker relays
`/geo/*` and `/api/*` to `GEO_ORIGIN` and `API_ORIGIN`
(`ts/server/edge/wrangler.jsonc` `vars`), where Caddy sends them to pxl-geo and
pxl-api, each with its database (`infra/prod/Caddyfile`,
`infra/prod/docker-compose.yml`). The roots are `spec/spec/route.json`'s. The
reasoning below (pre-baked tiles, object storage, one Worker and one origin) is
for what the bucket serves.

---

## The problem this started from

The engine has been runnable only on a developer machine, because a running
terrain tile depended on a live PostGIS instance:
`GET /api/world/terrain/tiles/:page_idx` ran three spatial queries and a full
bake **on every request**, with no cache. Deploying that meant paying for a
database, a Node process, and egress on multi-megabyte tile responses — enough
to stop the demo from ever going up.

The observation that unlocks the whole thing: **`page_idx → tile bytes` is a
pure function.** OSM and DEM source data don't change underneath a running
server, so the same page always bakes to the same bytes. Anything that pure can
be computed once, offline, and served as a static file. That takes the tiles out
of "web app + API + PostGIS" and into a bucket of files; only search and the
application are left live.

## Decisions

### 1. Tiles are pre-baked to an object store, not served live

Every tile is baked once by an offline tool, the Rust bakes (`bake_terrain`,
`bake_place`, `bake_division` in `rust/server/geo-bake/src/bin/`), and uploaded
as static objects. No server and no database is in a tile's request path; search
and the application are, behind the same Worker.

The offline bake and the dev tile server
(`rust/server/geo-bake/src/bin/serve_tiles.rs`) ask the same question and bake
by the same functions, so both produce the same objects
(`rust/server/geo-bake/src/terrain/leaf.rs`). A bucket tile and a dev
tile can't drift, because there is only one assembly path to drift from.

The dev tile server bakes on demand for development, where baking on demand is
what you actually want.

### 2. Open-ocean pages are not baked at all — an empty answer covers them

A terrain leaf exists where the elevation source covers it and something is
drawn in it: land from OSM's land polygons, a building, or a way
(`rust/server/geo-bake/src/terrain/leaf.rs` `HAS_GROUND_SQL`). The offline bake
and the dev server ask the same question, so "has land" means exactly what it
means to both.

Baking and uploading a copy of "flat water" for every open-sea page would add to
the dataset for no information.

Instead a page a version leaves out has no object, and the Worker answers it
`204` (`left` in `ts/server/edge/index.ts`); the client reads a 204 as no object
(`get` in `ts/client/src/namespace/read.ts`). A 204 here isn't an error — it's
the signal for "nothing here". 404 stays for a key that names nothing, a genuine
failure that must keep surfacing as one. The dev tile server answers a tile it
leaves out with the same 204 (`serve_tiles.rs`).

### 3. Tile URLs are versioned by prefix, the version the bucket's to name

A tile's URL is `/tile/<kind>/<version>/<name>` on the app's own origin
(`tile_url` in `ts/client/src/namespace/route.ts`), and which version of each
kind is served is the bucket's `source` declaration, which the client reads at
boot (`spec/spec/bake/published.json`). `VITE_TILE_BASE_URL` only points the dev
server's proxy at a local tile server (`ts/app/web/vite.config.ts`).

The version is part of the URL, and **tiles are never overwritten in place**. A
re-bake uploads a new version and the declaration names it (`upload` and
`serve` in `script/publish.py`). This is the cheap answer to a problem that
otherwise needs per-layer cache purging: an object store, a CDN edge, the
browser cache, and any future OPFS cache all invalidate for free when the URL
changes, with no purge API call and no TTL to wait out. Old prefixes just stop
being requested.

Per-tile content hashing was considered and rejected as more machinery than a
dataset re-baked a handful of times a year needs.

### 4. Objects are stored pre-compressed

Object stores don't compress on the fly. Each tile is compressed with zstd at
bake time (`CONTENT_ENCODING` in `rust/server/geo-bake/src/util.rs`; see
[compression by traffic](./20260913-compression-by-traffic.md)) and uploaded
with the `content-type` and `content-encoding` the bake's manifest names,
`application/octet-stream` and `zstd` (`write_manifest` in the same file;
`script/publish.py`); `fetch()` decompresses it transparently. Since versioned
tiles are immutable, they also carry
`cache-control: public, max-age=31536000, immutable`.

## Hosting comparison

**Precondition that decides most of it:** the app bundle's bandwidth is
negligible anywhere. Effectively all traffic is tiles, and tiles go to object
storage regardless of host. So the free-bandwidth numbers the three providers
advertise barely enter into it.

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
hostname (`/tile/*`), making tile requests same-origin, so CORS never enters the
picture. The client then asks for tiles on its own origin (`tile_url`).

**File-count ceiling.** Cloudflare Pages caps a deployment at 20,000 files, so
the tiles can't live there even in principle. Vercel and Netlify permit it but
would make builds absurd and would meter tile traffic against the 100 GB
allowance — a session panning around pulls tens of megabytes, so that allowance
is a number that genuinely depletes. Tiles belong in object storage regardless
of which host is picked.

**Vercel's Hobby terms** prohibit commercial use. Irrelevant to a portfolio
demo; relevant if the demo ever becomes anything, at which point it's $20/mo.

**Decision: Cloudflare for the app + R2 for the tiles.** The app is the Worker's
static assets, not a Pages project (`assets` in
`ts/server/edge/wrangler.jsonc`). Vercel has the better developer experience by
a clear margin, but that doesn't outweigh the combination of ongoing CORS
management, a second vendor, and the non-commercial clause. Netlify shows no
advantage over Cloudflare for this workload.

### Cost

R2's free tier (10 GB storage, 1M class-A writes/mo, 10M class-B reads/mo) and
zero egress fees cover this entirely: a dataset that does not grow on its own,
one bake's worth of uploads at each publish, and read volume that a demo won't
come near. **Expected running cost: $0.**

Even the pathological case stays trivial — going over 10 GB of storage costs
about $0.015/GB/month. Egress, the thing that stopped this deployment from
happening in the first place, is free on R2 no matter the volume.

### There is no spending cap, and that is fine here

R2 has **no native spend limit**. Cloudflare shipped spend limits in June 2026,
but for Workers AI only; for R2 it remains an open feature request. Nothing
stops service at a threshold, so exposure has to be reasoned about rather than
bounded by a switch.

Three of the four meters are effectively fixed and cannot drift:

| Meter               | This deployment                             | Free tier               |
| ------------------- | ------------------------------------------- | ----------------------- |
| Storage             | the dataset, which does not grow on its own | 10 GB                   |
| Class A (writes)    | one bake's worth, at each publish           | 1M/month                |
| Egress              | —                                           | always free             |
| **Class B (reads)** | **scales with traffic**                     | 10M/month, then $0.36/M |

Only reads vary. One tile request is one class-B operation. Past the free tier
the cost is small enough that reaching it would be a different kind of problem.

**The effective control is caching, not billing configuration.** The Worker
puts what it reads out of R2 through the Cache API (`caches.default` in
`ts/server/edge/index.ts`), so edge hits never touch R2 at all, which is sound
precisely because tiles are immutable and versioned by URL. That turns the one
variable meter into a fraction of itself.

That cache sits on the same content-encoding path as the double-compression trap
below, and it did in fact regress on the first attempt — see "the same trap
again" below for what broke and why `no-transform` alone was not the fix.

A dashboard usage notification is worth setting up as a backstop, but it reports
rather than prevents.

### What actually bounds the spend (verified 2026-08-17)

Leaving no payment method on file is not a hard ceiling, twice over: R2 cannot
be enabled without one, and there is no auto-stop once the free tier is passed —
usage is billed pay-as-you-go against the card. The real bound is somewhere
else, and it is stronger than a billing setting.

Three facts, each checked rather than assumed:

- **The bucket is private.** `wrangler r2 bucket dev-url get pixelet-tiles`
  reports public access via the r2.dev URL disabled, and no custom domain is
  attached to it. Every path to the bucket goes through the Worker.
- **Workers is on the free plan**, whose limit is 100k requests/day and whose
  behavior past it is to fail with a 1027 error, not to bill. `run_worker_first`
  routes `/tile/*` and `/asset/*` through the Worker
  (`ts/server/edge/wrangler.jsonc`), so a bucket read cannot happen without a
  Worker invocation.
- **The edge cache is real here.** A repeat request for a tile answers
  `cf-cache-status: HIT`. This is worth stating because the opposite is widely
  believed — that the Cache API is inert on `*.workers.dev` — and it is not what
  this deployment does.

Together those compose into a ceiling nobody configured: at most 100k Worker
invocations a day is at most ~3M class-B reads a month even at a 0% cache hit
rate, against a 10M free tier. Storage and class-A (one bake's worth of uploads
at each publish) are nowhere near their own limits. **R2 cannot bill while those
three hold.** Exactly three things break it, and all three are deliberate acts:
moving Workers to a paid plan, enabling the r2.dev URL, or attaching a custom
domain to the bucket.

The Worker also validates the object key's shape before touching R2 (`KEY_RE`
in `ts/server/edge/index.ts`): a page of a kind the client reads
(`BASE_LAYERS`) under the object `spec/spec/route.json` shapes (`TILE_OBJECT`),
the Earth's picture whole (`TILE_SPHERE`), an asset named by its sha256, or the
`source` declaration.

A miss is a billed class-B operation, so without this anyone can walk
`/tile/<anything>` or `/asset/<anything>` and charge one read per request. A key
of the wrong shape answers 404, as an asset with no object does; a page key of
the right shape with no object answers 204 (`left`), since that is a page its
version leaves out.

### On its own domain — and what that took (2026-08-17)

Closed, by moving the app off `*.workers.dev` and onto `plei.me`. The
`wrangler.jsonc` half is two keys:

```jsonc
"routes": [{ "pattern": "plei.me", "custom_domain": true }],
"workers_dev": false,
```

`custom_domain: true` means wrangler owns the DNS record and the certificate —
neither is created by hand, and an existing record at the same name makes the
deploy fail rather than being silently replaced. `workers_dev: false` is not
optional bookkeeping: WAF rules attach to a zone, so leaving the old hostname up
means every rule written against `plei.me` is bypassable by asking for
`pixelet.canplane.workers.dev` instead. A firewall with the side door propped
open. It disabled Preview URLs as a side effect, which wrangler warns about;
nothing needed them.

`plei.me` rather than one of the `xpute.*` domains the account also holds.
`@xpute/kit` is a runtime kit that carries no pixelet knowledge and is meant to
stay portable, so spending that name on one of the things built with it would be
the same conflation the package boundary exists to prevent.

**The rest of it lives only in the Cloudflare dashboard**, which is worth
writing down precisely because it is not in version control and cannot be
diffed, reviewed, or restored from this repository.

| Where                          | What                             | Why                                                                         |
| ------------------------------ | -------------------------------- | --------------------------------------------------------------------------- |
| DNS                            | `plei.me` — created by wrangler  | Do not hand-edit; deploys recreate it                                       |
| DNS                            | `www` A `192.0.2.1`, **proxied** | Placeholder so the redirect below can fire                                  |
| Rules → Redirect Rules         | `www -> apex (301)`              | Dynamic, `concat("https://plei.me", http.request.uri.path)`, preserve query |
| Security → WAF → Rate limiting | `Throttle /tile/* per IP`        | 100 requests / 10 seconds, block                                            |

The rate limit has to name the tile root, `/tile/*` (`TILE_ROOT` in
`spec/spec/route.json`); a rule on another path does not throttle the tiles.

Two of those have a reason that is not obvious from the setting itself.

**The dummy A record.** Cloudflare only receives traffic for hostnames it has a
DNS record for, so a Redirect Rule cannot fire for a name that does not resolve
— `www` has to exist and be proxied before any rule applies to it. `192.0.2.1`
is RFC 5737's documentation range: not routable, so nothing can accidentally
reach it, and the request terminates at the edge in the rule before the address
is ever used. Both records carry a comment saying as much, since a stray A
record pointing at a documentation IP is exactly the kind of thing a future
reader deletes as junk. (The 100-character comment limit is why they are terse.)

This whole pattern is a consequence of having no origin. With a real server the
same redirect was three lines of nginx and the A record held that server's own
address; the placeholder exists only because the edge replaced the machine.

**Ten seconds, not one minute.** The Free plan allows a single rate-limiting
rule and only a 10-second counting period; the longer windows are paid. So the
limit is expressed as 100 requests per 10 seconds rather than the 300 per minute
it would otherwise be — and deliberately loose. A moving client pulls many tiles
at a time, and Korean mobile carriers put many users behind one CGNAT address,
so a tight limit costs real sessions. At 100/10s a full sweep of the bucket is
friction rather than prevention, and friction is the right goal: **the actual
ceiling is the Workers free plan's 100k/day**, and this rule only stops one
address from spending that budget alone.

### The other shape of "custom domain", which is not this one

Kept because the two are easy to confuse, and only one of them is safe here. The
move above put the domain on the **Worker**. Putting it on the **bucket**
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
reintroduces CORS, which the same-origin `/tile/*` path was chosen to avoid in
the first place.

> Pricing figures are from model knowledge and should be re-checked against the
> providers' current pages before committing.

### Uploading

A bake's objects are far too many for `wrangler r2 object put`, which handles
one object per invocation. R2 is S3-compatible, so `script/publish.py` uploads a
version with `aws s3 sync` against R2's S3 endpoint (`upload`, `endpoint`),
setting the `content-encoding` and `content-type` the bake's manifest names and
the immutable `cache-control` on the whole prefix.

## Deliberately not addressed

**Reverse geocoding calls no third party.** It is baked, by the divisions bake
(`bake_division` in `rust/server/geo-bake/src/bin/`), from Overture's Divisions
theme ([20260908-overture-poi-adoption.md](./archive/20260908-overture-poi-adoption.md));
nothing is called at request time.

**Placed objects stay on the device.** They are the kernel's kept slot
`kept::OBJECTS` below its `STORAGE` root
(`rust/client/kernel/src/sys/io/host.spec.rs`); the page keeps the slot in
`localStorage` under its key, `pxl:kept:objects` (`kept_key` in
`ts/client/src/namespace/storage.ts`), the device app in the platform's
preferences. Nothing is shared between visitors; each browser sees only its own
edits. Correct for a demo. The 5–10 MB quota and synchronous main-thread I/O are
known and unaddressed.

**Client engine code is readable in the bundle.** Anything that runs in the
browser ships to the browser — scene code, coordinate math, LOD shaders, the
voxel decoder. Minification is not obfuscation. This is a constant of
client-side rendering, not something deployment changes: the same exposure
exists whether the demo goes up or not. What stays private is the part that
matters more — the bake pipeline itself (OSM policy, POI ranking, PostGIS SQL,
quantization) lives in `rust/server/geo-bake`, which is never deployed, and the
baked binaries don't give it up.

**API hardening remains parked.** Rate limiting, timeouts, and circuit breaking
were judged out of scope (see project notes).

## Deploying it: what actually broke

The shape that shipped is one Cloudflare Worker serving the built app, the tiles
and the assets (`ts/server/edge/wrangler.jsonc`, `ts/server/edge/index.ts`), so
tiles live under `/tile/` and assets under `/asset/` on the app's own hostname
and are same-origin by construction, as `/geo/*` and `/api/*` are through its
relay. Two settings make that work together, and both matter:

```jsonc
"not_found_handling": "single-page-application",
"run_worker_first": ["/geo/*", "/api/*", "/tile/*", "/asset/*"]
```

The first is needed so a deep link falls back to `index.html` instead of 404ing.
On its own it would also swallow every request meant for a server or the bucket
and hand back `index.html`, which the client would then try to decode as terrain
— a failure with no resemblance to its cause. The second routes the four roots,
decided in `spec/spec/route.json`, to the Worker before the asset fallback sees
them.

### The trap: the runtime re-compresses an already-compressed body

Tiles are stored pre-compressed, and the Worker replays R2's stored metadata
onto the response with `writeHttpMetadata()`, including the `content-encoding`.
That is not enough. By default the Workers runtime assumes a `Response` body is
_not_ encoded and applies its own compression on top.

Measured on gzip, against a stored 8,653-byte tile:

| Client sends                | Bytes received | `content-encoding` | Result                                                   |
| --------------------------- | -------------- | ------------------ | -------------------------------------------------------- |
| `Accept-Encoding: gzip`     | **4,343**      | `gzip`             | gzip inside gzip — one decode leaves it still compressed |
| `Accept-Encoding: identity` | 8,653          | **absent**         | raw gzip bytes, client never told to decode              |

Both paths end with `decode_terrain()` receiving compressed input, and neither
announces itself at the point of failure.

The fix is `encodeBody: "manual"` in the `ResponseInit`, which tells the runtime
the body is already in its final encoding (`serve` in
`ts/server/edge/index.ts`). After it, the same request returned exactly 8,653
bytes with `content-encoding: gzip`, and the decoded payload was byte-identical
to a local `gunzip` of the same object.

**Uploading a few tiles before the rest is what surfaced this.** The bytes in R2
were correct the whole time; only the serving path was wrong, so no amount of
checking the upload would have found it — only checking what comes back over
the wire did.

### The same trap again, one layer out

Putting the Cache API in front of R2 — the cost control described above —
reintroduced the identical failure on cache hits alone. A tile stored at 34,261
bytes served at 34,261 on a MISS and **30,728 on a HIT**, and the decode broke.

`no-transform` on the cached response, which community reports suggest, did not
prevent it. The actual cause: **`encodeBody: "manual"` does not survive the
cache round trip.** A cached entry comes back as an ordinary `Response` with no
memory of that flag, so the runtime re-encodes the already-compressed body on
its way out. So the cache stores the packed bytes with their encoding parked on
`x-pxl-stored-encoding` (`STORED_ENCODING`), and `serve()` puts it back with
`encodeBody: "manual"` on both paths out (`ts/server/edge/index.ts`).

Worth noting how this failed: only on the second request, only through the edge,
and with a _smaller_ response — the sort of thing that looks like the
compression working rather than breaking. Comparing served size against stored
size is what catches it; nothing about the response is otherwise suspicious.

### End to end

| Check                | Result                                                          |
| -------------------- | --------------------------------------------------------------- |
| App loads            | 200, `text/html`                                                |
| Uploaded tile        | 200, with the `content-encoding` it was stored in (`serve`)     |
| **Page never baked** | **204** (`left`) — not `index.html`, so the routing split holds |

## Pre-deployment secret audit

Run against the built bundle rather than reasoned about, since the whole point
is what actually ships.

| Check                                                                                                    | Result                                                                           |
| -------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `.env` variable **names** in the bundle (`ADMIN_TOKEN`, `DB_*_PASSWORD`, `DB_*_URL`, `postgresql://`, …) | **0 hits**                                                                       |
| `.env` **values** matched literally against the bundle                                                   | only `DB_MAIN_HOST`/`DB_GEO_HOST`, whose value is `localhost` — a false positive |
| Source maps in `dist/`                                                                                   | **0** (`sourcemap: mode !== "production"`, `ts/app/web/vite.config.ts`)          |
| Stray files in `dist/`                                                                                   | none — bundle, css, fonts, favicons, locales                                     |

Structurally this holds because Vite's `loadEnv(mode, cwd, "VITE_")` only reads
`VITE_`-prefixed variables, so the database credentials are never loaded into
the build at all. pxl-api is deployed, and its `/api/internal` is behind the
admin token (`API_INTERNAL` in `rust/server/api/src/main.rs`, gated by `shell`
in `rust/server/src/serve.rs`).

Two secret-looking literals in the bundle were checked and are benign: React's
own list of input types (which contains `password`), and the deliberate
attribution link in the license section.

**Location is the sensitive surface here.** The only personal data leaving the
client is the view's center, which forward search sends as `near` to the
project's own `/geo/search` (`fetchSearch` in
`ts/client/src/shell/search/fetch.ts`), debounced (`DEBOUNCE_MS` in
`ts/client/src/shell/search/modal.tsx`). That is disclosed in the settings'
disclaimer section, translated rather than hardcoded (`location_notice` in
`ts/client/src/shell/setting/info/disclaimer.tsx`), because it is a statement
to the user rather than a notice a license demands verbatim.

Everything else stays on the device: `pxl:lang`, `pxl:ui_hotbar` and the
kernel's two kept slots, the settings and the placed objects
(`pxl:kept:settings`, `pxl:kept:objects`; `ts/client/src/namespace/storage.ts`),
in `localStorage`.

`ADMIN_TOKEN` is production's own (`infra/prod/.env`), never carried over from
development's (`rust/server/.env`). The bucket stays private and is read only
through the Worker ("What actually bounds the spend", above).

## Changes this produced

Where the parts this produced are held:

| File                                                       | What it holds                                                                                        |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `rust/server/geo-bake/src/bin/`                            | the bakes (`bake_terrain`, `bake_place`, `bake_division`) and the dev tile server (`serve_tiles.rs`) |
| `rust/server/geo-bake/src/terrain/leaf.rs`                 | which terrain leaves exist, asked alike by the offline bake and the dev server                       |
| `rust/server/geo-bake/src/util.rs`                         | the stored encoding (`CONTENT_ENCODING`) and the manifest a set carries                              |
| `script/publish.py`                                        | the upload and the bucket's `source` declaration                                                     |
| `ts/server/edge/wrangler.jsonc`, `ts/server/edge/index.ts` | the Worker: the app, the bucket's roots, the relay, the key shape and the cache                      |
| `ts/client/src/namespace/route.ts`                         | tile URLs (`tile_url`)                                                                               |
| `ts/client/src/namespace/read.ts`                          | a 204 read as no object                                                                              |
| `ts/app/web/vite.config.ts`                                | `VITE_TILE_BASE_URL` for the dev server's proxy                                                      |
