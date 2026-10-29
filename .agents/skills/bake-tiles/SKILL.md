---
name: bake-tiles
description: Bake terrain, place and division tiles from the geo DB into each kind's set, out/<kind>/, and verify it before it is uploaded. Use when a bake pipeline or its source data changed, or a tile set needs (re)building. Uploading is the publish-bucket skill; deploying, the deploy skill.
---

# Baking tiles

How to build a tile set and check it. What a tile holds and how the bake folds
it is in ARCHITECTURE.md, "Making the data"; uploading a set is
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

The first four write a tile set to `out/<kind>/`; star and geoid are not tile
sets and are not published. Each binary these run is listed in the Makefile's
`TOOLS` and has a cargo alias of the same name in `rust/`.

## Before starting

The bakes read the geo database directly:

```bash
docker compose -f infra/local/docker-compose.yml up -d
```

A missing or partial table needs an ingest; the bake names it
([infra/geo/README.md](../../../infra/geo/README.md)). **Never `down -v`.**

## Stages and flags

A bake changes its set only through `out/<kind>.stage/`, one shard at a time:
`leaf` (the named shards' leaves), then `fold`, then `commit`. `publish` runs
only when named and makes `out/<kind>.publish/` for an upload. Any stage can be
stopped and rerun, and `commit` refuses to run before `fold`. After a change of
source, code or coverage, purge
(`rm -rf out/<kind> out/<kind>.stage out/<kind>.publish`) and bake every shard.

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

1. **Zero failures** in the leaf stage's `failed=`.
2. **Terrain: spot-check against the dev tile server, at more than one zoom.**
   Compare the decompressed packets, not the files. Run `make serve-tile`, not
   `dev`:

   ```bash
   v=$(curl -s localhost:3001/tile/source | jq -r .version.terrain)
   curl -s localhost:3001/tile/terrain/$v/<name> | zstd -dcq | shasum
   zstd -dcq ./out/terrain/<name> | shasum
   ```

   A tile coarser than z13 takes up to a minute on its first request. On a
   mismatch, investigate before publishing.
3. **Places:** the manifest's `zooms` reach the coarsest the client reads
   (pxl-spec `lattice.rs` `coarsest`), and a leaf decodes
   (`pxl-spec bake/tile/place.rs` `decode`). Places draw only with the `place`
   setting on (off by default).
4. **Divisions:** `failed=` is zero and a tile decodes
   (`pxl-spec bake/tile/division.rs` `decode`).

## Versions

**Every bake gets a new version name.** Bump the kind's `FORMAT` (`spec/spec/bake/published.json`) only when how the client
reads a tile changes (its format in `pxl-spec bake/`).

After a re-ingest or a bake change, `rm -rf out/oven` (the dev tile server's
cache).
