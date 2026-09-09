# Geo-Infrastructure Operational Guide

This document describes the infrastructure layout, data pipelines, and operational procedures for pixelet's geographic information system (GIS).
The infrastructure is organized into independent layers — **Vector (coastline)** and **Raster (elevation)** — all managed together within a single PostGIS instance (`db-geo`).

Terrain's own building/road/land-use features moved off this directory's OSM pipeline entirely (`osm_elements`/`osm_roads`, dropped) onto Overture Maps — see [Overture ingestion](#3a-overture-buildings-transportation-base) below. Coastline's fine-grained shape moved too (Overture's Base `water` theme has an `ocean` subtype — confirmed created from the same OSMCoastline tool osm_coastline_polygons came from, QA'd). This directory's `osm-coastline/` pipeline stays only for the coarse land/ocean page pre-filter (`terrain/tools/bake_tiles.ts`'s own classification step, a cheap bbox-existence check with no real precision requirement) — everything that touches an actual page's coastline pixels reads Overture instead.

## 1. Database Topology

| Service     | Port (Ext:Int) | Role                                        | Stack          |
| :---------- | :------------- | :------------------------------------------ | :------------- |
| **db-main** | 5432:5432      | Business logic & Metadata                   | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | **Spatial Data Hub** (Features + Elevation) | PostGIS 16-3.4 |

## 2. Data Management Rules

This directory manages two pipelines directly — **ESA Elevation (Raster)** and **OSM Coastline (Vector)** — sharing the same design philosophy (Symmetry).
(Paths below are relative to this document's own location, `infra/geo/`.)

| Feature         | **ESA Elevation (Raster)**                | **OSM Coastline (Vector)**                |
| :-------------- | :---------------------------------------- | :---------------------------------------- |
| **Source**      | GEDTM30 (Cloud-Optimized GeoTIFF)         | OSM Coastline (Water Polygons)            |
| **Directory**   | `./esa-elevation/`                        | `./osm-coastline/`                        |
| **Raw Data**    | `./esa-elevation/region/[CODE]/raw/*.tif` | `./osm-coastline/region/global/raw/*.shp` |
| **Main Table**  | `esa_elevation_raster`                    | `osm_coastline_polygons` **(Primary)**    |
| **Catalog**     | `esa_elevation_catalog`                   | `osm_coastline_catalog`                   |
| **Schema File** | `01_esa_elevation_schema.sql`             | `01_osm_coastline_schema.sql`             |
| **SRID**        | EPSG:3857 (Web Mercator)                  | EPSG:3857 (Web Mercator)                  |

Buildings/Transportation/Base (the third vector layer terrain reads) live entirely outside this directory now — see the next section.

## 3. How to Run (Quick Start)

This section covers running the full pipeline using the standard automation scripts.

### Prerequisites (macOS)

The following tools must be installed in your local Mac environment to build the data infrastructure.

```bash
# 1. Install the toolchain (gettext is required for envsubst)
brew install gdal postgis jq gettext

# 2. Link the gettext tools (envsubst, etc.) onto the system PATH
brew link --force gettext
```

- **gdal**: Raster/vector data conversion toolkit (warping, `ogr2ogr`, `raster2pgsql`, etc.)
- **postgis**: PostgreSQL spatial extension and DB ingestion utilities
- **jq**: JSON metadata parsing and manipulation
- **gettext (envsubst)**: Environment variable substitution inside shell scripts

`osm2pgsql` is no longer part of this toolchain — it only ever backed the OSM Features pipeline (below), which no code reads anymore.

### Execution Steps

All commands are run from the **project root (`pixelet/`)**.

#### 1. Start the Geo Database container and health-check it

Brings up the container using the `.env` file at the root.

> **`down -v` destroys the data volume, and on this machine that volume is the only surviving copy of the ingested data.** The raw source files are not in the repository and are no longer on disk either (`region/*/raw/` holds only `.gitkeep`). Re-ingesting reproduces an equivalent dataset from each source's own current state, not necessarily the same one byte-for-byte. `down -v` belongs in a first build or a deliberate rebuild; to start an already-built database, drop that line.

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

#### 2. Obtain the raw data

**None of the raw data is in this repository** — every source file is gitignored (see `infra/geo/.gitignore`), and only `.gitkeep` placeholders mark the directories. The ingestion scripts assume the files are already in place.

Each pipeline's `metadata.json` lists the exact paths, and its `acquisition` block records whether the download is automated and where the file actually comes from. Paths there are relative to that pipeline's own directory (e.g. `infra/geo/esa-elevation/`).

| Layer         | Automated?                             | Source                                                                                                   |
| :------------ | :------------------------------------- | :------------------------------------------------------------------------------------------------------- |
| OSM Coastline | no — download and unzip manually       | [`land-polygons-split-3857.zip`](https://osmdata.openstreetmap.de/download/land-polygons-split-3857.zip) |
| ESA Elevation | **yes** — the script crops it directly | [OpenGeoHub/OpenLandMap, GEDTM30 v1.2](https://codeberg.org/openlandmap/GEDTM30)                         |

##### OSM Coastline (manual)

```bash
cd infra/geo/osm-coastline/region/global/raw
curl -LO https://osmdata.openstreetmap.de/download/land-polygons-split-3857.zip
unzip land-polygons-split-3857.zip     # -> land-polygons-split-3857/land_polygons.shp
rm land-polygons-split-3857.zip
```

The result must be `region/global/raw/land-polygons-split-3857/land_polygons.shp`, with its `.shx`/`.dbf`/`.prj`/`.cpg` sidecars beside it. The script names only the `.shp`; `ogr2ogr` resolves the rest itself. It errors out with `Main shapefile missing` if the path is wrong.

This is the **split** version (large polygons pre-split), not the plain one — it is what the coastline/land classification is tuned for.

##### ESA Elevation (automated)

Nothing to do. `provider.url` in `metadata.json` is a real, directly fetchable Cloud-Optimized GeoTIFF (GEDTM30 v1.2's own single global file, not per-region tiles and not an interactive portal page like the GLO-30 source this replaced) — `setup-esa-elevation.sh` crops each region's own `bbox_4326` straight out of it over HTTP range requests (`gdalwarp -te ... /vsicurl/<url>`), with no raw-file staging step.

GEDTM30 is a bare-earth model (fuses GLO-30 + ALOS World 3D with real ICESat-2/GEDI laser-altimetry returns via ML), unlike GLO-30's own surface model — building/canopy height no longer bakes into the ground elevation raster. Confirmed live: sampling this table at Lotte World Tower's own coordinates (a 555 m building) returns ~19.6 m, Seoul's real ground elevation, not the tower's rooftop height.

Coverage is one bbox per region (`metadata.json`'s own `bbox_4326`, not a `coverage[]` list of separate tiles the way GLO-30 needed) — real elevation exists only inside that rectangle; nothing outside `region_code: KR`'s own bbox is populated.

#### 3. Ingest each layer (order doesn't matter)

Each script internally loads the environment variables from the root `.env` to handle authentication and connection details.

```bash
# [Channel 1] Build ESA Elevation (elevation data)
chmod +x infra/geo/esa-elevation/scripts/setup-esa-elevation.sh
./infra/geo/esa-elevation/scripts/setup-esa-elevation.sh

# [Channel 2] Build OSM Coastline (coastline/water surface)
chmod +x infra/geo/osm-coastline/scripts/setup-osm-coastline.sh
./infra/geo/osm-coastline/scripts/setup-osm-coastline.sh
```

Buildings/Transportation/Base are a separate step, outside this directory — see [Overture ingestion](#3a-overture-buildings-transportation-base) below.

## 3a. Overture: Buildings, Transportation, Base

Terrain's building/road/land-use features come from three Deno tools under
`apps/api/src/bake/overture/tools/` — `ingest_buildings.ts`,
`ingest_transportation.ts`, `ingest_base.ts` — not from this directory's own
bash+`envsubst`+GDAL convention the two pipelines above use. Each tool's own
top comment is the source of truth for its mechanics (why `geojsonseq` over
plain `geojson`, which Overture `--type` values it pulls, why certain
subtypes are skipped); this section only orients where they sit relative to
everything else here.

```bash
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_buildings.ts
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_transportation.ts
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_base.ts
```

- **Toolchain**: the official `overturemaps` Python CLI, auto-installed into a local `.venv` next to the tools on first run (gitignored) — the one dependency here that isn't Deno/TS or GDAL.
- **Source**: Overture Maps Foundation's public S3 release, no API key or portal step — `overturemaps download --bbox=... --type=<theme>`.
- **Tables**: `overture_buildings`, `overture_transportation`, `overture_base_land_use`/`overture_base_water`/`overture_base_land`/`overture_base_infrastructure` — each tool's own `DROP TABLE IF EXISTS` + recreate is its own Clean Start, same convention as the two script-driven pipelines.
- **Catalog**: `overture_catalog` (`apps/api/src/bake/overture/catalog.ts`), one row per table, recording Overture's own release id (`overturemaps releases latest`, e.g. `2026-08-19.0`) as `data_date` — the data's actual as-of snapshot, not the day the ingest happened to run. `bake_util.ts`'s `read_source_catalogs()` picks this up into every baked tile version's `manifest.json`, alongside `esa_elevation_catalog`/`osm_coastline_catalog`.
- **Region**: `--bbox` flag, defaulting to all of South Korea (`124,33,132,39`) — same box the two directory-native pipelines use.

## 4. Ingestion Workflow (Deep Dive)

The internal mechanics of the two pipelines this directory still owns directly. Refer to this section for debugging and maintenance. Both share **Clean Start (Drop & Create)** and **catalog-based state tracking** to maintain data integrity — see [3a](#3a-overture-buildings-transportation-base) above for Overture's own version of the same idea.

### Pipeline B: ESA Elevation (Raster)

#### Step 1: Schema Initialization

- Drops the existing `esa_elevation_raster` and `esa_elevation_catalog`.
- Injects `init/01_esa_elevation_schema.sql` to create the catalog structure. (The raster table itself is managed automatically by the `raster2pgsql` ingestion tool.)

#### Step 2: Crop + Reprojection (GDAL Warp)

- Runs `gdalwarp` inside the Docker container (`/data`) environment, reading directly from the remote COG (`/vsicurl/<provider.url>`) — no local raw-file download.
- Crops to each region's own `bbox_4326` (`-te`, source CRS) while reprojecting into the system standard, **EPSG:3857**, applying `DEFLATE` compression.

#### Step 3: Ingestion

- Pipes the local `raster2pgsql` directly into Docker's `psql` for ingestion, over every region's own warped file at once (one row per file — no VRT mosaic needed, each region's crop is already one continuous raster).
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
  -- Vector Count Check (Overture Buildings/Transportation/Base)
  SELECT 'overture_buildings' AS table_name, count(*) FROM overture_buildings
  UNION ALL SELECT 'overture_transportation', count(*) FROM overture_transportation
  UNION ALL SELECT 'overture_base_land_use', count(*) FROM overture_base_land_use
  UNION ALL SELECT 'overture_base_water', count(*) FROM overture_base_water
  UNION ALL SELECT 'overture_base_land', count(*) FROM overture_base_land
  UNION ALL SELECT 'overture_base_infrastructure', count(*) FROM overture_base_infrastructure;

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
  ├── esa-elevation/  # Raster Layer (Height)
  └── osm-coastline/  # Vector Layer (Water Polygons)
  ```

  Buildings/Transportation/Base live at `apps/api/src/bake/overture/tools/`
  instead — see [3a](#3a-overture-buildings-transportation-base).

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

Two notices are actually required, shown to end users in `apps/web/src/app/settings/sections/attribution_section.tsx`. Neither is a style choice; don't shorten, reword, or drop one without re-checking the source license first. Nothing else needs its own notice — see the note below the table.

| Source                     | License                                                   | Required notice                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| :------------------------- | :-------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenStreetMap              | ODbL                                                      | `© OpenStreetMap contributors` — the "historical form" explicitly listed as acceptable by the [OSMF Attribution Guidelines](https://osmfoundation.org/wiki/Licence/Attribution_Guidelines). Covers every OSM-derived layer this project uses, direct or not — per [Overture's own attribution guidance](https://docs.overturemaps.org/attribution/), Buildings/Transportation/Base/Divisions are substantially OSM-sourced and carry this same ODbL obligation through, even though they're ingested from Overture's release rather than an OSM file directly. |
| GEDTM30 (`esa-elevation/`) | [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/) | Since this pipeline crops+reprojects (`gdalwarp`) the raw DTM — i.e. the data is adapted, not redistributed as-is — CC-BY 4.0 requires noting changes alongside attribution: `Contains modified GEDTM30 data. Ho Y, Grohmann CH, Lindsay J, Reuter HI, Parente L, Witjes M, Hengl T (2025). GEDTM30: global ensemble digital terrain model at 30 m and derived multiscale terrain variables. PeerJ 13:e19673.`                                                                                                                                                 |

Overture Maps Foundation's own name isn't legally required anywhere: Places' CDLA-Permissive-2.0 imposes no attribution condition at all (confirmed directly against the license text — its only notice clause is "share the license text alongside the data," not credit-the-source), and Buildings/Transportation/Base/Divisions carry ODbL only because they're substantially OSM-sourced — that obligation runs to OSM's own contributors, not to Overture as the redistributor. Overture's own [attribution page](https://docs.overturemaps.org/attribution/) does _recommend_ a combined `© OpenStreetMap contributors, Overture Maps Foundation` notice for those themes as its own best practice, but that's a suggestion, not something ODbL or CDLA-Permissive-2.0 actually mandates — this project keeps the notice at the legally-required minimum (OSM + GEDTM30) rather than including it.

## 7. Security & Sensitive Information

- **Credentials**: Sensitive information such as DB passwords is injected only via the `.env` file — never hardcoded in documentation or code.
- **Environment Variables**: `DB_GEO_USER`, `DB_GEO_PASSWORD`, and similar variables must be loaded into the environment before running any script.
