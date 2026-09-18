---
name: bake-tiles
description: Bake terrain, POI and reverse-geocode pages from the geo DB into out/<kind>/<version> and verify the set before it is uploaded. Use when a bake pipeline or its source data changed, or a tile set needs (re)building. Uploading and deploying is the publish-bucket skill.
---

# Baking tiles

The deployed API answers forward search only. Terrain, POI and reverse
geocode are static objects in the bucket, one per L3 page, produced offline
by the tools in `crates/pxl/geo-bake` from the local geo database. This is
how each set gets built and checked; getting it into the bucket is
[publish-bucket](../publish-bucket/SKILL.md).

Background and the reasoning behind the design are in
[issue/20260816-deployment-hosting.md](../../../issue/20260816-deployment-hosting.md).
Read it before changing a pipeline's shape; this file is the procedure only.

## The three bakes

| Kind    | Tool                                           | Reads                                           | Output                                  | Empty page             |
| ------- | ---------------------------------------------- | ----------------------------------------------- | --------------------------------------- | ---------------------- |
| terrain | `deno task bake:tiles` (crates/pxl/geo-bake)   | Overture buildings/transportation/base, GEDTM30 | gzipped XTP, one per land page          | shared `_ocean`        |
| poi     | `deno task bake:poi` (crates/pxl/geo-bake)     | Overture places, terrain's `land_pages.json`    | gzipped XTP, one per page with POI      | no object (404 → `[]`) |
| geocode | `deno task bake:geocode` (crates/pxl/geo-bake) | Overture divisions, terrain's `land_pages.json` | gzipped JSON, one per page with a label | no object              |

Every tool writes to `./out/<kind>/<version>`, where the version defaults to
`PUBLISHED_VERSION.<kind>` in `spec/bake/published.json`
and the last path segment of `--out` overrides it. The object name is the
page's `page_idx` with no extension — the filename _is_ the object key.

Terrain goes first: POI and geocode do not classify land themselves, they
read the `land_pages.json` terrain leaves next to its output directory
(`--land-cache`, default `./out/terrain/land_pages.json`). Without a
terrain bake on this machine, fetch the published copy from
`s3://pixelet-tiles/_meta/land_pages.json` (see publish-bucket) — that is
what the monthly workflow does.

All three are **resumable**: an object is written to `.part` and renamed,
so an interrupted run leaves no truncated file, and a re-run skips what
already exists. POI and geocode also leave a local `.empty` marker for a
page that produced nothing, so a resumed run does not re-query it; the
marker is never uploaded.

## Before starting

The bakes read the geo database directly, so it must be up and populated:

```bash
docker compose -f infra/local/docker-compose.yml --env-file .env up -d
```

Terrain needs `overture_buildings`, `overture_transportation`,
`overture_base_{land,land_use,water,infrastructure}` and `elevation_raster`;
POI needs `overture_places`; geocode needs `overture_divisions` and
`overture_division_points`. If a table is missing, that is an ingestion
job, not a bake job — [infra/geo/README.md](../../../infra/geo/README.md).
Never `down -v` to "reset" the database; the volume is several gigabytes of
ingested data.

## Terrain

```bash
deno task bake:tiles --out ./out/terrain/<version> --concurrency 8
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

What it produces:

- one gzipped object per land-or-coastal page
- one `_ocean` object, the shared fallback every open-ocean page resolves to
- `manifest.json` and `land_pages.json` one level above the version directory

Open-ocean pages are deliberately not baked. Roughly two thirds of the
coverage bbox is open sea, and the client turns a 404 into the shared
`_ocean` tile. The land/ocean classification is cached in `land_pages.json`
and reused unless `--reclassify` is passed.

### Verifying

1. **Zero failures.** The final line reports `failed=`. Anything above zero
   means tiles are missing from the set; repeat the run (it skips the
   successes).
2. **Check the flat-elevation warning.** A tile whose elevation query returns
   the wrong shape is written successfully with a zero-filled grid — it bakes
   perfectly flat and never appears in `failed`. The summary prints a
   `WARNING: N of M tiles ... baked FLAT` line when this happens, and
   `manifest.json` records it under `run.flat_elevation_tiles`. A few hundred
   at the outer edge of the DEM coverage is expected (the padded sampling
   window falls outside the source raster there). A large number, or any away
   from the boundary, means something else is wrong.
3. **The ocean tile stayed page-independent.** The tool bakes an empty source
   at three different pages and aborts if the bytes differ. If it ever does,
   the single-fallback design no longer holds — stop and re-read the
   discussion doc rather than working around it.
4. **Spot-check a known tile against the dev tile server.** A baked page and a
   served one are byte-identical by construction, since both go through
   `crates/pxl/geo-bake/src/terrain/source.rs`. With `deno task dev` up:

   ```bash
   curl -s localhost:3001/geo/tiles/terrain/<page_idx> --compressed | shasum
   gunzip -c ./out/terrain/<version>/<page_idx> | shasum
   ```

   A mismatch means the two paths have drifted — investigate before publishing.

## POI and geocode

```bash
deno task bake:poi --out ./out/poi/<version>
deno task bake:geocode --out ./out/geocode/<version>
```

Both take `--out`, `--land-cache <path>`, `--concurrency <n>`, `--limit <n>`
and `--force` (re-fetch pages that already have an object or `.empty`
marker). Neither needs terrain's tables, only its `land_pages.json`.

Verify the same way: `failed=` at zero, and one page opened by hand —
`gunzip -c ./out/poi/<version>/<page_idx> | head -c 2 | xxd -p` is `5854`
(XTP), a geocode page is JSON.

A POI format change that does not touch the data used to be a **repack** —
re-encoding what is on disk without the database. That tool was v1/v2 JSON to
v3 XTP and went with the TypeScript; a format change now is a re-bake, which
at 300 pages is not the cost it was when the shortcut was written.

These two are also refreshed monthly without a human:
`.github/workflows/overture-monthly-refresh.yml` ingests Overture Places
and Divisions into a throwaway PostGIS, bakes both with `--force`, and
syncs the published version prefixes in place. It runs the same tasks this
file does, so a flag that works here works there; it carries both toolchains,
since the ingests are Deno and the bakes are Rust.

## Versions, and when the bake logic changes

`PUBLISHED_VERSION` is what the client requests. Bumping one there is the
whole change on the client's side; the bake tools read it for their
default output directory.

A published **terrain** version is immutable: a change to `bake_terrain` or
anything under `crates/pxl/geo-bake/src/terrain/` invalidates every existing tile
and means a full re-bake into a **new** version — not a partial one, because
a mixed bucket serves tiles from two pipelines with no way to tell them
apart.

A **POI or geocode** version names the page _format_, not the data: the
monthly refresh rewrites the same prefix with newer Overture content. A
format change (a new field, a new encoding) is a new version and a repack or
re-bake; a data refresh is not.

The live API caches baked tiles in memory, so during development a bake
change also needs the running server's cache dropped — or the server
restarted, the cache is purely in-process:

```bash
curl -X POST localhost:3000/api/internal/world/terrain/cache/invalidate -H "x-admin-token: $ADMIN_TOKEN"
# likewise .../world/poi/cache/invalidate and .../world/geocode/cache/invalidate
```
