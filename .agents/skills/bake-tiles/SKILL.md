---
name: bake-tiles
description: Pre-bake terrain tiles from the geo DB into gzipped static objects, verify them, and publish a new version to the tile bucket. Use when the terrain bake pipeline changed, the OSM/DEM source data changed, or the demo needs its tile set (re)built for deployment.
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

`pixelet-db-geo` must hold `osm_elements`, `osm_coastline_polygons`, and
`esa_elevation_raster`. If it doesn't, that's an ingestion job, not a bake job —
see [infra/geo/README.md](../../../infra/geo/README.md). Never `down -v` to "reset"
it; the volume is several gigabytes of ingested data.

## Baking

```bash
deno run -A --env=.env apps/api/src/bake/terrain/tools/bake_tiles.ts \
  --out ./out/tiles/v1 --concurrency 8
```

| Flag | Meaning |
| --- | --- |
| `--out <dir>` | output directory; its last path segment is the version |
| `--concurrency <n>` | parallel bakes — match `DB_POOL_SIZE`, the DB is the bottleneck |
| `--limit <n>` | stop after n tiles (smoke test) |
| `--force` | re-bake tiles already on disk |
| `--reclassify` | ignore the cached land/ocean scan and redo it |

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
  extension — the filename *is* the object key
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
2. **The ocean tile stayed page-independent.** The tool bakes an empty source at
   three different pages and aborts if the bytes differ. If it ever does abort,
   the single-fallback design no longer holds — stop and re-read the discussion
   doc rather than working around it.
3. **Spot-check a known tile against the live API.** Static and live tiles are
   supposed to be byte-identical, since both go through
   `apps/api/src/modules/world/terrain/source.ts`. With the API running:

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

```bash
# 1. bake into the new version directory
deno run -A --env=.env apps/api/src/bake/terrain/tools/bake_tiles.ts --out ./out/tiles/v2

# 2. upload the whole prefix, pre-gzipped
rclone copy ./out/tiles/v2 r2:<bucket>/v2 \
  --header-upload "Content-Encoding: gzip" \
  --header-upload "Content-Type: application/octet-stream" \
  --header-upload "Cache-Control: public, max-age=31536000, immutable" \
  --transfers 32

# 3. point the app at it and rebuild
#    apps/web/.env  ->  VITE_TILE_BASE_URL=<origin>/v2
deno task build:web
```

Objects are stored already gzipped, so the upload must set
`Content-Encoding: gzip` or clients will receive compressed bytes and fail to
decode them. This is the single easiest step to get wrong.

`wrangler r2 object put` handles one object per invocation and is not usable at
this scale; use an S3-compatible bulk tool.

If the app and the tiles end up on different origins, the bucket also needs a
CORS policy naming the app's domain. Same-origin serving (binding the bucket to
a path on the app's own hostname) avoids that entirely, and
`VITE_TILE_BASE_URL` accepts a relative value like `/tiles/v2` for exactly that
case.

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
