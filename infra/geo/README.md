# Filling db-geo

`db-geo` is the spatial database: twelve tables from two sources, one PostGIS
instance. Everything the client sees of the world — terrain, POI, reverse
geocode — is baked from these tables offline and published to a bucket; only
forward search reads them at request time.

This directory holds what fills them. Nothing here is deployed, and nothing
here runs on a schedule except the monthly Overture refresh in CI. These are
run by hand, a few times a year, against a database that is expensive to
rebuild.

> **Every ingest drops its tables before it writes.** This is the first thing
> to know about running one. `--bbox` bounds what is fetched from the source,
> not what is removed: a run with a small box leaves a small table, not a
> partially updated one. There is no incremental mode and no merge — Overture
> cuts a new dated release rather than streaming edits, so each run replaces
> what it owns. Running an ingest to check that it starts is enough to empty
> the table it owns.

```
infra/geo/
├── overture/                     five themes, one provider, one catalog
│   ├── init/                       one schema file per theme, plus the catalog's
│   └── script/                     ingest_<theme>.rs, and the module they share
└── elevation/                    one raster, out of one global GeoTIFF
    ├── init/01_elevation_schema.sql
    ├── metadata.json               the regions, their bboxes and the source URL
    └── script/setup-elevation.sh
```

## 1. The tables

| Source                                                                           | Tables                                                                                                                                                 | Catalog             | Fills them                            |
| :------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------ | :------------------------------------ |
| **Overture — Buildings, Transportation, Base**<br>terrain features and coastline | `overture_buildings`, `overture_transportation`, `overture_base_land`, `overture_base_land_use`, `overture_base_water`, `overture_base_infrastructure` | `overture_catalog`  | `overture/script/ingest.rs`           |
| **Overture — Places, Divisions**<br>POI, reverse geocode, forward search         | `overture_places`, `overture_divisions`, `overture_division_points`                                                                                    | `overture_catalog`  | `overture/script/ingest.rs`           |
| **GEDTM30**<br>ground elevation                                                  | `elevation_raster`                                                                                                                                     | `elevation_catalog` | `elevation/script/setup-elevation.sh` |

Coastline is not a table of its own. `OVERTURE_COAST_SQL`
(`crates/pxl/server/geo-bake/src/terrain/repository.rs`) carves it at bake time out of
`overture_base_water`'s `ocean` subtype, and `HAS_GROUND_SQL`
(`crates/pxl/server/geo-bake/src/terrain/leaf.rs`) calls a page open water
when no `overture_base_land` row or building touches its bbox and no way
(not a ferry) comes within `DRAWN_REACH_U` of it.

Both catalogs keep one row per table: which bbox it was pulled with and which
release it is from. `read_source_catalogs()` (`bake_tiles.rs`) copies them into
every baked version's `manifest.json`, which is how a published set says what
data it was made from.

### Overture Maps

A public S3 release — no key, no portal step. The ingests reach it through the
official `overturemaps` Python CLI, installed on first use into
`./out/overture/.venv` (gitignored). They ask for `-f geojsonseq` and read the
extract a line at a time: South Korea's full Places theme is 700,000+ features,
and as one pretty-printed document it runs to about 900 MB.

There is no default bbox: it is derived from the elevation's `bbox_4326`
(§4).

### GEDTM30 v1.2 (OpenGeoHub / OpenLandMap)

One global Cloud-Optimized GeoTIFF, cropped to each region's `bbox_4326`
straight out of the remote file over HTTP range requests. There is no raw-file
staging step and no portal page to click through.

It is a bare-earth DTM: ground elevation carries no building or canopy height.
Real elevation exists only inside `metadata.json`'s `bbox_4326`; outside that
box there is nothing, not zero.

## 2. Prerequisites (macOS)

```sh
brew install gdal postgis jq gettext
brew link --force gettext          # puts envsubst on PATH

rustup toolchain install nightly   # the Overture ingests are single-file cargo scripts
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
> gitignored `out/elevation/`. Re-ingesting reproduces an equivalent dataset
> from each source's current state, not a byte-for-byte copy.

## 4. Run the ingests

One script takes every Overture type, and every feature of each in the
bbox, into a table a type; the elevation is its own:

```sh
./infra/geo/elevation/script/setup-elevation.sh

./infra/geo/overture/script/ingest.rs --bbox "$(cargo run -q --manifest-path crates/Cargo.toml --bin ingest_bbox)"
```

The bbox is the bake's to say, and `ingest_bbox` prints it: the pages whose
elevation grid reads only `metadata.json`'s `bbox_4326` are what is
published (`crates/pxl/server/geo-bake/src/coverage.rs`), and the bbox is
their rectangle grown by whole pages on each side, the fewest that hold the
farthest an Overture read goes past a page (`Coverage::margin_pages`), in
lon/lat. Pass it as printed: a bake refuses to run
unless every `overture_*` table its statements read has a catalog row with
exactly that bbox, all of one release, and the error prints the `--bbox` to
ingest with.

Buildings is the long one: 7.2 million rows for Korea.

Every row is its GERS id, all of its properties in `properties` and its
geometry. Nothing is dropped on the way in: what a reader uses of a table is
chosen in its own query, and a column it filters on is generated from
`properties` by Postgres.

The flags are `--bbox`, required, and `--out` for the
directory the extract is kept in. An extract is every column of every feature
in the box, and an ingest whose extract is already the latest release over the
same bbox reads it instead of downloading it again — so a change to what a
table keeps of a theme is a re-run, not a re-download. Keep the extracts;
deleting one is what forces a fresh pull.

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

SELECT r_table_name, srid, scale_x, scale_y FROM raster_columns
WHERE r_table_name = 'elevation_raster';
```

The catalog is the other half of the check, and the faster one: every table
should carry a row, with the same bbox and the same release.

```sql
SELECT file_name, region_code, data_date, total_count, imported_at::date
FROM overture_catalog ORDER BY file_name;
```

A table with rows but no catalog entry means an ingest stopped between its last
insert and its last statement. A table whose `data_date` differs from its
neighbors' was filled from a different Overture release, and a bake over the
set would mix them.

After a large ingest, `VACUUM ANALYZE` — or `VACUUM FULL ANALYZE` where
downtime is acceptable — reclaims what drop-and-recreate leaves behind.

## 6. How the elevation pipeline works

Bash, `envsubst` and GDAL, driven by `metadata.json`:

1. **Schema.** Drops `elevation_raster` and `elevation_catalog`, then runs
   `init/01_elevation_schema.sql`. The raster table itself is created by
   `raster2pgsql`, which is why the schema file documents it rather than owning
   it.
2. **Crop and reproject.** `gdalwarp` inside the GDAL image reads the remote
   COG through `/vsicurl/`, crops to `bbox_4326` (`-te`, in the source CRS), and
   writes EPSG:3857 with DEFLATE into `out/elevation/region/<CODE>/warped/`.
3. **Ingest.** `raster2pgsql -t 256x256` piped into `psql`, one row per region
   file, the raster chunked into 256×256 tiles inside the table.
4. **Finalize.** `ANALYZE`, then `elevation_catalog.status = COMPLETED` with the
   row count.

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
