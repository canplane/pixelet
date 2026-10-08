---
name: bake-tiles
description: Bake terrain, POI and reverse-geocode tiles from the geo DB into out/<kind>/<version> and verify the set before it is uploaded. Use when a bake pipeline or its source data changed, or a tile set needs (re)building. Uploading and deploying is the publish-bucket skill.
---

# Baking tiles

The deployed API answers forward search only. Terrain, POI and reverse
geocode are static objects in the bucket, produced offline
by the tools in `crates/server/geo-bake` from the local geo database. This is
how each set gets built and checked; getting it into the bucket is
[publish-bucket](../publish-bucket/SKILL.md).

Background and the reasoning behind the design are in
[decision/20260816-deployment-hosting.md](../../../decision/20260816-deployment-hosting.md).
Read it before changing a pipeline's shape; this file is the procedure only.

## The three bakes

| Kind    | Tool                     | Reads                                           | Output                                                             | Empty tile |
| ------- | ------------------------ | ----------------------------------------------- | ------------------------------------------------------------------ | ---------- |
| terrain | `deno task bake:tiles`   | Overture buildings/transportation/base, GEDTM30 | packed XTP, one per land tile                                      | no object  |
| poi     | `deno task bake:poi`     | Overture places                                 | XTP counts, where a crowd splits                                   | no object  |
| geocode | `deno task bake:geocode` | Overture divisions, terrain's `land_tiles.json` | XTP: a dictionary, and one per group of 16 × 16 tiles with a label | no object  |

Every tool writes to `./out/<kind>/<version>`. The version is
`v<format>.<day>.<n>`: the kind's `FORMAT` in
`spec/spec/bake/published.json`, the day the bake ran (UTC) and that
day's count of bakes of the kind, from 1 — `v7.2026-10-04.1`
(`pxl-spec bake/published.spec.rs` `version_name`). What it was baked from
is the manifest's: the Overture release its tables were ingested from and the
commit it was built at. The last path segment of `--out` overrides it.

A tile is named by its 2D Morton key written a digit a zoom, and every bake
writes that name straight out, flat in its version: the filename _is_ the
object key, and a name's length is its zoom (publish-bucket).

Terrain goes first for geocode: it does not classify land itself, it
read the `land_tiles.json` terrain leaves next to its output directory
(`--land-cache`, default `./out/terrain/land_tiles.json`). Without a
terrain bake on this machine, fetch the published copy from
`s3://pixelet-tiles/_meta/land_tiles.json` (see publish-bucket).

Every object is packed in the encoding `util.rs`'s `CONTENT_ENCODING` names —
zstd — and each manifest says so in `content_encoding`, which is what the
upload labels the objects with. A version baked before zstd (terrain v4,
POI v5, geocode v2) is gzip, and a bake refuses to resume into it: one
version holds one encoding.

Terrain and geocode are **resumable**: an object is written to `.part` and renamed,
so an interrupted run leaves no truncated file, and a re-run skips what
already exists. Geocode also leaves a local `.empty` marker for a
tile that produced nothing, so a resumed run does not re-query it; the
marker is never uploaded.

## Before starting

The bakes read the geo database directly, so it must be up and populated:

```bash
docker compose -f infra/local/docker-compose.yml --env-file .env up -d
```

Terrain needs `overture_buildings`, `overture_transportation`,
`overture_base_{land,land_use,water,infrastructure}` and `elevation_raster`;
POI needs `overture_places`; geocode needs `overture_division_areas` and
`overture_divisions`. If a table is missing, that is an ingestion
job, not a bake job — [infra/geo/README.md](../../../infra/geo/README.md).
Every bake reaches the DB only through `geo-bake coverage.rs` `Overture`,
which takes the `overture_*` tables from the statements the bake runs and
refuses unless each holds exactly the coverage's shards in `overture_catalog`,
all of one release; the elevation must hold exactly them.
The error names the ingest to run.
Never `down -v` to "reset" the database; the volume is several gigabytes of
ingested data.

## Terrain

A tile is baked in two stages:

- **leaf** — each tile exactly as the dev server bakes it on demand
  (`geo-bake terrain/leaf.rs` `bake_leaf`): its deck solve (`deck.rs`) —
  every structure reaching the tile or the eight around it, solved whole
  wherever it runs — and from that one result its lines (decks, bores and
  the grades at their ends) and its surface (ground and elevation), which
  paints a way only where no line draws it. Resumable.
- **assemble** — the leaves linked into the version, and the pyramid folded
  over them, with no database. Whole every time.

A first bake, or one after an ingest:

```bash
deno task bake:tiles --out ./out/terrain/<version> --concurrency 8
```

After a change to how decks or bores are solved, the leaves again, since the
surface paints around what the solve carries:

```bash
deno task bake:tiles --out ./out/terrain/<version> --force
```

| Flag                | Meaning                                                            |
| ------------------- | ------------------------------------------------------------------ |
| `--stage <s>`       | `leaf` or `assemble`, repeatable; both when omitted                |
| `--out <dir>`       | published tiles; its last path segment is the version              |
| `--build <dir>`     | the artifacts (default `./out/build/terrain`)                      |
| `--concurrency <n>` | parallel tiles — match `DB_POOL_SIZE`, the DB is the bottleneck    |
| `--limit <n>`       | stop after n tiles (smoke test)                                    |
| `--force`           | bake leaves already on disk again, and solve every structure again |
| `--reclassify`      | ignore the cached land/ocean scan and redo it                      |
| `--region <name>`   | bake again only the land tiles under a tile, repeatable            |
| `--base <dir>`      | the version a region is baked over (default the published one)     |
| `--grow`            | a region bake of the coverage's shards the base does not hold      |

It runs long enough to want a log file rather than a terminal:

```bash
... > /tmp/bake.log 2>&1 &
grep '^\[bake\] ' /tmp/bake.log | tail -2
```

A fix that reaches one area need not bake every tile again. `--region`
bakes the land tiles under a tile's name again, by the code as it is now,
and assembles a new version from `--base`: the base's tiles linked whole, the
region's leaves in place of the ones that differ, and folded again only the
tiles above whose `Layer::sources` read a tile that changed, a zoom at a time,
until a zoom changes nothing. Minutes, where a full bake is hours:

```bash
deno task bake:tiles --region 010332102130 --base ./out/terrain/<version>
```

Every tile it writes is what a full bake of the same code writes; the tiles
outside the region keep the base's, so **the base must be of this code**: a
version names the binary that baked it in its `_built`, and a region bake
refuses a base of other code. A change to what a tile is made of is a full
bake. `_built` also lists the shards the version covers, so when the
coverage grows, `--grow` bakes exactly the shards it gained over the base and
refuses a base holding a shard the coverage dropped. And **the region must
hold everything the change reaches**: a structure solved again that runs out of the region meets
the base's solve of it at the edge. Choosing the region is the caller's job.
The manifest records the base and the region's release, which may differ.

The new version is complete on disk, and also a **partial snapshot**: its
`_snapshot` lists the full version it overlays and the tiles it writes or
removes over it, so the bucket holds only those
([20261004-a-snapshot-of-a-subtree.md](../../../decision/20261004-a-snapshot-of-a-subtree.md)).
A region baked over a partial version lists its changes over that one's
full base, earlier partial's included.

What it produces, beside `leaf/` under `--build` with a `leaf.json` of what
built it — one code and one source a directory: the leaf stage removes
another's leaves rather than resume over them, and assemble refuses leaves
not all of its own code:

- `z15/`, one packed object per tile anything is drawn in — land, or a way,
  building, deck or bore — the leaves
- `z14/`, `z13/`, … each zoom folded from the one below it until a zoom is a
  single tile or `z5`, refolded whole on every run (they are a function of the leaves,
  so a resumed run never leaves a stale pyramid over them)
- `manifest.json` and `land_tiles.json` in the directory above the version's

Open-ocean tiles are deliberately not baked. Roughly two thirds of the
coverage bbox is open sea, and a tile with no object is drawn from the one
over it. The fold reads a tile it has no object for as open sea inside the
coverage and, past it, as unknown, a cell with no surface, sea or land, so
the planet's picture is seen under it; and a coarser one as the fold of
those under it (`terrain/absent.rs`). A tile counts as open ocean only when nothing is drawn in it:
a bridge or an undersea tunnel over open water makes its tiles leaves. The
classification is cached in `land_tiles.json`, keyed by the tile size, the
coverage, the release and the rule that classified it (`terrain/leaf.rs`
`land_tiles`); a list made under any other is refused and made again.

**A tile is baked from more than its own bbox, and cropped to it.** What is
drawn in a tile can come from past its edge, so every read reaches as far as
that, worked back from how far each thing draws:

- **Rows** (ways, buildings, decks, bores, water) are read from the tile's
  bbox grown by `DRAWN_REACH_U` (terrain/repository.rs `drawn_from`): a way
  drawn its widest, `WIDEST` cells (the width a deck's byte holds, and the
  ground draws no wider), plus the corridor gap one way paves to another.
  The tile keeps only its own cells, and its lines' runs within their
  footprint (pxl-spec bake/deck.rs `footprint`).
- **Elevation** is sampled `ELE_PAD` samples past the tile, what a bicubic
  window at the tile's edge reads. The published shards
  (`infra/geo/elevation/metadata.json`) are warped each to its own edges, so
  a padded grid at a shard's edge reads its neighbor's raster, and at the
  coverage's outer edge finds none.

### Verifying

1. **Zero failures.** Each stage's line reports `failed=`. Anything above
   zero in `surface` means tiles are missing; repeat the stage (it skips the
   successes). `assemble` refuses to finish with a tile missing.
2. **Check the flat-elevation warning.** A tile the elevation raster does not
   reach is written successfully with a zero-filled grid — it bakes perfectly
   flat and never appears in `failed`. The `leaf` stage prints a
   `WARNING: N tiles ... baked FLAT` line for the tiles that run baked, and
   nothing records it: a resumed run counts only its own. A tile whose
   padded window only partly leaves the raster is not flat: its grid has the
   ground where there is any. A large number means the coverage and the
   raster disagree.
3. **Spot-check tiles against the dev tile server, at more than one zoom.**
   The dev server bakes terrain on a miss with the same functions this tool
   runs (`terrain/leaf.rs` for a leaf, `pyramid.rs` for a tile above),
   so a tile it serves holds the packet this tool wrote. It packs at zstd's
   quick level where the bake packs at its stored one, so compare the packets,
   not the objects. With `deno task dev:tiles` up — not `dev`, which proxies
   every route to the live deployment:

   The dev server answers the bucket's addresses, so it takes the tile's name
   while the file on disk is still under its zoom — one tile, two names:

   ```bash
   curl -s localhost:3001/tile/terrain/<name> | zstd -dcq | shasum
   zstd -dcq ./out/terrain/<version>/<name> | shasum   # gunzip -c before zstd
   ```

   A coarse tile makes the dev server bake the missing tiles under it first, so the
   first ask coarser than z13 takes seconds to a minute. A mismatch means the two
   paths have drifted — investigate before publishing.

### Looking at a bake before it is published

A finished bake is on disk as the bucket holds it, so it is looked at in
the app with nothing uploaded by serving `out/` as it is: `deno task
dev:shelf` runs the tile server with `--shelf ./out`, which answers each
kind from its last bake, the version `out/<kind>/manifest.json` names
(`serve_tiles.rs` `last_bakes`), so nothing is published to look at it;
`preview:shelf` is the same under a production bundle.

## POI

POI is a second layer of terrain's pyramid: the same tiles, zooms and names,
holding, per cell (`pxl-spec bake/poi_tile.rs`, one XTP tree), a count of
places and, of each kind, the place of least id with the leaf cell it
stands in — its names, held to 32 code points, the source's own id and its
kind by name, from a pool of places held once a tile. A kind is the place's
category as the search list names it (`pxl-geo-spec overture/properties.rs`),
one of `spec/spec/bake/poi_badge.json`'s. The leaves are counted from `overture_places`, the zooms above are
folded by adding, with the same fold driver terrain uses
(`geo-bake/src/pyramid.rs`), and then every tile its parent's occupancy marks none is let go
(`prune`): where no cell over a tile holds two places, the tile over it says
all of its ground, so the tree stops there.

```bash
deno task bake:poi --out ./out/poi/<version> --concurrency 8
```

It is not resumable: a run clears the version directory and bakes it whole,
which takes the one pass over the table the leaves need. Verify that the
manifest's `zooms` reach z5 and that a leaf decodes (`poi_tile::decode`).
The client shows it only outside production, behind `poi` in the
development settings, until a nationwide version is published.

## Geocode

```bash
deno task bake:geocode --out ./out/geocode/<version>
```

It takes `--out`, `--land-cache <path>`, `--concurrency <n>`, `--limit <n>`
and `--force` (re-fetch existing tiles and `.empty` markers). It needs
terrain's `land_tiles.json`, not its tables.

Verify `failed=` is zero and that `_dict` and a group open
(`division_tile::open_division_dict`, `open_division_group`). A group's name
is a tile's first eleven digits. A tile's name is the division it is in and
each above it, by Overture's `parent_division_id`, which the ingest takes
whole, ancestors past the coverage included.

## Versions, and when the bake logic changes

`PUBLISHED_VERSION` is what the client requests. Setting one there to a
bake's folder name is the whole change on the client's side.

A published version is immutable, for every kind: a tile is never
rewritten under a name a cache or an open client already holds, and going
back is naming the version before. **Every bake is a new name**, whatever
made it — a new Overture release, or a change to how a tile is baked — and
each kind is named on its own, since each is baked again on its own. A
kind is baked again whole, never the tiles that changed: a coarse tile is
folded from the ones under it, so a partial bake would leave the zooms
disagreeing, and a mixed folder would serve tiles from two pipelines with no
way to tell them apart.

Only a change to **how the client reads a tile** — its format in
`pxl-spec bake/` — bumps the kind's `FORMAT`.

The local tile server shares one demand path across terrain, POI and geocode
(`geo-bake/src/demand/`). `--cache` is the parent directory, default `cache/tile`;
each kind owns a directory under the source it bakes from. Concurrent requests join the
same job, packets are written atomically, and responses carry zstd encoding.
All three kinds share the server's `--workers` execution slots (default 4)
and `--queue` additional admitted-job slots (default 64). Dependencies count
against admission too; terrain resolves them sequentially, without a task
fan-out. A new job over the limit gets 503; a cached result or an existing
job needs no new slot. `--job-seconds` (default 30) bounds each
job, including its dependency and execution-slot waits. One absolute deadline
is assigned on admission; starting the bake does not reset it. The job remains
joinable and keeps its admission slot until process termination and reaping
finish. Only then can a failed job be admitted again. Successful processes
publish through the common staging owner: one packet move or one directory
rename. Dictionary backing and its packet are published together. Staging
stays private until success. A deadline or admission refusal is retryable,
never an empty tile. These are execution/time budgets,
not a cap on process memory or total cached disk bytes.

`--folds` (default 2, so z13) bounds the pyramid layers, terrain and POI
alike, in zooms over the leaves: a tile coarser is answered 503, since each
one folds the whole zoom under it on first request, and a tile a client
asks for takes a worker before one a fold alone needs. Geocode groups have a fixed bake footprint.
The common job limits apply to all of them.
A tile is cached under the source it was baked from,
`cache/tile/<source>/<kind>/`, where `<source>` names the running binary and
the geo DB's catalogs (`demand.rs` `source_id`). A restart keeps the cache
while both are the same; after a re-ingest or a bake change, a restart starts
an empty one and removes the old.
