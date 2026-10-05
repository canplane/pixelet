---
name: bake-tiles
description: Bake terrain, POI and reverse-geocode tiles from the geo DB into out/<kind>/<version> and verify the set before it is uploaded. Use when a bake pipeline or its source data changed, or a tile set needs (re)building. Uploading and deploying is the publish-bucket skill.
---

# Baking tiles

The deployed API answers forward search only. Terrain, POI and reverse
geocode are static objects in the bucket, produced offline
by the tools in `crates/pxl/server/geo-bake` from the local geo database. This is
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
`spec/pxl/spec/bake/published.json`, the day the bake ran (UTC) and that
day's count of bakes of the kind, from 1 — `v7.2026-10-04.1`
(`pxl-spec bake/published.spec.rs` `version_name`). What it was baked from
is the manifest's: the Overture release its tables were ingested from and the
commit it was built at. The last path segment of `--out` overrides it.

The bucket names a tile by its **quadkey** and nothing else (publish-bucket).
Geocode writes that name straight out, so for it the filename _is_ the object
key. Terrain and POI, the two pyramids, do not: they write
`z<zoom>/<tile index>`, because the fold clears a zoom's directory to redo it
and drops the zooms a run did not reach. Their names are translated at
publish, by hard links, which is the one step geocode skips.

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
POI needs `overture_places`; geocode needs `overture_divisions` and
`overture_division_points`. If a table is missing, that is an ingestion
job, not a bake job — [infra/geo/README.md](../../../infra/geo/README.md).
Every bake reaches the DB only through `geo-bake coverage.rs` `Overture`,
which takes the `overture_*` tables from the statements the bake runs and
refuses unless each has an `overture_catalog` row of one release over
exactly the bbox `cargo run -q --manifest-path crates/Cargo.toml --bin
ingest_bbox` prints; the error names the `--bbox` to ingest with.
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
| `--region <qk>`     | bake again only the land tiles under a quadkey, repeatable         |
| `--base <dir>`      | the version a region is baked over (default the published one)     |

It runs long enough to want a log file rather than a terminal:

```bash
... > /tmp/bake.log 2>&1 &
grep '^\[bake\] ' /tmp/bake.log | tail -2
```

A fix that reaches one area need not bake every tile again. `--region`
bakes the land tiles under a quadkey prefix again, by the code as it is now,
and assembles a new version from `--base`: the base's tiles linked whole, the
region's leaves in place of the ones that differ, and folded again only the
tiles above whose `Folds::sources` read a tile that changed, a zoom at a time,
until a zoom changes nothing. Minutes, where a full bake is hours:

```bash
deno task bake:tiles --region 132110320312 --base ./out/terrain/<version>
```

Every tile it writes is what a full bake of the same code writes; the tiles
outside the region keep the base's. So **the region must hold everything the
change reaches**: a structure solved again that runs out of the region meets
the base's solve of it at the edge. Choosing the region is the caller's job.
The manifest records the base and the region's release, which may differ.

The new version is complete on disk, and also a **partial snapshot**: its
`_snapshot` lists the full version it overlays and the tiles it writes or
removes over it, so the bucket holds only those
([20261004-a-snapshot-of-a-subtree.md](../../../decision/20261004-a-snapshot-of-a-subtree.md)).
A region baked over a partial version lists its changes over that one's
full base, earlier partial's included.

What it produces, beside `leaf/` under `--build` with a `leaf.json` of what
built it:

- `z15/`, one packed object per tile anything is drawn in — land, or a way,
  building, deck or bore — the leaves
- `z14/`, `z13/`, … each zoom folded from the one below it until a zoom is a
  single tile or `z5`, refolded whole on every run (they are a function of the leaves,
  so a resumed run never leaves a stale pyramid over them)
- `manifest.json` and `land_tiles.json` in the directory above the version's

Open-ocean tiles are deliberately not baked. Roughly two thirds of the
coverage bbox is open sea, and a tile with no object is drawn from the one
over it. The fold reads a tile it has no object for as open sea inside the
coverage and as void past it, a tile with no surface, and a coarser one as the
fold of those under it (`terrain/absent.rs`). A tile counts as open ocean only when nothing is drawn in it:
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
  window at the tile's edge reads. Past the raster a sample is no data, so
  the published coverage is cropped to tiles whose padded grid lies inside
  the raster (coverage.rs `Coverage::of_source`, from
  `infra/geo/elevation/metadata.json`); the source must reach
  that far past the area to be published, never end at it.

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

   The dev server answers the bucket's addresses, so it takes the quadkey
   while the file on disk is still under its zoom — one tile, two names:

   ```bash
   curl -s localhost:3001/tile/terrain/<quadkey> | zstd -dcq | shasum
   zstd -dcq ./out/terrain/<version>/z<zoom>/<tile_idx> | shasum   # gunzip -c before zstd
   ```

   A coarse tile makes the dev server bake the missing tiles under it first, so the
   first ask coarser than z13 takes seconds to a minute. A mismatch means the two
   paths have drifted — investigate before publishing.

### Looking at a bake before it is published

The dev tile server serves a tile from its cache when the tile's file is
there and bakes it only when it is not, and its cache is laid out as a
baked set is: `<cache>/terrain/z<zoom>/<tile_idx>`, zstd as the
bake writes them (`demand.rs`, `--cache`, `./out/dev` by default). So a
finished bake is looked at in the app, with nothing uploaded, by putting it
where the cache is:

```bash
# with the dev tile server stopped
mv out/dev/terrain out/dev/terrain.on-demand   # or remove it
cp -al out/terrain/<version> out/dev/terrain
deno task dev:local
```

`cp -al` links the files rather than copying them, so it takes no space,
and the cache's purge (`POST …/world/terrain/cache/invalidate`) unlinks the
links and leaves the bake under `out/terrain/<version>` as it was. A tile
the bake has no object for — open water past the coverage, a zoom coarser
than the pyramid's top — is still baked on demand into the cache beside the
linked ones. After a purge the server bakes everything on demand again; link
the set back in to see the bake.

## POI

POI is a second layer of terrain's pyramid: the same tiles, zooms and names,
holding, per cell (`pxl-spec bake/poi_tile.rs`, one XTP tree), a count of
places and, of each kind, the place of least id with the leaf cell it
stands in — its names, held to 32 code points, the source's own id and its
kind by name, from a pool of places held once a tile. A kind is the place's
category as the search list names it (`pxl-geo-spec overture/properties.rs`),
one of `spec/pxl/spec/bake/poi_badge.json`'s. The leaves are counted from `overture_places`, the zooms above are
folded by adding, with the same fold driver terrain uses
(`geo-bake/src/pyramid.rs`), and then every tile its parent settles is let go
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
(`place_tile::open_place_dict`, `open_place_group`). A group's name is a
tile's first eleven digits.

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
(`geo-bake/src/demand/`). `--cache` is the parent directory, default `out/dev`;
each kind owns a private generation beneath it. Concurrent requests join the
same job, packets are written atomically, and responses carry zstd encoding.
All three kinds share the server's `--workers` execution slots (default 4)
and `--queue` additional admitted-job slots (default 64). Dependencies count
against admission too; terrain resolves them sequentially, without a task
fan-out. A new job over the limit gets 503; a cached result or an existing
job needs no new slot. `--job-seconds` (default 30) bounds each generation
job, including its dependency and execution-slot waits. One absolute deadline
is assigned on admission; starting the bake does not reset it. The job remains
joinable and keeps its admission slot until process termination and reaping
finish. Only then can a failed job be admitted again. Successful processes
publish through the common staging owner: one packet move or one directory
rename. Dictionary backing and its packet are published together. Staging
stays private until success. A deadline or admission refusal is retryable,
never an empty tile. These are execution/time budgets,
not a cap on process memory or total cached disk bytes.

`--top-z` (default 12) bounds the pyramid layers, terrain and POI alike: a
tile coarser than it is answered 503, since each one folds the whole zoom
under it on first request. Geocode groups have a fixed bake footprint.
The common job limits apply to all of them and survive cache invalidation.
A tile is cached while its file is in `out/dev/<kind>/`, and a restart keeps
it. After re-ingest or a bake change, invalidate the affected kind as a whole
— a restart alone serves the old tiles:

```bash
curl -X POST localhost:3001/geo/internal/world/terrain/cache/invalidate -H "x-admin-token: $ADMIN_TOKEN"
# Likewise .../world/poi/... and .../world/geocode/...; send no body.
# The header is checked only when ADMIN_TOKEN is set.
```

Invalidation swaps the generation. Old requests can finish against their old
directory but cannot publish into the new one; the old directory is removed
when its last job finishes. Geocode's dictionary and groups share a generation;
a pyramid layer's tiles share one.
