---
name: bake-tiles
description: Bake terrain, POI and reverse-geocode pages from the geo DB into out/<kind>/<version> and verify the set before it is uploaded. Use when a bake pipeline or its source data changed, or a tile set needs (re)building. Uploading and deploying is the publish-bucket skill.
---

# Baking tiles

The deployed API answers forward search only. Terrain, POI and reverse
geocode are static objects in the bucket, produced offline
by the tools in `crates/pxl/server/geo-bake` from the local geo database. This is
how each set gets built and checked; getting it into the bucket is
[publish-bucket](../publish-bucket/SKILL.md).

Background and the reasoning behind the design are in
[issue/20260816-deployment-hosting.md](../../../issue/20260816-deployment-hosting.md).
Read it before changing a pipeline's shape; this file is the procedure only.

## The three bakes

| Kind    | Tool                     | Reads                                           | Output                                   | Empty page               |
| ------- | ------------------------ | ----------------------------------------------- | ---------------------------------------- | ------------------------ |
| terrain | `deno task bake:tiles`   | Overture buildings/transportation/base, GEDTM30 | packed XTP, one per land page            | shared `_ocean`          |
| poi     | `deno task bake:poi`     | Overture places + elevation, or canonical JSONL | bounded PXS1 summaries for one XY region | explicit zero-count root |
| geocode | `deno task bake:geocode` | Overture divisions, terrain's `land_pages.json` | packed JSON, one per page with a label   | no object                |

Every tool writes to `./out/<kind>/<version>`, where the version defaults to
`PUBLISHED_VERSION.<kind>` in `spec/pxl/spec/bake/published.json`
and the last path segment of `--out` overrides it.

The bucket names a page by its **quadkey** and nothing else (publish-bucket).
POI (inside each snapshot’s `tile/`) and geocode write that name straight out, so for them the filename _is_
the object key. Terrain does not: it writes `<level>/<page index>`, because it
clears a level's directory to redo it and to drop the levels a run did not
fold. Its names are translated at publish, by hard links, which is the one
step POI and geocode skip.

Terrain goes first for geocode: it does not classify land itself, it
read the `land_pages.json` terrain leaves next to its output directory
(`--land-cache`, default `./out/terrain/land_pages.json`). Without a
terrain bake on this machine, fetch the published copy from
`s3://pixelet-tiles/_meta/land_pages.json` (see publish-bucket) — that is
what the monthly workflow does.

Every object is packed in the encoding `util.rs`'s `CONTENT_ENCODING` names —
zstd — and each manifest says so in `content_encoding`, which is what the
upload labels the objects with. A version baked before zstd (terrain v4,
POI v5, geocode v2) is gzip, and a bake refuses to resume into it: one
version holds one encoding.

Terrain and geocode are **resumable**: an object is written to `.part` and renamed,
so an interrupted run leaves no truncated file, and a re-run skips what
already exists. Geocode also leaves a local `.empty` marker for a
page that produced nothing, so a resumed run does not re-query it; the
marker is never uploaded.

## Before starting

The bakes read the geo database directly, so it must be up and populated:

```bash
docker compose -f infra/local/docker-compose.yml --env-file .env up -d
```

Terrain needs `overture_buildings`, `overture_transportation`,
`overture_base_{land,land_use,water,infrastructure}` and `elevation_raster`;
POI needs `overture_places` and `elevation_raster`; geocode needs `overture_divisions` and
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

- `0/`, one packed object per land-or-coastal page — the leaves
- `1/`, `2/`, … each level folded from the one below it until a level is a
  single page, refolded whole on every run (they are a function of the leaves,
  so a resumed run never leaves a stale pyramid over them)
- one `_ocean` object beside the levels, the shared fallback every open-ocean
  page resolves to at every level; the run aborts if folding it does not give
  it back
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
4. **Spot-check pages against the dev tile server, at more than one level.**
   The dev server demand-pages terrain out of the same functions this tool
   runs (`terrain/leaf.rs` for a leaf, `terrain/pyramid.rs` for a level above),
   so a page it serves holds the packet this tool wrote. It packs at zstd's
   quick level where the bake packs at its stored one, so compare the packets,
   not the objects. With `deno task dev:tiles` up — not `dev`, which proxies
   every route to the live deployment:

   The dev server answers the bucket's addresses, so it takes the quadkey
   while the file on disk is still under its level — one page, two names:

   ```bash
   curl -s localhost:3001/geo/tiles/terrain/<quadkey> | zstd -dcq | shasum
   zstd -dcq ./out/terrain/<version>/<level>/<page_idx> | shasum   # gunzip -c before zstd
   ```

   A level-k page makes the dev server fault the pages under it first, so the
   first ask above level 3 takes seconds to a minute. A mismatch means the two
   paths have drifted — investigate before publishing.

## POI

The development slice bakes one 15-digit XY region down to the transport's
23-digit resolution, stopping earlier where XY cannot distinguish members.
Raw membership does not enlarge a packet. A terminal summary is still a
count and bounds, not a bounded list of entities.

```bash
deno task bake:poi --root <15-digit-quadkey> --revision <nonzero-u64> --out ./out/poi/<snapshot>
# To verify without a database:
deno task bake:poi --root <15-digit-quadkey> --revision <nonzero-u64> --input /tmp/canonical.jsonl --out ./out/poi/<snapshot>
```

Canonical input is JSONL sorted by `id`, with `id`, `position: [x,y,z]`
and `text`. XYZ uses absolute canonical units. Exact duplicate identities
count once; conflicting identities, missing elevation and out-of-region
records fail the bake. Overture acquisition streams a repeatable-read
transaction and resolves elevation before normalization.

The snapshot's `tile/` directory holds zstd-compressed nodes named by quadkey,
including the 15-digit region root. Each decoded packet is at most 341 bytes.
`canonical.jsonl`, `source.jsonl` and `manifest.json` beside `tile/` hold private
backing, provenance and completion metadata. Only `tile/` is publishable.
The whole snapshot is staged and renamed together. Existing output or staging
files are refused; inspect an interrupted run and choose a fresh output name.

Verify the manifest's root, revision and count. Decompress `tile/<root>`:
its first four bytes must be `PXS1`. In `dev:local`, the tile server creates
requested regions from the DB through the same snapshot writer. No snapshot
path override is needed. Enable `show_poi` in development settings; it defaults
off and is unavailable in production. The kernel discovers a bounded number
of nearby visible region addresses and requests their summaries. Offscreen
branches take no candidate slots; residency has its own byte budget. Parents
remain until a complete relevant child cut is usable and fits. A missing
response is not an empty region; empty regions have zero-count root packets.
The production smoke check does not require this development snapshot.

## Geocode

```bash
deno task bake:geocode --out ./out/geocode/<version>
```

It takes `--out`, `--land-cache <path>`, `--concurrency <n>`, `--limit <n>`
and `--force` (re-fetch existing pages and `.empty` markers). It needs
terrain's `land_pages.json`, not its tables.

Verify `failed=` is zero and decompress a page to check its JSON. A group's
name is a page's first eleven digits. The monthly workflow ingests Places
for forward search and Divisions for geocode, then refreshes geocode only.
POI snapshots are baked separately and remain immutable.

## Versions, and when the bake logic changes

`PUBLISHED_VERSION` is what the client requests. Bumping one there is the
whole change on the client's side; the bake tools read it for their
default output directory.

A published **terrain** version is immutable: a change to `bake_terrain` or
anything under `crates/pxl/server/geo-bake/src/terrain/` invalidates every existing tile
and means a full re-bake into a **new** version — not a partial one, because
a mixed bucket serves tiles from two pipelines with no way to tell them
apart.

A **geocode** version names the page _format_, not the data: the
monthly refresh rewrites the same prefix with newer Overture content. A
format change (a new field, a new encoding) is a new version and a re-bake; a
data refresh is not.

POI snapshots require a new published version prefix even for a data refresh,
so root and refinement packets cannot mix snapshots.

The local tile server shares one demand path across terrain, POI and geocode
(`geo-bake/src/demand/`). `--cache` is the parent directory, default `out/dev`;
each kind owns a private generation beneath it. Concurrent requests join the
same job, packets are written atomically, and responses carry zstd encoding.
Source data must stay unchanged within a running generation. After re-ingest
or a bake change, restart or invalidate the affected kind as a whole:

```bash
curl -X POST localhost:3001/geo/internal/world/terrain/cache/invalidate -H "x-admin-token: $ADMIN_TOKEN"
# Likewise .../world/poi/... and .../world/geocode/...; send no body.
# The header is checked only when ADMIN_TOKEN is set.
```

Invalidation swaps the generation. Old requests can finish against their old
directory but cannot publish into the new one; the old directory is removed
when its last job finishes. Geocode's dictionary and groups share a generation;
a POI region's backing and descendants share a completed snapshot. A client
that encounters a new POI revision resolves that region again.
