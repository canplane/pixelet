# Filling db-geo

`db-geo` is a local copy of three public sources — Overture Maps, GEDTM30's
elevation and OpenStreetMap's land polygons — in one PostGIS instance. The
bakes read it to make every tile; forward search reads it live. Three ingests
fill it, run by hand when a source or the coverage changes.

What it holds is decided by two things a file pins:

- **Which version.** Each source's `source.json` pins its version and when
  it was taken (`pin`), and the run writes both into the database, so the
  database says what it holds without these files.
- **Where.** The coverage is a list of **shards**, each a z8 tile
  (`SHARD_ZOOM`) named by its Morton key, kept in
  `coverage.json` and read by `pxl-geo-bake coverage.rs`. A shard
  holds everything its leaves' bakes read, out to its apron
  (`SHARD_APRON`), so it bakes from itself alone and shards taken one at a
  time are the same as taken together. The coast is no shard's: it is taken
  whole.

A table is filled beside itself and swapped in, never emptied first
([infra/README.md](../README.md) §2).

Overture is the base; what it cannot supply sits beside it, named for the
role it fills.

```
infra/geo/
├── coverage.json          the shards
├── tools/
│   ├── Dockerfile         every tool the ingests run, pinned
│   └── run                a tool, run in that image
├── overture/
│   └── source.json        the release; ingested by pxl-geo-bake's ingest_overture, every type a table
├── elevation/
│   ├── source.json        the version
│   └── ingest.sh
└── coastline/
    ├── source.json        the zip's hash
    └── ingest.sh
```

## The tables

| Source            | Tables                                                                                                                                                         | Pin held in             |
| :---------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------- | :---------------------- |
| Overture, read    | `overture_buildings`, `overture_transportation`, `overture_base_land`, `overture_base_land_use`, `overture_base_water`, `overture_base_infrastructure`         | `overture_catalog`      |
| Overture, read    | `overture_places`, `overture_divisions`, `overture_division_areas`                                                                                             | `overture_catalog`      |
| Overture, unread  | `overture_addresses`, `overture_building_parts`, `overture_connectors`, `overture_division_boundaries`, `overture_base_land_cover`, `overture_base_bathymetry` | `overture_catalog`      |
| GEDTM30 v1.2      | `elevation_raster`                                                                                                                                             | `elevation_catalog`     |
| OSM land polygons | `osm_land_polygons`                                                                                                                                            | the table's own comment |

`overture_catalog` has a row per table and shard, `elevation_catalog` a row
per shard, each with the `version`, the `accessed_at` and a count — of
features for Overture, of blocks for the elevation. `holdings` (`pxl-geo-bake
coverage.rs`) copies them into a baked set's `manifest.json`. An Overture row
is its GERS id, its properties as the release holds them, and its geometry; a
column a reader filters on is generated from `properties`.

## Run

```sh
docker compose -f infra/local/docker-compose.yml up -d db-geo

make ingest-overture ARGS="[--type <type>]… [--shard <name>]… [--whole]"
./infra/geo/elevation/ingest.sh
./infra/geo/coastline/ingest.sh
```

Each reads `infra/local/.env`, the scripts themselves and the Overture
ingest through `script/with-env.sh`; no source needs a key. Every tool they
run — GDAL, PostGIS's raster loader, psql, Overture's CLI — is `tools/`'s
image's, which `tools/run` builds from its Dockerfile before each call, so
the host needs Docker and cargo alone. Ubuntu 24.04's are PostgreSQL 16 and
PostGIS 3.4, db-geo's own majors. Never `down -v`: the volume is the only
copy, and filling it again takes hours.

**Overture.** A run takes the types it names (every one by default) and the
shards it names (the coverage's by default), and adds them to each table as it
is, leaving a feature held already; `--whole` makes each table anew from every
shard and swaps it in. Each type's write is one transaction, its catalog rows
with it, so a type stopped partway leaves nothing of itself. The official
`overturemaps` CLI, pinned in the tools' image, downloads each type's
GeoParquet to `overture/raw/`, kept until the release or the shards change.
Divisions come with their ancestors, which may lie past every shard.

**Elevation.** Each shard is warped out of the remote GeoTIFF over HTTP onto
one grid, at the stride the bake samples, into blocks each named by the
tile it is (`BLOCK_ZOOM`, `pxl-geo-bake elevation.rs`), so a block two
aprons share is stored once. A run fills the tables beside and swaps them in
once every shard is whole; a run stopped partway resumes from the shards
beside, unless they hold another version, which it refuses rather than mix,
and any other run warps every shard anew.

**Coastline.** OSM's land polygons for the world, one zip. osmdata names no
version and serves only its latest, so the zip's SHA-256 (its first 16 hex)
is its version: the zip kept in `coastline/raw/` is used while it matches,
and is the pin's only copy once osmdata moves on — deleting it makes the pin
unreachable. A newer coast is taken by changing the pin.

## Check

```sql
SELECT table_name, count(*) AS shards, min(version), max(version), sum(total_count)
FROM overture_catalog GROUP BY table_name ORDER BY table_name;

SELECT count(*) AS shards, min(version), max(version) FROM elevation_catalog;

SELECT obj_description('osm_land_polygons'::regclass);
```

Every table should name each of the coverage's shards at the pinned version. A
bake checks the same before it reads: it refuses an Overture table that lacks
a shard it needs or holds one past the coverage, and an elevation that holds
other than exactly the coverage's shards (`pxl-geo-bake coverage.rs`
`Overture::reading`, `elevation.rs` `Raster::open`). Replacing rows leaves
dead space behind; `VACUUM ANALYZE` reclaims it.

Production keeps its own copy of the three tables search reads, which a
re-ingest reaches only when it is copied over
([infra/README.md](../README.md) §3).
