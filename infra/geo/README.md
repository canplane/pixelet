# Filling db-geo

Every tile is baked from one database, `db-geo`, and a bake reads nothing else.
So the database has to say exactly what it holds: which version of each source,
over which part of the world. `db-geo` is a local copy of three public sources,
Overture Maps, GEDTM30's elevation and OpenStreetMap's land polygons, in one
PostGIS instance. The bakes read it to make every tile, and forward search reads
it live. Three ingests fill it, run by hand when a source or the coverage
changes.

Two files decide what it holds. Each source's `source.json` pins its version
and the time it was taken, and the ingest writes both into the database, so the
database can say what it holds without these files. The coverage,
`coverage.json`, lists the shards: tiles at zoom 8, each named by its Morton
key. A shard holds everything its leaves' bakes read, out to its apron, so each
shard bakes from itself alone and shards taken one at a time give the same
result as shards taken together. The coast belongs to no shard; it is taken
whole.

Overture is the base. What it cannot supply sits beside it, named for the role
it fills.

## The tables

| Source            | Tables                                                                                                                                                         | Pin held in             |
| :---------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------- | :---------------------- |
| Overture, read    | `overture_buildings`, `overture_transportation`, `overture_base_land`, `overture_base_land_use`, `overture_base_water`, `overture_base_infrastructure`         | `overture_catalog`      |
| Overture, read    | `overture_places`, `overture_divisions`, `overture_division_areas`                                                                                             | `overture_catalog`      |
| Overture, unread  | `overture_addresses`, `overture_building_parts`, `overture_connectors`, `overture_division_boundaries`, `overture_base_land_cover`, `overture_base_bathymetry` | `overture_catalog`      |
| GEDTM30 v1.2      | `elevation_raster`                                                                                                                                             | `elevation_catalog`     |
| OSM land polygons | `osm_land_polygons`                                                                                                                                            | the table's own comment |

`overture_catalog` has one row for each table and shard, and `elevation_catalog`
one row for each shard. Each row holds the `version`, the `accessed_at` and a
count: of features for Overture, of blocks for the elevation. When a set is
baked, these rows and the coast's pin are copied into its `manifest.json`, which
records what the set was baked from. An Overture row is its GERS id, its
properties as the release holds them, and its geometry; a column a reader
filters on is generated from the properties.

## Running the ingests

```sh
docker compose -f infra/local/docker-compose.yml up -d db-geo

make ingest-overture ARGS="[--type <type>]… [--shard <name>]… [--whole]"
./infra/geo/elevation/ingest.sh
./infra/geo/coastline/ingest.sh
```

Each reads `infra/local/.env`; no source needs a key. Every tool they run, GDAL,
PostGIS's raster loader, psql and Overture's CLI, comes from the image in
`tool/`, which `tool/run` builds before each call, so the host needs only
Docker, cargo and `jq`. The image's PostgreSQL 18 is db-geo's own major, and
its PostGIS 3.6 is one release behind db-geo's 3.7, since no Ubuntu release
ships 3.7. Never `down -v`: the volume is the only copy, and filling it again
takes hours.

**Overture.** A run takes the types it names, or all of them, and the shards it
names, or the coverage's. It writes each type into its table in place: a
feature already held takes the release's version, and a feature the release no
longer has is removed from the shard its lower-left corner lies in. With
`--whole`, each table is built anew from every shard and swapped in. Each type
is written in one transaction together with its catalog rows, so a type stopped
partway leaves nothing of itself behind. The `overturemaps` CLI, pinned in the
tools' image, downloads each type's GeoParquet to `overture/raw/`, where it is
kept until the release or the shards change. Divisions come with their
ancestors, which may lie outside every shard.

**Elevation.** Each shard is warped out of the remote GeoTIFF over HTTP onto one
grid, at the stride the bake samples, and cut into blocks. A block is a square
of samples as wide as a tile at one zoom and is named by that tile, so a block
that two aprons share is stored once. A run fills the tables beside the old ones
and swaps them in once every shard is whole. A run that stopped partway resumes
from the shards already beside, unless those hold another version, which it
refuses rather than mix.

**Coastline.** OSM's land polygons for the whole world come as one zip. osmdata
names no version and serves only its latest, so the zip's SHA-256 is its
version. The zip kept in `coastline/raw/` is used while it
matches the pin, and once osmdata moves on it is the pin's only copy: deleting it
makes the pinned coast unreachable. A newer coast is taken by changing the pin.

## Checking what it holds

```sql
SELECT table_name, count(*) AS shards, min(version), max(version), sum(total_count)
FROM overture_catalog GROUP BY table_name ORDER BY table_name;

SELECT count(*) AS shards, min(version), max(version) FROM elevation_catalog;

SELECT obj_description('osm_land_polygons'::regclass);
```

Every table should name each of the coverage's shards at the pinned version. A
bake checks the same before it reads anything: it refuses an Overture table that
lacks a shard it needs or holds one outside the coverage, and an elevation that
holds anything other than exactly the coverage's shards (`pxl-geo-bake
coverage.rs`, `elevation.rs`). Replacing rows leaves dead space behind, which
`VACUUM ANALYZE` reclaims.

Production keeps its own copy of the three tables search reads. A re-ingest
reaches it only when they are copied over ([infra/README.md](../README.md) §3).
