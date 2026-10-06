# Filling db-geo

`db-geo` is the spatial database: sixteen tables from two sources, one PostGIS
instance. Everything the client sees of the world — terrain, POI, reverse
geocode — is baked from these tables offline and published to a bucket; only
forward search reads them at request time.

This directory holds what fills them. Nothing here is deployed, and nothing
here runs on a schedule. These are run by hand, a few times a year, against a
database that is expensive to rebuild.

> **The database is filled a shard at a time, and a shard is whole or absent.**
> A shard is a z8 tile (`SHARD_ZOOM`), named by its Morton key, its edges in
> EPSG:3857 meters rounded half to even. A feature is a row in the one shard
> holding its bbox's lower corner, the bbox its vertices' extremes in meters,
> rounded the same way; a shard is half open, so a corner on an edge is the
> next shard's.
> What a shard holds depends on the shard alone, so shards ingested one by one
> are the shards ingested together. Each ingest replaces a shard's rows before
> it writes them, and writes the shard's catalog row last; a run stopped
> partway resumes at the first shard without one.

```
infra/geo/
├── overture/                     fifteen types in six themes, one provider, one catalog
│   └── script/ingest.rs            the one ingest, schema included
└── elevation/                    one global GeoTIFF, a raster a shard
    ├── init/01_elevation_schema.sql
    ├── metadata.json               the published shards and the source URL
    ├── script/setup-elevation.sh
    └── warped/                     a shard's GeoTIFF, gitignored
```

**Which shards.** `metadata.json` names the shards the bakes publish, and
the ingests take exactly those (`pxl-geo-bake coverage.rs`). A bake refuses
to run unless every table it reads holds exactly them, all of one release,
and the elevation exactly them.

## 1. The tables

| Source                                                                           | Tables                                                                                                                                                         | Catalog             | Fills them                            |
| :------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------ | :------------------------------------ |
| **Overture — Buildings, Transportation, Base**<br>terrain features and coastline | `overture_buildings`, `overture_transportation`, `overture_base_land`, `overture_base_land_use`, `overture_base_water`, `overture_base_infrastructure`         | `overture_catalog`  | `overture/script/ingest.rs`           |
| **Overture — Places, Divisions**<br>POI, reverse geocode, forward search         | `overture_places`, `overture_divisions`, `overture_division_points`                                                                                            | `overture_catalog`  | `overture/script/ingest.rs`           |
| **Overture — the other types**<br>taken whole, read by nothing yet               | `overture_addresses`, `overture_building_parts`, `overture_connectors`, `overture_division_boundaries`, `overture_base_land_cover`, `overture_base_bathymetry` | `overture_catalog`  | `overture/script/ingest.rs`           |
| **GEDTM30**<br>ground elevation                                                  | `elevation_raster`                                                                                                                                             | `elevation_catalog` | `elevation/script/setup-elevation.sh` |

Coastline is not a table of its own. `OVERTURE_COAST_SQL`
(`crates/pxl/server/geo-bake/src/terrain/repository.rs`) carves it at bake time out of
`overture_base_water`'s `ocean` subtype, and `HAS_GROUND_SQL`
(`crates/pxl/server/geo-bake/src/terrain/leaf.rs`) calls a page open water
when no `overture_base_land` row or building touches its bbox and no way
(not a ferry) comes within `DRAWN_REACH_U` of it.

Each catalog keeps one row per shard: `overture_catalog` a row per table and
shard, `elevation_catalog` a row per shard, each with its release.
`read_source_catalogs()` (`bake_tiles.rs`) copies them into every baked
version's `manifest.json`, which is how a published set says what data it was
made from.

### Overture Maps

A public S3 release — no key, no portal step. The ingest reaches it through
Overture's official `overturemaps` CLI, installed on first use into
`cache/overture/.venv`: it downloads a type's GeoParquet over every shard at once, a little wider
since its bbox test is strict, to `cache/overture/<type>.parquet`, kept until
the release changes; the ingest then puts each feature in its shard.

### GEDTM30 v1.2 (OpenGeoHub / OpenLandMap)

One global Cloud-Optimized GeoTIFF, each shard warped straight out of the
remote file over HTTP range requests onto one grid: the shard's own edges and
the stride the bake samples elevation at (z20, `TILE_ELE_SPAN` to a leaf), so
the shards meet without a seam and a bake's sample is a pixel. There is no
raw-file staging step and no portal page to click through.

It is a bare-earth DTM: ground elevation carries no building or canopy height.

## 2. Prerequisites (macOS)

```sh
brew install gdal postgis jq gettext
brew link --force gettext          # puts envsubst on PATH

rustup toolchain install nightly   # the Overture ingest is a single-file cargo script
```

`gdal` for `gdalwarp` and `raster2pgsql`, `postgis` for the ingestion
utilities, `jq` and `envsubst` for the elevation script, `python3` for the
Overture CLI's venv. Docker runs the database and the GDAL image.

## 3. Bring up db-geo

Run from the project root.

```sh
docker compose -f infra/local/docker-compose.yml --env-file .env up -d db-geo

export $(grep -v '^#' .env | xargs)
until docker exec "$DB_GEO_CONTAINER" pg_isready -U postgres; do sleep 2; done
```

That starts a database that is already built. Emptying it first takes a
`down -v` before that line, and is almost never what you want:

> The data volume is the only copy of the ingested data on this machine. The
> sources are not in the repository — the warped rasters land under the
> gitignored `elevation/warped/`. Re-ingesting reproduces an equivalent dataset
> from each source's current state, not a byte-for-byte copy.

## 4. Run the ingests

One script takes every Overture type into a table a type, a shard at a time;
the elevation is its own, a shard at a time too. Neither takes a flag: the shards
are the coverage's.

```sh
./infra/geo/elevation/script/setup-elevation.sh

./infra/geo/overture/script/ingest.rs
```

Buildings is the long one: 7.2 million rows for Korea.

Every row is its GERS id, all of its properties in `properties` and its
geometry. Nothing is dropped on the way in: what a reader uses of a table is
chosen in its own query, and a column it filters on is generated from
`properties` by Postgres.

A shard whose catalog row already names the latest release is skipped;
deleting a shard's catalog row is what makes it read again. A table keyed
otherwise than by shard and id is refused, with the word to drop it.

Production keeps its own copy of the three tables search reads, and a
re-ingest reaches it only when they are copied over: one that changes a
table's shape is copied before the servers that read it are deployed
([infra/README.md](../README.md) §3, Data migration).

## 5. Verify

```sql
SELECT 'overture_buildings' AS t, count(*) FROM overture_buildings
UNION ALL SELECT 'overture_transportation', count(*) FROM overture_transportation
UNION ALL SELECT 'overture_base_land', count(*) FROM overture_base_land
UNION ALL SELECT 'overture_base_land_use', count(*) FROM overture_base_land_use
UNION ALL SELECT 'overture_base_water', count(*) FROM overture_base_water
UNION ALL SELECT 'overture_base_infrastructure', count(*) FROM overture_base_infrastructure
UNION ALL SELECT 'overture_places', count(*) FROM overture_places
UNION ALL SELECT 'overture_divisions', count(*) FROM overture_divisions
UNION ALL SELECT 'overture_division_points', count(*) FROM overture_division_points
UNION ALL SELECT 'overture_division_boundaries', count(*) FROM overture_division_boundaries
UNION ALL SELECT 'overture_building_parts', count(*) FROM overture_building_parts
UNION ALL SELECT 'overture_connectors', count(*) FROM overture_connectors
UNION ALL SELECT 'overture_base_land_cover', count(*) FROM overture_base_land_cover
UNION ALL SELECT 'overture_base_bathymetry', count(*) FROM overture_base_bathymetry
UNION ALL SELECT 'overture_addresses', count(*) FROM overture_addresses;

SELECT count(*), min(ST_ScaleX(rast)), max(ST_ScaleX(rast)) FROM elevation_raster;
```

The catalog is the other half of the check, and the faster one: every table
should carry a row for each of the coverage's shards, all of one release.

```sql
SELECT file_name, count(*) AS shards, min(data_date), max(data_date), sum(total_count)
FROM overture_catalog GROUP BY file_name ORDER BY file_name;
```

A shard with rows but no catalog row means an ingest stopped inside it. A
`data_date` that differs from the rest was filled from a different Overture
release, and a bake over the set would mix them; a bake refuses both.

After a large ingest, `VACUUM ANALYZE` — or `VACUUM FULL ANALYZE` where
downtime is acceptable — reclaims what replacing a shard's rows leaves behind.

## 6. How the elevation pipeline works

Bash, `envsubst` and GDAL, driven by `metadata.json`:

1. **Schema.** `init/01_elevation_schema.sql`, which creates the raster and
   catalog tables if they are missing.
2. **The shards.** `elevation_shards` (pxl-geo-bake) prints each published
   shard: its name, its EPSG:3857 edges and the bake's stride.
3. **Crop and reproject.** `gdalwarp` inside the GDAL image reads the remote
   COG through `/vsicurl/` and writes the shard's edges at that stride, in
   EPSG:3857 with DEFLATE, to `warped/<shard>.tif`.
4. **Ingest.** `raster2pgsql -a -F -t 256x256` piped into `psql`: the shard's
   raster in 256×256 tiles, each carrying its file's name, then its catalog
   row.

## 7. Backup

```sh
docker exec -t $DB_GEO_CONTAINER pg_dump -U $PGUSER -d $PGDATABASE \
  -t public.elevation_raster -t public.elevation_catalog | gzip > elevation_backup.sql.gz

gunzip -c elevation_backup.sql.gz | docker exec -i $DB_GEO_CONTAINER psql -U $PGUSER -d $PGDATABASE
```

Only elevation is worth backing up. The Overture tables rebuild from the public
release by re-running their ingests; the warped GeoTIFF the raster came from is
gigabytes and gitignored, so restoring that means downloading and warping again.

## 8. Credentials

Everything reads the repository's own `.env` — `DB_GEO_URL` for the ingests,
`DB_GEO_CONTAINER` and the rest for the shell script. Neither source needs a
key: Overture's release and the GEDTM30 COG are both public.
