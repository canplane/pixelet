#!/bin/bash

set -e
set -o pipefail

# Base paths and environment
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OSM_ROOT="$(dirname "$SCRIPT_DIR")"

# Load .env
DOTENV_PATH="$(cd "$OSM_ROOT/../../../" && pwd)/.env"

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

# OSM settings
OSM_CACHE_SIZE=2000
OSM_STYLE_NAME=default.style
METADATA_FILE="$OSM_ROOT/metadata.json"

echo "💧 Starting osm-features integration (hstore-only ingestion)"

# ---------------------------------------------------------
# Step 1: Metadata Parsing & Preparation
# ---------------------------------------------------------
if [ ! -f "$METADATA_FILE" ]; then
  echo "❌ Error: Metadata file not found at $METADATA_FILE"
  exit 1
fi

ENTRY_COUNT=$(jq '. | length' "$METADATA_FILE")
if [ "$ENTRY_COUNT" -eq 0 ]; then
  echo "❌ Error: No entries found in metadata.json"
  exit 1
fi

echo "🔍 Found $ENTRY_COUNT region(s) in metadata.json"

# ---------------------------------------------------------
# Step 2: Schema Initialization
# ---------------------------------------------------------
echo "🧹 Initializing Database Schema..."
# hstore extension is required
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c "CREATE EXTENSION IF NOT EXISTS hstore;"

# Drop existing tables
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"DROP TABLE IF EXISTS osm_elements, osm_roads, osm_features_catalog CASCADE;"

SCHEMA_FILE="$OSM_ROOT/init/01_osm_features_schema.sql"
if [ -f "$SCHEMA_FILE" ]; then
    docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" < "$SCHEMA_FILE"
else
    echo "❌ Error: Schema file ($SCHEMA_FILE) not found."
    exit 1
fi

# ---------------------------------------------------------
# Step 3: Loop Processing (osm2pgsql)
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
        TARGET_PBF="$OSM_ROOT/$REL_FILE"
        FILE_NAME=$(basename "$TARGET_PBF")
        mkdir -p "$(dirname "$TARGET_PBF")"

        # Download if absent. Guarded the same way as the elevation pipeline:
        # only fetch when the URL points at a file, so a provider landing page
        # can never be saved under a .pbf name. `-f` makes curl fail on an HTTP
        # error instead of writing the error body to the target.
        if [ ! -f "$TARGET_PBF" ]; then
            case "$DOWNLOAD_URL" in
                *.pbf)
                    echo "⬇️  Downloading from $DOWNLOAD_URL..."
                    curl -fL -o "$TARGET_PBF" "$DOWNLOAD_URL"
                    ;;
                *)
                    echo "❌ Missing raw file: $TARGET_PBF"
                    echo "   provider.url does not point at a .pbf, so it cannot be fetched automatically."
                    echo "   See infra/geo/README.md > 'Obtaining the raw data'."
                    exit 1
                    ;;
            esac
        fi

        # Register in the catalog
        docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
        "INSERT INTO osm_features_catalog (region_code, data_date, file_name, status) 
         VALUES ('$REGION_CODE', '$DATA_DATE', '$FILE_NAME', 'PROCESSING')
         ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

        APPEND_FLAG=""
        if [ "$IS_FIRST_RUN" = false ]; then
            APPEND_FLAG="--append"
        fi

        STYLE_PATH="$OSM_ROOT/${OSM_STYLE_NAME:-default.style}"
        
        # --hstore-all is the key flag: it puts every tag into the tags column
        osm2pgsql -U "$PGUSER" -d "$PGDATABASE" -H "$DB_HOST" -P "$PGPORT" \
          --slim --cache ${OSM_CACHE:-2000} \
          --hstore-all --extra-attributes \
          --multi-geometry --number-processes 4 \
          $APPEND_FLAG --style "$STYLE_PATH" "$TARGET_PBF"

        IS_FIRST_RUN=false
    done
done

# ---------------------------------------------------------
# Step 4: Migration (hstore Only Strategy)
# ---------------------------------------------------------
if [ "$IS_FIRST_RUN" = true ]; then
    echo "⚠️  No files were processed. Exiting."
    exit 0
fi

echo "🚀 Migrating data into primary store (osm_elements)..."

MIGRATION_SQL="$OSM_ROOT/temp_migration.sql"
cat <<EOF > "$MIGRATION_SQL"
BEGIN;

-- 1. Point data — only the tags column is needed, since --hstore-all captured everything
INSERT INTO osm_elements (osm_id, type, tags, way) 
SELECT DISTINCT ON (osm_id) 
    osm_id, 'point', tags, way 
FROM planet_osm_point
ON CONFLICT (osm_id, type) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 2. Line Data
INSERT INTO osm_elements (osm_id, type, tags, way) 
SELECT DISTINCT ON (osm_id) 
    osm_id, 'line', tags, way 
FROM planet_osm_line
ON CONFLICT (osm_id, type) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 3. Polygon data (includes relation IDs)
INSERT INTO osm_elements (osm_id, type, tags, way) 
SELECT DISTINCT ON (osm_id) 
    osm_id, 'polygon', tags, way 
FROM planet_osm_polygon
ON CONFLICT (osm_id, type) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 4. Generalized Roads
INSERT INTO osm_roads (osm_id, tags, way) 
SELECT DISTINCT ON (osm_id) osm_id, tags, way 
FROM planet_osm_roads 
ORDER BY osm_id 
ON CONFLICT (osm_id) DO NOTHING;

COMMIT;

-- 5. Cleanup & Analyze
ANALYZE osm_elements;
ANALYZE osm_roads;

-- Clustering only pays off once the data is in, so it runs last.
-- It can take a while; comment it out if that is a problem.
CLUSTER osm_elements USING idx_osm_elements_way;

-- Drop the staging tables to reclaim space
DROP TABLE IF EXISTS planet_osm_point, planet_osm_line, planet_osm_polygon, planet_osm_roads, planet_osm_nodes, planet_osm_rels, planet_osm_ways;

-- Update status
UPDATE osm_features_catalog 
SET status = 'COMPLETED', 
    total_count = (SELECT count(*) FROM osm_elements),
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

# Run the SQL file
docker cp "$MIGRATION_SQL" "$DB_CONTAINER":/tmp/migration.sql
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -f /tmp/migration.sql

# Remove the temporary file
rm -f "$MIGRATION_SQL"
echo "✨ OSM features integration completed! (primary: osm_elements, generalized: osm_roads)"