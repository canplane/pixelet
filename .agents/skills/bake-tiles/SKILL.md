---
name: bake-tiles
description: Pre-bake terrain tiles from the geo DB into gzipped static objects, verify them, and publish a new version to the tile bucket. Use when the terrain bake pipeline changed, the Overture/DEM source data changed, or the demo needs its tile set (re)built for deployment.
---

# Baking and publishing terrain tiles

The deployed demo has no API server: terrain comes from static objects in a
bucket. This is how that bucket gets built and republished.

Background and the reasoning behind this design are in
[discussions/20260816-deployment-hosting.md](../../../discussions/20260816-deployment-hosting.md).
Read it before changing the pipeline's shape; this file is the procedure only.

## Before starting

The bake reads the geo database directly, so it must be up and populated:

```bash
docker compose -f infra/docker-compose.yml --env-file .env up -d
```

`pixelet-db-geo` must hold `overture_buildings`, `overture_transportation`,
`overture_base_land_use`/`overture_base_water`/`overture_base_land`/
`overture_base_infrastructure`, `osm_coastline_polygons` (still OSM — the
coarse land/ocean page pre-filter only, see
[infra/geo/README.md](../../../infra/geo/README.md)), and `esa_elevation_raster`
(GEDTM30). If it doesn't, that's an ingestion job, not a bake job — see that
same README. Never `down -v` to "reset" it; the volume is several gigabytes
of ingested data.

## Baking

```bash
deno run -A --env=.env apps/api/src/bake/terrain/tools/bake_tiles.ts \
  --out ./out/tiles/v1 --concurrency 8
```

| Flag                | Meaning                                                         |
| ------------------- | --------------------------------------------------------------- |
| `--out <dir>`       | output directory; its last path segment is the version          |
| `--concurrency <n>` | parallel bakes — match `DB_POOL_SIZE`, the DB is the bottleneck |
| `--limit <n>`       | stop after n tiles (smoke test)                                 |
| `--force`           | re-bake tiles already on disk                                   |
| `--reclassify`      | ignore the cached land/ocean scan and redo it                   |

It runs long enough to want a log file rather than a terminal:

```bash
... > /tmp/bake.log 2>&1 &
grep '^\[bake\] ' /tmp/bake.log | tail -2
```

**The run is resumable.** Tiles are written to `.part` and renamed, so an
interrupted bake leaves no truncated file that a later run would mistake for
finished work. Re-running skips what already exists. The land/ocean
classification is cached in `land_pages.json` next to the output directory and
is reused unless `--reclassify` is passed.

What it produces:

- one gzipped object per land-or-coastal page, named by its `page_idx` with no
  extension — the filename _is_ the object key
- one `_ocean` object, the shared fallback every open-ocean page resolves to
- `manifest.json` and `land_pages.json` one level above the version directory

Open-ocean pages are deliberately not baked. Roughly two thirds of the coverage
bbox is open sea, and the client turns a 404 into the shared `_ocean` tile.

## Verifying before upload

1. **Zero failures.** The final line reports `failed=`. Anything above zero
   means tiles are missing from the set, and the run should be repeated (it will
   skip the successes).
2. **Check the flat-elevation warning.** A tile whose elevation query returns the
   wrong shape is written successfully with a zero-filled grid — it bakes
   perfectly flat and never appears in `failed`. The summary prints a `WARNING:
   N of M tiles ... baked FLAT` line when this happens, and `manifest.json`
   records it under `run.flat_elevation_tiles`. A few hundred at the outer edge
   of the DEM coverage is expected and accepted (the padded sampling window falls
   outside the source raster there). A large number, or any away from the
   boundary, means something else is wrong.
3. **The ocean tile stayed page-independent.** The tool bakes an empty source at
   three different pages and aborts if the bytes differ. If it ever does abort,
   the single-fallback design no longer holds — stop and re-read the discussion
   doc rather than working around it.
4. **Spot-check a known tile against the live API.** Static and live tiles are
   supposed to be byte-identical, since both go through
   `apps/api/src/bake/terrain/source.ts`. With the API running:

   ```bash
   curl -s localhost:3000/api/world/terrain/tiles/<page_idx> --compressed | shasum
   gunzip -c ./out/tiles/v1/<page_idx> | shasum
   ```

   A mismatch means the two paths have drifted, which should be impossible by
   construction — investigate before publishing.

## Publishing

Tiles are immutable per version. **Never overwrite an existing version in
place** — publish a new prefix and point the app at it. That invalidates the CDN
edge, browser caches, and any future OPFS cache at once, with no purge call.

The deployment is one Cloudflare Worker serving both the built app and the
tiles (`apps/web/wrangler.jsonc`), so tiles live on the app's own hostname
under `/tiles/`. That is what makes them same-origin: there is no CORS policy
to configure, and `VITE_TILE_BASE_URL` stays a relative path that survives a
domain change.

### One-time setup

Done in the Cloudflare dashboard, not from here:

1. Create the R2 bucket. Its name must match `r2_buckets[0].bucket_name` in
   `wrangler.jsonc`. Keep it **private** — the Worker reads it through a
   binding, so it never needs to be publicly readable.
2. Create an R2 API token (S3-compatible Access Key ID + Secret) for uploads.
3. Note the account ID; the S3 endpoint is
   `https://<account_id>.r2.cloudflarestorage.com`.

Store the credentials in a dedicated AWS CLI profile so an existing default
profile is left alone:

```bash
aws configure --profile r2          # region: auto
```

### Upload

`aws s3 sync` is used rather than `rclone` because the AWS CLI is already
present; either works, since R2 is S3-compatible. `wrangler r2 object put`
handles one object per invocation and is not usable at this scale.

```bash
aws s3 sync ./out/tiles/v1 s3://<bucket>/v1 \
  --profile r2 \
  --endpoint-url https://<account_id>.r2.cloudflarestorage.com \
  --content-type application/octet-stream \
  --content-encoding gzip \
  --cache-control "public, max-age=31536000, immutable"
```

**Objects are stored already gzipped, so the upload must set
`Content-Encoding: gzip`.** Without it the client receives compressed bytes it
does not know to decode. This is the single easiest step to get wrong, and it
fails as a decode error far from its cause.

If the AWS CLI rejects the upload over checksums (v2 sends `x-amz-checksum-*`
headers that some S3-compatible stores refuse), set
`AWS_REQUEST_CHECKSUM_CALCULATION=when_required` for the command.

### Verify before pointing the app at it

```bash
# object count should match the manifest's `objects`
aws s3 ls s3://<bucket>/v1/ --profile r2 --endpoint-url <endpoint> --recursive | wc -l

# the shared ocean tile must exist
aws s3api head-object --bucket <bucket> --key v1/_ocean \
  --profile r2 --endpoint-url <endpoint>

# and metadata must have survived: expect ContentEncoding "gzip"
aws s3api head-object --bucket <bucket> --key v1/<some_page_idx> \
  --profile r2 --endpoint-url <endpoint>
```

### Deploy

```bash
# apps/web/.env -> VITE_TILE_BASE_URL=/tiles/v1
deno task build:web
cd apps/web && npx wrangler deploy
```

Then attach the custom domain to the Worker in the dashboard.

**Check the served bytes, not just the stored ones.** The bucket can be
perfectly correct while the serving path is not:

```bash
U=https://<host>/tiles/v1/<page_idx>
curl -s -H "Accept-Encoding: gzip" -D - -o /dev/null "$U"   # expect content-encoding: gzip
curl -s --compressed "$U" | head -c 2 | xxd -p              # expect 5854 (XT), never 1f8b
```

A response whose gzip body is _smaller_ than the stored object means the
runtime compressed it a second time — `encodeBody: "manual"` is missing from
the Worker's `Response`. A body that still starts `1f8b` after `--compressed`
means the same thing from the other direction.

**Request each tile twice.** Edge caching sits in front of R2, and
`encodeBody: "manual"` does not survive the cache round trip — a first request
can be correct while every cached hit after it is double-compressed. Compare
both responses against the stored size, and check `cf-cache-status` to confirm
the second one actually hit.

Also confirm an unbaked ocean page returns **404 `text/plain`**, not
`index.html`: that is what a missing `run_worker_first` produces, and the
client would try to decode the HTML as terrain.

Old version prefixes simply stop being requested; delete them whenever.

## When the bake logic changes

A change to `bake_terrain` or anything under `apps/api/src/bake/terrain/`
invalidates every existing tile. That means a full re-bake into a new version —
not a partial one — because a mixed-version bucket serves tiles from two
different pipelines with no way to tell them apart.

The live API caches baked tiles in memory, so during development a bake change
also needs the running server's cache dropped:

```bash
curl -X POST localhost:3000/api/internal/world/terrain/cache/invalidate \
  -H "x-admin-token: $ADMIN_TOKEN"
```

Restarting the server does the same thing — the cache is purely in-process.
