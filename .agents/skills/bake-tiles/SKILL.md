---
name: bake-tiles
description: Bake terrain, place and division tiles from the geo DB into each kind's set, out/<kind>/, and verify it before it is uploaded. Use when a bake pipeline or its source data changed, or a tile set needs (re)building. Uploading is the publish-bucket skill; deploying, the deploy skill.
---

# Baking tiles

How a set is built and checked. What a tile holds and how the bake folds it is
ARCHITECTURE.md's "Making the data"; getting a set into the bucket is
[publish-bucket](../publish-bucket/SKILL.md).

## The bakes

```bash
make bake-terrain    # Overture buildings/transportation/base, GEDTM30
make bake-place      # Overture places
make bake-division   # Overture divisions, leaves at z12
make download-earth && make bake-earth   # the Earth's picture, out/earth/
make bake-star       # the Yale catalog's stars (make download-star first)
make bake-geoid ARGS=egm2008-5.pgm   # EGM2008, into spec/spec/space/geoid.json
```

The first four write a tile set, `out/<kind>/`; star and geoid are not tile
sets and are not published. The binaries make runs are the Makefile's `TOOLS`, each
rust/'s cargo alias of the same name.

## Before starting

The bakes read the geo database directly:

```bash
docker compose -f infra/local/docker-compose.yml up -d
```

A missing or partial table is an ingest's job, not a bake's: the bake refuses
and names the ingest to run ([infra/geo/README.md](../../../infra/geo/README.md)).
**Never `down -v` to "reset" the database**: the volume is several gigabytes of
ingested data.

## Stages and flags

A bake changes its set only through `out/<kind>.stage/`, a shard at a time:
`leaf` (the named shards' leaves), then `fold`, then `commit`; `publish`, only
when named, makes `out/<kind>.publish/` for an upload. Every stage can stop and
run again, and `commit` refuses to run before `fold`. A change of source, code
or coverage is a purge: `rm -rf out/<kind> out/<kind>.stage out/<kind>.publish`, then bake every shard.

```bash
make bake-terrain ARGS="--shard 01033210 --stage leaf"
make bake-terrain ARGS="--stage fold --stage commit"
```

| Flag                | Meaning                                                                       |
| ------------------- | ----------------------------------------------------------------------------- |
| `--stage <s>`       | `leaf`, `fold`, `commit`, `publish`; repeatable; the first three when omitted |
| `--shard <name>`    | bake that shard's leaves, repeatable; every shard when omitted                |
| `--out <dir>`       | the set (default `./out/<kind>`), staged in `<dir>.stage`                     |
| `--concurrency <n>` | parallel tiles — match `DB_POOL_SIZE`, the DB is the bottleneck               |

## Verifying

1. **Zero failures.** The leaf stage's line reports `failed=`; above zero,
   tiles are missing, so repeat the stage.
2. **Terrain: spot-check against the dev tile server, at more than one zoom.**
   It bakes a missed tile with the same functions, but packs at zstd's quick
   level, so compare the decompressed packets, not the objects. Run
   `make serve-tile` — not `dev`, which proxies every route to the live
   deployment:

   ```bash
   v=$(curl -s localhost:3001/tile/source | jq -r .version.terrain)
   curl -s localhost:3001/tile/terrain/$v/<name> | zstd -dcq | shasum
   zstd -dcq ./out/terrain/<name> | shasum
   ```

   A tile coarser than z13 makes the server bake what is under it first, so
   the first ask takes seconds to a minute. A mismatch means the two paths
   have drifted: investigate before publishing.
3. **Places:** the manifest's `zooms` reach the coarsest the client reads (pxl-spec `lattice.rs` `coarsest`), and a leaf decodes
   (`pxl-spec bake/tile/place.rs` `decode`). The client draws places only with
   the `place` setting on, which is off by default, so a world without them is
   not a failed bake.
4. **Divisions:** `failed=` is zero and a tile decodes
   (`pxl-spec bake/tile/division.rs` `decode`).

## Versions

**Every bake is a new version name**, whatever made it, and a published version
is never rewritten: going back is `./script/publish.py serve <kind> <version>`.
Only a change to how the client reads a tile — its format in `pxl-spec bake/` —
bumps the kind's `FORMAT` (`spec/spec/bake/published.json`).

The dev tile server keeps what it baked on demand under `out/oven/` across
restarts. After a re-ingest or a bake change, run `rm -rf out/oven`, or it
serves the old tiles.
