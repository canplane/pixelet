# Geo-Infrastructure Operational Guide

This document describes the infrastructure layout, data pipelines, and operational procedures for pixelet's geographic information system (GIS).
The infrastructure is organized into independent layers — **Vector (features, coastline)** and **Raster (elevation)** — all managed together within a single PostGIS instance (`db-geo`).

## 1. Database Topology

| Service     | Port (Ext:Int) | Role                                        | Stack          |
| :---------- | :------------- | :------------------------------------------ | :------------- |
| **db-main** | 5432:5432      | Business logic & Metadata                   | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | **Spatial Data Hub** (Features + Elevation) | PostGIS 16-3.4 |

## 2. Data Management Rules

Data is managed across three separate pipelines according to its nature. All three pipelines share the same design philosophy (Symmetry).
(Paths below are relative to this document's own location, `infra/geo/`.)

| Feature         | **OSM Features (Vector)**                                        | **ESA Elevation (Raster)**                | **OSM Coastline (Vector)**                |
| :-------------- | :--------------------------------------------------------------- | :---------------------------------------- | :---------------------------------------- |
| **Source**      | OpenStreetMap (PBF)                                              | Copernicus GLO-30 (GeoTIFF)               | OSM Coastline (Water Polygons)            |
| **Directory**   | `./osm-features/`                                                | `./esa-elevation/`                        | `./osm-coastline/`                        |
| **Raw Data**    | `./osm-features/region/[CODE]/raw/*.osm.pbf`                     | `./esa-elevation/region/[CODE]/raw/*.tif` | `./osm-coastline/region/global/raw/*.shp` |
| **Main Table**  | 1. `osm_elements` **(Primary)**<br>2. `osm_roads` **(Reserved)** | `esa_elevation_raster`                    | `osm_coastline_polygons` **(Primary)**    |
| **Catalog**     | `osm_features_catalog`                                           | `esa_elevation_catalog`                   | `osm_coastline_catalog`                   |
| **Schema File** | `01_osm_features_schema.sql`                                     | `01_esa_elevation_schema.sql`             | `01_osm_coastline_schema.sql`             |
| **SRID**        | EPSG:3857 (Web Mercator)                                         | EPSG:3857 (Web Mercator)                  | EPSG:3857 (Web Mercator)                  |

## 3. How to Run (Quick Start)

This section covers running the full pipeline using the standard automation scripts.

### Prerequisites (macOS)

The following tools must be installed in your local Mac environment to build the data infrastructure.

```bash
# 1. Install the toolchain (gettext is required for envsubst)
brew install osm2pgsql gdal postgis jq gettext

# 2. Link the gettext tools (envsubst, etc.) onto the system PATH
brew link --force gettext
```

- **osm2pgsql**: Vector data ingestion engine (PBF processing)
- **gdal**: Raster/vector data conversion toolkit (warping, `ogr2ogr`, `raster2pgsql`, etc.)
- **postgis**: PostgreSQL spatial extension and DB ingestion utilities
- **jq**: JSON metadata parsing and manipulation
- **gettext (envsubst)**: Environment variable substitution inside shell scripts

### Execution Steps

All commands are run from the **project root (`pixelet/`)**.

#### 1. Start the Geo Database container and health-check it

Brings up the container using the `.env` file at the root.

```bash
# 1. Bring up the infrastructure (clean start, dropping any existing data volume)
docker compose -f infra/docker-compose.yml --env-file .env down -v
docker compose -f infra/docker-compose.yml --env-file .env up -d db-geo

# 2. Load the .env environment variables into the current shell session
export $(grep -v '^#' .env | xargs)

# 3. Confirm the DB is reachable (health check)
echo "Waiting for PostGIS [$DB_GEO_CONTAINER] to be ready..."
until docker exec "$DB_GEO_CONTAINER" pg_isready -U postgres; do
  echo 'DB is unavailable - sleeping...'
  sleep 2
done
echo '✅ DB is ready!'
```

#### 2. Ingest each layer (order doesn't matter)

Each script internally loads the environment variables from the root `.env` to handle authentication and connection details.

```bash
# [Channel 1] Build OSM Features (roads, buildings, etc.)
chmod +x infra/geo/osm-features/scripts/setup-osm-features.sh
./infra/geo/osm-features/scripts/setup-osm-features.sh

# [Channel 2] Build ESA Elevation (elevation data)
chmod +x infra/geo/esa-elevation/scripts/setup-esa-elevation.sh
./infra/geo/esa-elevation/scripts/setup-esa-elevation.sh

# [Channel 3] Build OSM Coastline (coastline/water surface)
chmod +x infra/geo/osm-coastline/scripts/setup-osm-coastline.sh
./infra/geo/osm-coastline/scripts/setup-osm-coastline.sh
```

## 4. Ingestion Workflow (Deep Dive)

The internal mechanics of each pipeline. Refer to this section for debugging and maintenance. Every layer shares **Clean Start (Drop & Create)** and **catalog-based state tracking** to maintain data integrity.

### Pipeline A: OSM Features (Vector)

#### Step 1: Schema Initialization

- Drops the existing tables (`osm_elements`, `osm_roads`) and catalog (`osm_features_catalog`) to prevent contamination from stale data.
- Runs `init/01_osm_features_schema.sql` to verify the PostGIS extension and recreate the table definitions and spatial/tag indexes.

#### Step 2: Raw Processing (osm2pgsql)

- Iterates over `metadata.json` and processes the target PBF data.
- The **`--hstore-all`** option is central here: it preserves **all attribute data losslessly in the `tags` column** with no separate column mapping. This gives the schema flexibility to accept any tag without a migration.
- The `--multi-geometry` option preserves the source data's geometry without loss.

#### Step 3: Sequential Merge & Optimization

- **Migration**: Generates a temporary SQL file to avoid shell-scripting errors. Moves data from the temporary tables (`planet_osm_point`, etc.) into the main table (`osm_elements`), consolidating the hstore-merged tags and geometry into a single table.
- **Physical Clustering**: Immediately after migration, runs a **`CLUSTER` command** keyed on the spatial index (`GiST`) to physically optimize the on-disk data layout (maximizes range-query performance).
- **Generalized Roads (`osm_roads`)**: Keeps a separately-stored, simplified geometry for rendering performance at zoomed-out levels.

#### Step 4: Finalization & Catalog Update

- Drops the temporary tables (`planet_osm_*`), updates `osm_features_catalog`'s status to `COMPLETED`, and records the total number of records processed.

---

### Pipeline B: ESA Elevation (Raster)

#### Step 1: Schema Initialization

- Drops the existing `esa_elevation_raster` and `esa_elevation_catalog`.
- Injects `init/01_esa_elevation_schema.sql` to create the catalog structure. (The raster table itself is managed automatically by the `raster2pgsql` ingestion tool.)

#### Step 2: Reprojection (GDAL Warp)

- Runs `gdalwarp` inside the Docker container (`/data`) environment.
- Reprojects the raw GeoTIFF (EPSG:4326) into the system standard, **EPSG:3857**, applying `DEFLATE` compression for efficient storage.

#### Step 3: Indexing & Ingestion (Hybrid Workflow)

- **Path Mapping**: Runs `gdalbuildvrt` with a path substitution list for the Docker container (`/data`) to build a virtual raster index.
- **Path Symmetry**: Rewrites (via `sed`) the virtual paths inside the generated VRT back to **local host paths**, so local tools can locate the actual files.
- **Stream Ingestion**: Pipes the local `raster2pgsql` directly into Docker's `psql` for ingestion.
  - `-t 256x256`: matches the page size required for the 3D voxel grid computation to be optimal.

#### Step 4: Finalization & Catalog Update

- Runs `ANALYZE` on the ingested data and updates `esa_elevation_catalog`'s status to `COMPLETED` to mark it as available.

---

### Pipeline C: OSM Coastline (Vector)

#### Step 1: Schema Initialization

- Drops the existing table (`osm_coastline_polygons`) and catalog (`osm_coastline_catalog`).
- Runs `init/01_osm_coastline_schema.sql` to recreate the coastline-specific table and spatial index.

#### Step 2: Raw Processing (ogr2ogr)

- Uses GDAL's `ogr2ogr` tool to load Shapefile (.shp) data directly into PostGIS.
- Iterates over the global coastline data as defined in `metadata.json`, physically consolidating it into a single table via the `-update -append` options.

#### Step 3: Topology Validation & Optimization

- Generates and runs a **`temp_optimization.sql` file** to avoid shell-parsing errors and keep the operation atomic.
- **ST_MakeValid**: Automatically corrects geometry errors (self-intersections, etc.) introduced during ingestion.
- **Spatial Clustering**: Runs a `CLUSTER` command keyed on the GiST index to optimize spatial query performance.

#### Step 4: Finalization & Catalog Update

- Removes the temporary SQL file, updates `osm_coastline_catalog`'s status to `COMPLETED`, and syncs the final count of ingested objects.

## 5. Maintenance Notes

- **Verification Query**: Use the queries below to verify data was ingested correctly after a run.

  ```sql
  -- Vector Count Check
  SELECT type, count(*) FROM osm_elements GROUP BY type;

  -- Raster Metadata Check
  SELECT r_table_name, r_raster_column, srid, scale_x, scale_y
  FROM raster_columns
  WHERE r_table_name = 'esa_elevation_raster';

  -- Coastline Validity Check
  SELECT count(*) as total, count(*) FILTER (WHERE ST_IsValid(geom)) as valid FROM osm_coastline_polygons;
  ```

- **SRID Consistency**: All spatial data must strictly adhere to **EPSG:3857 (Web Mercator)**. This prevents coordinate drift during the rendering engine's 3D voxel computations.
- **Idempotency**: Every script includes either **Clean Start (DROP & CREATE)** or upsert logic so that re-running it never produces duplicate data or conflicts.
- **Directory Structure**:

  ```
  infra/geo/
  ├── osm-features/   # Vector Layer (Roads, Buildings)
  ├── esa-elevation/  # Raster Layer (Height)
  └── osm-coastline/  # Vector Layer (Water Polygons)
  ```

- **Network Context**:
  - **Local Access**: `localhost:5433` (when using external tools)
  - **Container Access**: `db-geo:5432` (for internal Docker-to-Docker communication)

- **Backup & Recovery Strategy**: In addition to full-DB backups, generating per-layer backup files is recommended so each layer can be recovered independently.

  ```bash
  # 1. Back up a specific layer (e.g. Coastline)
  docker exec -t $DB_GEO_CONTAINER pg_dump -U $PGUSER -d $PGDATABASE \
    -t osm_coastline_polygons -t osm_coastline_catalog > osm_coastline_backup.sql

  # 2. Back up elevation data (Raster) (compression recommended)
  docker exec -t $DB_GEO_CONTAINER pg_dump -U $PGUSER -d $PGDATABASE \
    -t public.esa_elevation_raster -t public.esa_elevation_catalog | gzip > esa_elevation_backup.sql.gz

  # 3. Restore data
  cat osm_coastline_backup.sql | docker exec -i $DB_GEO_CONTAINER psql -U $PGUSER -d $PGDATABASE
  ```

- **Storage Optimization**: Cleaning up dead tuples is essential right after a large-scale (Clean Start) ingestion. Once ingestion finishes, refresh statistics and reclaim storage with the commands below.
  ```sql
  VACUUM ANALYZE;      -- Refresh statistics and general cleanup (recommended while in operation)
  -- or
  VACUUM FULL ANALYZE; -- Physically reclaim disk space (recommended when downtime is acceptable)
  ```

## 6. Data Licensing & Attribution

Both external data sources this pipeline ingests require a specific attribution notice wherever the baked result is shown to end users — implemented in `apps/web/src/modules/map/ui/overlay/bottom_overlays.tsx`. Neither notice is a style choice; don't shorten, reword, or drop either one without re-checking the source license first.

| Source                                   | License                | Required notice                                                                                                                                                                                          |
| :---------------------------------------- | :---------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenStreetMap (features, coastline, and Nominatim reverse geocoding) | ODbL | `© OpenStreetMap contributors` — the "historical form" explicitly listed as acceptable by the [OSMF Attribution Guidelines](https://osmfoundation.org/wiki/Licence/Attribution_Guidelines). Covers both the baked map data and the Nominatim results (`use_reverse_geocode.ts`) under one notice — no separate wording needed per source. |
| Copernicus WorldDEM-30 (`esa-elevation/`) | [License-COPDEM-30](https://documentation.dataspace.copernicus.eu/APIs/SentinelHub/Data/DEM/resources/license/License-COPDEM-30.pdf), Article 6 | Since this pipeline reprojects (`gdalwarp`) and bakes the raw DEM — i.e. the data is adapted/modified, not redistributed as-is — Article 6(b)'s modified-data notice applies, quoted verbatim: `produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved` |

## 7. Security & Sensitive Information

- **Credentials**: Sensitive information such as DB passwords is injected only via the `.env` file — never hardcoded in documentation or code.
- **Environment Variables**: `DB_GEO_USER`, `DB_GEO_PASSWORD`, and similar variables must be loaded into the environment before running any script.
