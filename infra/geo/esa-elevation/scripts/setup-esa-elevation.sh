#!/bin/bash

set -e
set -o pipefail

# Base paths and environment
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELEV_ROOT="$(dirname "$SCRIPT_DIR")"

# Load .env
DOTENV_PATH="$(cd "$ELEV_ROOT/../../../" && pwd)/.env"

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
"DROP TABLE IF EXISTS public.esa_elevation_raster, public.esa_elevation_catalog CASCADE;"

SCHEMA_FILE="$ELEV_ROOT/init/01_esa_elevation_schema.sql"
if [ -f "$SCHEMA_FILE" ]; then
    docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" < "$SCHEMA_FILE"
else
    echo "⚠️  Warning: Schema file not found."
fi

# ---------------------------------------------------------
# Step 3: Loop Processing
# ---------------------------------------------------------
IS_FIRST_RUN=true

for ((i=0; i<ENTRY_COUNT; i++)); do
    ENTRY=$(jq -r ".[$i]" "$METADATA_FILE")
    REGION_CODE=$(echo "$ENTRY" | jq -r '.region_code')
    DOWNLOAD_URL=$(echo "$ENTRY" | jq -r '.provider.url // empty')
    DATA_DATE=$(echo "$ENTRY" | jq -r '.data_date // empty')
    
    if [ -z "$DATA_DATE" ]; then DATA_DATE=$(date +%Y-%m-%d); fi
    
    FILES=()
    while IFS= read -r line; do
        FILES+=("$line")
    done < <(echo "$ENTRY" | jq -r '.files[]')

    echo "==================================================="
    echo "📍 Processing Region: $REGION_CODE"
    echo "==================================================="

    for REL_FILE in "${FILES[@]}"; do
        RAW_FILE="$ELEV_ROOT/$REL_FILE"
        FILENAME=$(basename "$RAW_FILE")
        
        WARPED_DIR="$(dirname "$RAW_FILE")/../warped"
        
        # Derive the warped filename (4326 -> 3857)
        if [[ "$FILENAME" == *"4326"* ]]; then
            WARPED_FILENAME="${FILENAME//4326/3857}"
        else
            WARPED_FILENAME="${FILENAME%.*}_3857.tif"
        fi
        WARPED_FILE="$WARPED_DIR/$WARPED_FILENAME"
        
        echo "📂 Processing File: $FILENAME"
        mkdir -p "$(dirname "$RAW_FILE")"
        mkdir -p "$WARPED_DIR"

        # Auto-download only when the metadata URL actually points at a file.
        # provider.url here is OpenTopography's interactive portal page, and an
        # unguarded curl saves that HTML under the .tif name and exits 0 — the
        # failure then surfaces much later as GDAL's opaque "not recognized as
        # being in a supported file format", and because the bogus file now
        # exists, every re-run skips the download and fails identically forever.
        # Refuse up front and say where the file has to come from instead.
        if [ ! -f "$RAW_FILE" ]; then
            case "$DOWNLOAD_URL" in
                *.tif | *.tiff | *.zip)
                    echo "⬇️  Downloading from $DOWNLOAD_URL..."
                    curl -fL -o "$RAW_FILE" "$DOWNLOAD_URL"
                    ;;
                *)
                    echo "❌ Missing raw file: $RAW_FILE"
                    echo "   This source has no direct download URL — provider.url is a portal page,"
                    echo "   not a file, so it cannot be fetched automatically."
                    echo "   See infra/geo/README.md > 'Obtaining the raw data' for the exact"
                    echo "   bounding box to request for this file, and the filename to save it as."
                    exit 1
                    ;;
            esac
        fi

        # Register status
        docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
        "INSERT INTO esa_elevation_catalog (region_code, data_date, file_name, status) 
         VALUES ('$REGION_CODE', '$DATA_DATE', '$FILENAME', 'PROCESSING')
         ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

        # Warp, inside the container
        echo "🔄 Re-projecting to 3857 (Warping)..."
        docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
            gdalwarp -s_srs EPSG:4326 -t_srs EPSG:3857 \
            -overwrite -r bilinear -of GTiff -co COMPRESS=DEFLATE \
            "/data/$REL_FILE" "/data/${REL_FILE%/*}/../warped/$WARPED_FILENAME"
        
        IS_FIRST_RUN=false
    done
done

# ---------------------------------------------------------
# Step 4: VRT & Ingestion
# ---------------------------------------------------------
if [ "$IS_FIRST_RUN" = true ]; then
    echo "⚠️  No files were processed. Exiting."
    exit 0
fi

echo "📦 Building global VRT index..."

# 1. Build the list of warped files, as host paths
find "$ELEV_ROOT" -type f -path "*/warped/*_3857.tif" > "$ELEV_ROOT/warped_file_list.txt"

# 2. Rewrite those paths for the container (host -> container)
sed "s|$ELEV_ROOT|/data|g" "$ELEV_ROOT/warped_file_list.txt" > "$ELEV_ROOT/warped_file_list_docker.txt"

# 3. Build the VRT in the container, which records only /data paths
docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
    gdalbuildvrt -input_file_list /data/warped_file_list_docker.txt /data/index.vrt

# 4. Rewrite the paths inside the VRT back to host paths (container -> host),
#    so raster2pgsql running on the host can find the actual files
sed "s|/data|$ELEV_ROOT|g" "$ELEV_ROOT/index.vrt" > "$ELEV_ROOT/index_local.vrt"

echo "📥 Ingesting raster VRT..."
# 5. Ingest using the host-side tool
raster2pgsql -d -s 3857 -I -C -M -t 256x256 "$ELEV_ROOT/index_local.vrt" public.esa_elevation_raster \
    | docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE"

# 6. Clean up temporary artifacts
rm -f "$ELEV_ROOT/warped_file_list.txt" \
      "$ELEV_ROOT/warped_file_list_docker.txt" \
      "$ELEV_ROOT/index.vrt" \
      "$ELEV_ROOT/index_local.vrt"

# ---------------------------------------------------------
# Step 5: Finalization
# ---------------------------------------------------------
echo "🚀 Finalizing..."
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
ANALYZE public.esa_elevation_raster;
UPDATE esa_elevation_catalog 
SET status = 'COMPLETED', 
    total_count = (SELECT count(*) FROM public.esa_elevation_raster), 
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

echo "✨ Elevation integration completed successfully!"