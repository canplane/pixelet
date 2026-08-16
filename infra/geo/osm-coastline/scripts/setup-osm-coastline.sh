#!/bin/bash

set -e
set -o pipefail

# Base paths and environment
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COASTLINE_ROOT="$(dirname "$SCRIPT_DIR")"

# Load .env — the repo-root one, same as the sibling pipelines
DOTENV_PATH="$(cd "$COASTLINE_ROOT/../../../" && pwd)/.env"

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

# Metadata file, relative to this pipeline's root
METADATA_FILE="$COASTLINE_ROOT/metadata.json"

echo "🌊 Starting osm-coastline integration (Driven by metadata.json)"

# ---------------------------------------------------------
# Step 1: Metadata Parsing & Preparation
# ---------------------------------------------------------
if [ ! -f "$METADATA_FILE" ]; then
  echo "❌ Error: Metadata file not found at $METADATA_FILE"
  exit 1
fi

# Count metadata entries
ENTRY_COUNT=$(jq '. | length' "$METADATA_FILE")

if [ "$ENTRY_COUNT" -eq 0 ]; then
  echo "❌ Error: No entries found in metadata.json"
  exit 1
fi

echo "🔍 Found $ENTRY_COUNT region(s) in metadata.json"

# ---------------------------------------------------------
# Step 2: Schema Initialization (Symmetry with Features)
# ---------------------------------------------------------
echo "🧹 Initializing Database Schema for OSM Coastline..."

# 1. Drop existing tables (clean start)
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"DROP TABLE IF EXISTS osm_coastline_polygons, osm_coastline_catalog CASCADE;"

# 2. Apply the schema
SCHEMA_FILE="$COASTLINE_ROOT/init/01_osm_coastline_schema.sql"
if [ -f "$SCHEMA_FILE" ]; then
    docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" < "$SCHEMA_FILE"
else
    echo "❌ Error: Schema file not found at $SCHEMA_FILE."
    exit 1
fi

# ---------------------------------------------------------
# Step 3: Loop Processing (Iterate Regions -> Files)
# ---------------------------------------------------------
IS_FIRST_RUN=true

# Outer Loop: Regions defined in metadata
for ((i=0; i<ENTRY_COUNT; i++)); do
    # Parse the entry at the current index
    ENTRY=$(jq -r ".[$i]" "$METADATA_FILE")
    
    REGION_CODE=$(echo "$ENTRY" | jq -r '.region_code')
    DATA_DATE=$(echo "$ENTRY" | jq -r '.data_date // empty')
    if [ -z "$DATA_DATE" ]; then DATA_DATE=$(date +%Y-%m-%d); fi
    
    # Parse this region's file list
    FILES=()
    while IFS= read -r line; do
        FILES+=("$line")
    done < <(echo "$ENTRY" | jq -r '.files[]')

    echo "==================================================="
    echo "📍 Processing Region: $REGION_CODE"
    echo "==================================================="

    # Inner Loop: Files in current region
    for REL_FILE in "${FILES[@]}"; do
        # Paths in the JSON are relative to COASTLINE_ROOT
        TARGET_FILE="$COASTLINE_ROOT/$REL_FILE"
        FILE_NAME=$(basename "$TARGET_FILE")
        
        # Only the .shp of a shapefile set is processed
        if [[ "$FILE_NAME" != *.shp ]]; then
            # skip .dbf, .shx and friends — ogr2ogr picks them up on its own
            continue
        fi

        echo "---------------------------------------------------"
        echo "📂 Processing Main Shapefile: $FILE_NAME"
        echo "---------------------------------------------------"

        # No auto-download here: the artifact is a zip that has to be extracted
        # into this exact layout first, and provider.url is a landing page.
        if [ ! -f "$TARGET_FILE" ]; then
            echo "❌ Missing shapefile: $TARGET_FILE"
            echo "   Download and extract it first:"
            echo "     curl -LO https://osmdata.openstreetmap.de/download/land-polygons-split-3857.zip"
            echo "     unzip land-polygons-split-3857.zip"
            echo "   The .shx/.dbf/.prj/.cpg sidecars must end up beside the .shp."
            echo "   See infra/geo/README.md > 'Obtaining the raw data'."
            exit 1
        fi

        echo "📝 Registering catalog status (PROCESSING)..."
        docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
        "INSERT INTO osm_coastline_catalog (region_code, data_date, file_name, status) 
         VALUES ('$REGION_CODE', '$DATA_DATE', '$FILE_NAME', 'PROCESSING')
         ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

        # [Raw Ingestion via ogr2ogr]
        echo "🚀 Ingesting Shapefile via ogr2ogr..."
        
        ogr2ogr -f "PostgreSQL" PG:"host=$DB_HOST port=$PGPORT user=$PGUSER dbname=$PGDATABASE password=$PGPASSWORD" \
            "$TARGET_FILE" \
            -nln osm_coastline_polygons \
            -nlt MULTIPOLYGON \
            -t_srs EPSG:3857 \
            -update -append \
            --config PG_USE_COPY YES \
            -progress

        if [ $? -ne 0 ]; then
            echo "❌ Error: ogr2ogr failed for $FILE_NAME"
            exit 1
        fi
    done
done

# ---------------------------------------------------------
# Step 4: Validation & Optimization (Using Temp SQL File)
# ---------------------------------------------------------
echo "🔧 Validating Geometries & Optimizing..."

# Written to a file and executed, rather than passed inline, so the shell never reinterprets the SQL
TEMP_SQL="$COASTLINE_ROOT/temp_optimization.sql"

cat <<EOF > "$TEMP_SQL"
-- 1. Validate and repair (self-intersections and similar)
-- MakeValid is mandatory here: invalid coastline geometry breaks rendering outright
UPDATE osm_coastline_polygons
SET geom = ST_MakeValid(geom)
WHERE NOT ST_IsValid(geom);

-- 2. Refresh statistics
ANALYZE osm_coastline_polygons;

-- 3. Cluster — physically order rows by spatial adjacency
CLUSTER osm_coastline_polygons USING idx_osm_coastline_geom;

-- 4. Mark the catalog entry complete
UPDATE osm_coastline_catalog 
SET status = 'COMPLETED', 
    total_count = (SELECT count(*) FROM osm_coastline_polygons),
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

# Copy the SQL into the container and run it
docker cp "$TEMP_SQL" "$DB_CONTAINER":/tmp/optimization.sql
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -f /tmp/optimization.sql

# Remove the temporary file
rm -f "$TEMP_SQL"

echo "✨ OSM coastline integration completed successfully!"