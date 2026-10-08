---
name: bake-tiles
description: Bake terrain, place and division tiles from the geo DB into each kind's set, out/<kind>/, and verify it before it is uploaded. Use when a bake pipeline or its source data changed, or a tile set needs (re)building. Uploading and deploying is the publish-bucket skill.
---

# Baking tiles

The deployed API answers forward search only. Terrain, place and
division tiles are static objects in the bucket, produced offline
by the tools in `crates/server/geo-bake` from the local geo database. This is
how each set gets built and checked; getting it into the bucket is
[publish-bucket](../publish-bucket/SKILL.md).

Background and the reasoning behind the design are in
[decision/20260816-deployment-hosting.md](../../../decision/20260816-deployment-hosting.md).
Read it before changing a pipeline's shape; this file is the procedure only.

## The three bakes

| Kind     | Tool                      | Reads                                           | Output                                      | Empty tile |
| -------- | ------------------------- | ----------------------------------------------- | ------------------------------------------- | ---------- |
| terrain  | `deno task bake:terrain`  | Overture buildings/transportation/base, GEDTM30 | packed XTP, one per land tile               | no object  |
| place    | `deno task bake:place`    | Overture places                                 | XTP counts, where a crowd splits            | no object  |
| division | `deno task bake:division` | Overture divisions                              | XTP: per cell the division it is in, z12 up | no object  |

Every tool writes its kind's set, `./out/<kind>/`, with a manifest of the
Overture release its tables were ingested from and the commit it was built
at. A version is the set uploaded under a name, `v<format>.<day>.<n>`: the
kind's `FORMAT` in `spec/spec/bake/published.json`, the day it was
published (UTC) and that day's count, from 1 — `v7.2026-10-04.1`.

A tile is named by its 2D Morton key written a digit a zoom, and every bake
writes that name straight out, flat in its set: the filename _is_ the
object key, and a name's length is its zoom (publish-bucket).

Every object is packed in the encoding `util.rs`'s `CONTENT_ENCODING` names —
zstd — and each manifest says so in `content_encoding`, which is what the
upload labels the objects with.

Every object is written to `.part` and renamed, so an interrupted run
leaves no truncated file; a shard is made beside its old one and renamed in
whole (see Shards).

## Before starting

The bakes read the geo database directly, so it must be up and populated:

```bash
docker compose -f infra/local/docker-compose.yml --env-file .env up -d
```

Terrain needs `overture_buildings`, `overture_transportation`,
`overture_base_{land,land_use,water,infrastructure}` and `elevation_raster`;
Places needs `overture_places`; divisions needs `overture_division_areas` and
`overture_divisions`. If a table is missing, that is an ingestion
job, not a bake job — [infra/geo/README.md](../../../infra/geo/README.md).
Every bake reaches the DB only through `geo-bake coverage.rs` `Overture`,
which takes the `overture_*` tables from the statements the bake runs and
refuses unless each holds exactly the coverage's shards in `overture_catalog`;
the elevation must hold exactly them. The error names the ingest to run.
Every shard under one build root rests on one release and one code: a change
of either is baked again from nothing, so nothing records it per shard.
Never `down -v` to "reset" the database; the volume is several gigabytes of
ingested data.

## Shards

Each kind has one set, `out/<kind>/`: every zoom of the pyramid, flat
under the tiles' names, never pruned, with its `manifest.json`. A bake
changes it only through `out/<kind>.stage/`, a shard at a time, as the
ingest works, on the shards it is told (`tile/shard.rs`):

- `leaf` stages the named shards' leaves (`--shard <name>`, repeated; every
  shard of the coverage when none is named), each shard then marked whole by
  `<shard>.done`, which lists the set's leaves under it the bake no longer
  makes;
- `fold` folds again only what the whole shards change, reading the set as
  it will be, and stages the parents that changed (`pyramid.rs` `fold`): a
  shard's own tiles, the band along a neighbour's edge that elevation's
  padding reads, and the ancestors, up to a zoom where nothing changed;
- `commit`, only once folded, moves the staged tiles into the set and takes
  away what each `.done` lists; the manifest then says the source once
  (`coverage.rs` `holdings`);
- `publish`, only when named, makes `out/<kind>.publish/` for an upload.

Every stage can stop and run again. A shard not baked yet reads as sea. A
change of source, code or coverage is a purge: `deno task clear:shelf
<kind>`, then bake every shard. Which shards to bake, and when, is the operator's to say;
nothing tracks it.

| Flag                | Meaning                                                                       |
| ------------------- | ----------------------------------------------------------------------------- |
| `--stage <s>`       | `leaf`, `fold`, `commit`, `publish`; repeatable; the first three when omitted |
| `--shard <name>`    | bake that shard's leaves, repeatable; every shard when omitted                |
| `--out <dir>`       | the set (default `./out/<kind>`), staged in `<dir>.stage`                     |
| `--concurrency <n>` | parallel tiles — match `DB_POOL_SIZE`, the DB is the bottleneck               |

It runs long enough to want a log file rather than a terminal:

```bash
... > /tmp/bake.log 2>&1 &
grep '^\[bake' /tmp/bake.log | tail -2
```

## Terrain

A leaf is each tile exactly as the dev server bakes it on demand
(`geo-bake terrain/leaf.rs` `bake_leaf`): its separation solve (`separation.rs`) — every
structure reaching the tile or the eight around it, solved whole wherever it
runs — and from that one result its lines (bridges, tunnels and the approaches at
their ends) and its surface (ground and elevation), which paints a way only
where no line draws it.

```bash
deno task bake:terrain --shard 01033210 --stage leaf
deno task bake:terrain --stage assemble
```

A shard's leaves are its tiles anything is drawn in — land, or a way,
building, bridge or tunnel (`terrain/leaf.rs` `has_ground`) — found again each
time the shard is baked. Open-ocean tiles are deliberately not baked:
roughly two thirds of the coverage is open sea, and a tile with no object is
drawn from the one over it. The fold reads a tile it has no object for as
open sea inside the coverage and, past it, as unknown, a cell with no
surface, sea or land, so the planet's picture is seen under it; and a
coarser one as the fold of those under it (`terrain/absent.rs`).

**A tile is baked from more than its own bbox, and cropped to it.** What is
drawn in a tile can come from past its edge, so every read reaches as far as
that, worked back from how far each thing draws:

- **Rows** (ways, buildings, bridges, tunnels, water) are read from the tile's
  bbox grown by `DRAWN_REACH_U` (terrain/repository.rs `drawn_from`): a way
  drawn its widest, `WIDEST` cells (the width a line's byte holds, and the
  ground draws no wider), plus the corridor gap one way paves to another.
  The tile keeps only its own cells, and its lines' runs within their
  footprint (pxl-spec bake/separation.rs `footprint`).
- **Elevation** is sampled `ELE_PAD` samples past the tile, what a bicubic
  window at the tile's edge reads. The published shards
  (`infra/geo/coverage.json`) are warped each to its own edges, so
  a padded grid at a shard's edge reads its neighbor's raster, and at the
  coverage's outer edge finds none.

### Verifying

1. **Zero failures.** Each stage's line reports `failed=`. Anything above
   zero in `surface` means tiles are missing; repeat the stage (it skips the
   successes). `commit` refuses to run before `fold`.
2. **Spot-check tiles against the dev tile server, at more than one zoom.**
   The dev server bakes terrain on a miss with the same functions this tool
   runs (`terrain/leaf.rs` for a leaf, `pyramid.rs` for a tile above),
   so a tile it serves holds the packet this tool wrote. It packs at zstd's
   quick level where the bake packs at its stored one, so compare the packets,
   not the objects. With `deno task dev:tile` up — not `dev`, which proxies
   every route to the live deployment:

   The dev server answers the bucket's addresses, so it takes the tile's name
   while the file on disk is still under its zoom — one tile, two names:

   ```bash
   curl -s localhost:3001/tile/terrain/<name> | zstd -dcq | shasum
   zstd -dcq ./out/terrain/<name> | shasum
   ```

   A coarse tile makes the dev server bake the missing tiles under it first, so the
   first ask coarser than z13 takes seconds to a minute. A mismatch means the two
   paths have drifted — investigate before publishing.

### Looking at a bake before it is published

Each kind's set is looked at in the app with nothing uploaded: `deno task
dev:shelf` runs the tile server with `--shelf ./out`, which answers each
kind from `out/<kind>/` and any version name from it; `preview:shelf` is
the same under a production bundle.

## Places

Places is a second layer of terrain's pyramid: the same tiles, zooms and names,
holding, per cell (`pxl-spec bake/place_tile.rs`, one XTP tree), a count of
places and, of each kind, the place of least id with the leaf cell it
stands in — its names, held to 32 code points, the source's own id and its
kind by name, from a pool of places held once a tile. A kind is the place's
category as the search list names it (`pxl-geo-spec overture/properties.rs`),
one of `spec/spec/bake/place_badge.json`'s. The leaves are counted from `overture_places`, the zooms above are
folded by adding, with the same fold driver terrain uses
(`geo-bake/src/pyramid.rs`); a version leaves out every tile its parent's
occupancy marks none (`unasked`): where no cell over a tile holds two places,
the tile over it says all of its ground, so the tree stops there.

```bash
deno task bake:place
```

A shard's leaves are its places, read by the shard's bbox. Verify that the
manifest's `zooms` reach z5 and that a leaf decodes (`place_tile::decode`).
The client shows it only outside production, behind `place` in the
development settings, until a nationwide version is published.

## Divisions

```bash
deno task bake:division
```

Its leaves are z12 (`division_tile.rs` `DIVISION_LEAF`), a shard's every
z12 tile with a division in it (`division.rs` `leaf`); each coarser zoom is
folded, and a version leaves out a tile that says no more than its parent.

Verify `failed=` is zero and that a tile decodes (`division_tile::decode`). A
cell's name is the division it is in and each above it, by Overture's
`parent_division_id`, which the ingest takes whole, ancestors past the
coverage included.

## Versions, and when the bake logic changes

`PUBLISHED_VERSION` is what the client requests. Setting one there to a
version's name is the whole change on the client's side.

A published version is immutable, for every kind: a tile is never
rewritten under a name a cache or an open client already holds, and going
back is naming the version before. **Every bake is a new name**, whatever
made it — a new Overture release, or a change to how a tile is baked — and
each kind is named on its own, since each is baked again on its own. A
version is the set as it stands, every zoom folded from the leaves under it.

Only a change to **how the client reads a tile** — its format in
`pxl-spec bake/` — bumps the kind's `FORMAT`.

The local tile server shares one demand path across the terrain, place and division layers
(`geo-bake/src/demand/`). `--cache` is the parent directory, default `cache/tile`;
each kind owns a directory under it. Concurrent requests join the
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
rename. Staging
stays private until success. A deadline or admission refusal is retryable,
never an empty tile. These are execution/time budgets,
not a cap on process memory or total cached disk bytes.

`--folds` (default 2, so z13) bounds the pyramid layers, terrain, place and
division alike, in zooms over the leaves: a tile coarser is answered 503, since each
one folds the whole zoom under it on first request, and a tile a client
asks for takes a worker before one a fold alone needs. Divisions' leaves are z12, so its top is as
many zooms over them (`tile.rs` `top_over`).
The common job limits apply to all of them.
A tile is cached under `cache/tile/<kind>/` and kept across restarts. A
re-ingest or a bake change is a purge by command: `deno task clear:oven`.
