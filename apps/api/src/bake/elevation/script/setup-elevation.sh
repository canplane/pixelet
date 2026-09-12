#!/bin/bash

set -e
set -o pipefail

# Base paths and environment
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELEV_ROOT="$(dirname "$SCRIPT_DIR")"

REPO_ROOT="$(cd "$ELEV_ROOT/../../../../.." && pwd)"
# The warped rasters: gigabytes, under the repository's one gitignored
# working directory (out/, which the Docker build context also ignores),
# never under the source tree this script lives in.
DATA_ROOT="$REPO_ROOT/out/elevation"

# Load .env
DOTENV_PATH="$REPO_ROOT/.env"

if [ -f "$DOTENV_PATH" ]; then
    echo "✅ Loading environment variables from $DOTENV_PATH"
    export $(echo $(grep -v '^#' "$DOTENV_PATH" | xargs) | envsubst)
else
    echo "❌ Error: .env file not found at $DOTENV_PATH"
    exit 1
fi

# Required-variable guard
if [ -z "$DB_GEO_CONTAINER" ]; then
    echo "❌ Error: DB_GEO_CONTAINER is not set. Check your .env file."
    exit 1
fi

export PGPASSWORD="${DB_GEO_PASSWORD}"

# DB connection settings
DB_CONTAINER="${DB_GEO_CONTAINER}"
PGUSER="${DB_GEO_USER:-admin}"
PGDATABASE="${DB_GEO_NAME:-geo}"
PGPORT="${DB_GEO_PORT:-5433}"
DB_HOST="${DB_GEO_HOST:-localhost}"

# Metadata file location
METADATA_FILE="$ELEV_ROOT/metadata.json"
GDAL_IMAGE="ghcr.io/osgeo/gdal:ubuntu-small-latest"

echo "🏔️  Starting elevation-data integration"

# ---------------------------------------------------------
# Step 1: Metadata Parsing
# ---------------------------------------------------------
if [ ! -f "$METADATA_FILE" ]; then
  echo "❌ Error: Metadata file not found"
  exit 1
fi

ENTRY_COUNT=$(jq '. | length' "$METADATA_FILE")
if [ "$ENTRY_COUNT" -eq 0 ]; then
  echo "❌ Error: No entries found"
  exit 1
fi

echo "🔍 Found $ENTRY_COUNT region(s)"

# ---------------------------------------------------------
# Step 2: Schema Initialization
# ---------------------------------------------------------
echo "🧹 Initializing Database Schema for Elevation..."
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"CREATE EXTENSION IF NOT EXISTS postgis_raster;"

docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"DROP TABLE IF EXISTS public.elevation_raster, public.elevation_catalog CASCADE;"

SCHEMA_FILE="$ELEV_ROOT/init/01_elevation_schema.sql"
if [ -f "$SCHEMA_FILE" ]; then
    docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" < "$SCHEMA_FILE"
else
    echo "⚠️  Warning: Schema file not found."
fi

# ---------------------------------------------------------
# Step 3: Loop Processing
# ---------------------------------------------------------
# GEDTM30 ships as one global Cloud-Optimized GeoTIFF, not per-region tiles
# like GLO-30 was — each entry's own bbox_4326 is warped directly out of the
# remote COG via HTTP range requests (/vsicurl/), so there's no raw-file
# staging step and no portal-page refusal case. Still one raster2pgsql call
# for every region's warped file together at the end (Step 4), same as the
# old VRT-mosaic call did — raster2pgsql accepts multiple input files
# directly (one row per file), so skipping the VRT step here doesn't need a
# per-region -d/-a split.
WARPED_FILES=()

for ((i=0; i<ENTRY_COUNT; i++)); do
    ENTRY=$(jq -r ".[$i]" "$METADATA_FILE")
    REGION_CODE=$(echo "$ENTRY" | jq -r '.region_code')
    DOWNLOAD_URL=$(echo "$ENTRY" | jq -r '.provider.url')
    DATA_DATE=$(echo "$ENTRY" | jq -r '.data_date // empty')
    if [ -z "$DATA_DATE" ]; then DATA_DATE=$(date +%Y-%m-%d); fi

    WEST=$(echo "$ENTRY" | jq -r '.bbox_4326.west')
    SOUTH=$(echo "$ENTRY" | jq -r '.bbox_4326.south')
    EAST=$(echo "$ENTRY" | jq -r '.bbox_4326.east')
    NORTH=$(echo "$ENTRY" | jq -r '.bbox_4326.north')

    echo "==================================================="
    echo "📍 Processing Region: $REGION_CODE"
    echo "==================================================="

    REGION_LOWER=$(echo "$REGION_CODE" | tr '[:upper:]' '[:lower:]')
    WARPED_DIR="$DATA_ROOT/region/$REGION_CODE/warped"
    WARPED_FILENAME="gedtm30_${REGION_LOWER}_3857.tif"
    WARPED_FILE="$WARPED_DIR/$WARPED_FILENAME"
    mkdir -p "$WARPED_DIR"

    # Register status
    docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
    "INSERT INTO elevation_catalog (region_code, data_date, file_name, status)
     VALUES ('$REGION_CODE', '$DATA_DATE', '$WARPED_FILENAME', 'PROCESSING')
     ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

    # Crop + reproject straight from the remote COG — te is in the SOURCE crs
    # (EPSG:4326, -te_srs), so the region's own lon/lat bbox needs no manual
    # conversion; -t_srs 3857 matches this project's own working CRS.
    echo "🔄 Cropping + re-projecting to 3857 (Warping)..."
    docker run --rm -v "$DATA_ROOT":/data "$GDAL_IMAGE" \
        gdalwarp -te "$WEST" "$SOUTH" "$EAST" "$NORTH" -te_srs EPSG:4326 \
        -t_srs EPSG:3857 -overwrite -r bilinear -of GTiff -co COMPRESS=DEFLATE \
        "/vsicurl/$DOWNLOAD_URL" "/data/region/$REGION_CODE/warped/$WARPED_FILENAME"

    WARPED_FILES+=("$WARPED_FILE")
done

if [ ${#WARPED_FILES[@]} -eq 0 ]; then
    echo "⚠️  No regions were processed. Exiting."
    exit 0
fi

echo "📥 Ingesting raster(s)..."
raster2pgsql -d -s 3857 -I -C -M -t 256x256 "${WARPED_FILES[@]}" public.elevation_raster \
    | docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE"

# ---------------------------------------------------------
# Step 4: Finalization
# ---------------------------------------------------------
echo "🚀 Finalizing..."
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
ANALYZE public.elevation_raster;
UPDATE elevation_catalog
SET status = 'COMPLETED',
    total_count = (SELECT count(*) FROM public.elevation_raster),
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

echo "✨ Elevation integration completed successfully!"
