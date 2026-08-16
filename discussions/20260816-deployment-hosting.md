# Serverless Live-Demo Deployment

**2026-08-16** — how the demo gets deployed without a server, what it costs, and
which parts of the current architecture had to change to make that true.

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

Classifying the coverage bbox against `osm_coastline_polygons` (the same
predicate `repository.ts`'s `OSM_COAST_SQL` already uses, so "has land" means
exactly what it means to the live bake path) splits it three ways:

| Page kind | Land polygons touching its bbox | Count |
| --- | --- | --- |
| Land or coastal — must be baked | ≥ 1 | **43,505** |
| Pure open ocean — nothing but water | 0 | **80,230** |
| | | 123,735 total |

**65% of the bbox is open sea.** Baking and uploading 80,230 copies of "flat
water" would triple the dataset for no information.

Instead the bucket holds one `_ocean` object. The client requests a tile,
gets a 404 for any page that wasn't baked, and resolves that to the shared
tile. A 404 here isn't an error — it's the signal for "nothing but water here".

This is only sound if the ocean tile is genuinely page-independent, which is
not obvious: `bake_terrain` takes a `page_idx` and derives both a bbox and a
latitude-dependent `Z_SCALE` from it. So the tool **verifies it rather than
assuming it** — it bakes a synthesised empty source (zero elevation grid, no
features, no coastline rows) at three different pages and refuses to proceed
unless all three produce identical bytes. They do.

Synthesising the empty source, rather than sampling some far-offshore page from
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

Object stores don't compress on the fly. Each tile is gzipped at bake time and
uploaded with `content-type: application/octet-stream` +
`content-encoding: gzip`; `fetch()` decompresses transparently, so the client
needs no change. Since versioned tiles are immutable, they can also carry
`cache-control: public, max-age=31536000, immutable`.

This matters more than expected — see the numbers below.

## Measured numbers

Full bake against the local geo DB, concurrency 8, completed:

| | Value |
| --- | --- |
| Land tiles baked | **43,505**, zero failures |
| Ocean pages skipped | 80,230 (one shared `_ocean` object instead) |
| Objects to upload | 43,506 (tiles + `_ocean`) |
| Raw, uncompressed | 170.24 GB |
| **Gzipped total** | **892.5 MB** |
| Gzipped per tile | 21.0 kB average — **0.5% of raw** |
| Wall clock | 36m58s |
| App bundle | 1.97 MB (655 kB gzipped) |

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
inward neighbours shows why that would be worse:

- The neighbour one page inside the ring carries real elevation (96–344 in the
  sampled tile). So the geometric discontinuity at the boundary exists either
  way — dropping the ring does not remove the cliff, since the ocean fallback is
  also flat at zero.
- Dropping it *adds* a second error on top: land recolored as sea. Composition
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

| | interior (Seoul area) | periphery (toward Japan / China / North Korea) |
| --- | --- | --- |
| `land` channel | 15–99% landuse classes | **only SEA and BARE, 0% everything else** |
| structure pixels | 184 – 184,000 | **0** |
| transport pixels | 20,000 – 218,000 | **0** |

`osm_coastline_polygons` is global, so any page touching a landmass classifies as
land and gets baked — but the OSM *feature* extract is Geofabrik's South Korea
file. Those pages therefore bake into a bare land/sea silhouette and nothing
else.

The elevation data behaves the same way, and less visibly. The DEM is four
GeoTIFFs covering mainland South Korea, Jeju, the far western islands, and
Ulleungdo/Dokdo — they do not tile the full 124–132°E / 33–39°N rectangle. The
ingestion pipeline builds a VRT over their union bounding box, so the mosaic
*reports* full-rectangle coverage while the gaps hold zeros that are
indistinguishable from sea level downstream. Verified by probing the raster
directly: Jeju returns 934 m, a point in one of the implied gaps returns 0.

So the periphery is flat as well as featureless, and nothing in the data marks
the difference between "sea level" and "no data here". Correct for a South Korea
demo — and the reason the four boxes were chosen — but worth knowing before
reading anything into terrain outside them.

Size distribution over the 43,505 tiles confirms the scale:

| | |
| --- | --- |
| min / p10 | 4,190 B — identical in size to the `_ocean` tile, i.e. fully uniform |
| median | 19,331 B |
| max | 172,927 B |
| under 6 kB (effectively featureless) | **6,949 tiles (16.0%)** |
| total bytes in those | **29 MB** |

**16% of the tile count, 3% of the bytes** — featureless tiles compress to almost
nothing, which is exactly why pruning them isn't worth doing. Panning east, south
or north out of Korea yields plain bare terrain rather than an error or a hard
world edge, which is the better failure mode anyway.

## Hosting comparison

**Precondition that decides most of it:** the app bundle is 655 kB gzipped, so
its bandwidth is negligible anywhere. Effectively all traffic is tiles, and
tiles go to object storage regardless of host. So the free-bandwidth numbers the
three providers advertise barely enter into it.

| | Cloudflare Pages | Vercel (Hobby) | Netlify (Free) |
| --- | --- | --- | --- |
| Static bandwidth | Unlimited | 100 GB/mo | 100 GB/mo |
| Builds | 500/mo | ample | 300 min/mo |
| Files per deployment | 20,000 cap | effectively unbounded | effectively unbounded |
| Commercial use on free tier | allowed | **prohibited** | allowed |
| Same account as R2 | **yes** | no | no |
| CORS needed for tiles | **avoidable** | required | required |
| DX | fine | best | good |

Three things actually differentiate them:

**CORS avoidance.** With the app on Vercel or Netlify, app origin ≠ tile origin,
so the bucket needs a CORS policy naming the app domain — extra state to keep in
sync per domain change, and its absence fails *only* in production, never
locally. On Cloudflare the bucket can be bound to a path on the app's own
hostname (`pixelet.app/tiles/*`), making tile requests same-origin, so CORS never
enters the picture. `VITE_TILE_BASE_URL=/tiles/v1` then suffices.

**File-count ceiling.** Cloudflare Pages caps a deployment at 20,000 files, so
43,505 tiles can't live there even in principle. Vercel and Netlify permit it but
would make builds absurd and would meter tile traffic against the 100 GB
allowance — a session panning around pulls tens of megabytes, so that allowance
is a number that genuinely depletes. **Whichever host is picked, tiles belong in
object storage, not in the deployment.**

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

> Pricing figures are from model knowledge and should be re-checked against the
> providers' current pages before committing. Note also that Cloudflare has been
> steering new projects toward Workers-with-static-assets rather than Pages, so
> confirm which path the current documentation describes.

### Uploading

43,506 objects rules out `wrangler r2 object put`, which handles one object per
invocation. R2 is S3-compatible, so `rclone copy` or `aws s3 sync` is the
practical route, setting `content-encoding: gzip`,
`content-type: application/octet-stream`, and the immutable `cache-control` on
the whole prefix.

## Deliberately not addressed

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

**API hardening remains parked.** Rate limiting, timeouts, and circuit breaking
were judged out of scope (see project notes). They're moot for the static
deployment, which has no API to harden. The in-process LRU cache and
single-flight added to the live tile endpoint stay useful for development and for
any future non-static path.

## Pre-deployment secret audit

Run against the built bundle rather than reasoned about, since the whole point
is what actually ships.

| Check | Result |
| --- | --- |
| All 21 root `.env` variable **names** in the bundle (`ADMIN_TOKEN`, `DB_*_PASSWORD`, `DB_*_URL`, `postgresql://`, …) | **0 hits** |
| All root `.env` **values** matched literally against the bundle | only `DB_MAIN_HOST`/`DB_GEO_HOST`, whose value is `localhost` — a false positive |
| Source maps in `dist/` | **0** (`sourcemap: mode !== "production"`) |
| Stray files in `dist/` | none — bundle, css, fonts, favicons, locales |

Structurally this holds because Vite's `loadEnv(mode, cwd, "VITE_")` only reads
`VITE_`-prefixed variables, so the database credentials are never loaded into
the build at all. And since the API is not deployed, `ADMIN_TOKEN` and
`/api/internal/*` have no route to the internet in the first place.

Two secret-looking literals in the bundle were checked and are benign: React's
own list of input types (which contains `password`), and the deliberate
attribution link in the licence section.

**The sensitive surface is not secrets, it is location.** The only personal data
leaving the client is the map-centre coordinate sent to Nominatim — rounded to
four decimal places (~11 m) and debounced, never stored. That is now disclosed
in the settings licence section, translated rather than hardcoded, because it is
a statement to the user rather than a notice a licence demands verbatim.

Everything else stays on the device: `pxl:lang`, `pxl:render_settings`,
`pxl:debug_settings`, and `pxl:objects:v4` in `localStorage`.

Two items remain for deploy time: the R2 bucket must be public **read-only**
(its URL is baked into the bundle by design), and if the API is ever deployed,
`ADMIN_TOKEN` must be regenerated rather than carried over from development.

## Changes this produced

| File | Change |
| --- | --- |
| `apps/api/src/modules/world/terrain/source.ts` | new — tile assembly extracted so offline bake and live API share one path |
| `apps/api/src/modules/world/terrain/service.ts` | uses the extracted source instead of its own copy |
| `apps/api/src/bake/terrain/tools/bake_tiles.ts` | new — classify, bake, gzip, resumable, writes a manifest |
| `apps/web/vite.config.ts` | reads `VITE_TILE_BASE_URL` → `__TILE_BASE_URL__` |
| `apps/web/@types/index.d.ts` | declares the new build constant |
| `apps/web/.env.example` | documents the tile origin and its versioning rule |
| `apps/web/src/modules/world/terrain/use_tile_fetcher.ts` | tile-origin URL builder + 404 → shared ocean tile |
