#!/bin/bash

set -e
set -o pipefail

# 기본 경로 및 환경 설정
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OSM_ROOT="$(dirname "$SCRIPT_DIR")"

# .env 로드
DOTENV_PATH="$(cd "$OSM_ROOT/../../../" && pwd)/.env"

if [ -f "$DOTENV_PATH" ]; then
    echo "✅ Loading environment variables from $DOTENV_PATH"
    export $(echo $(grep -v '^#' "$DOTENV_PATH" | xargs) | envsubst)
else
    echo "❌ Error: .env file not found at $DOTENV_PATH"
    exit 1
fi

# 변수 가드
if [ -z "$GEO_DB_CONTAINER" ]; then
    echo "❌ Error: GEO_DB_CONTAINER is not set. Check your .env file."
    exit 1
fi

export PGPASSWORD="${GEO_DB_PASSWORD}"

# [설정] DB 연결 정보
DB_CONTAINER="${GEO_DB_CONTAINER}"
PGUSER="${GEO_DB_USER:-admin}"
PGDATABASE="${GEO_DB_NAME:-geo}"
PGPORT="${GEO_DB_PORT:-5433}"
DB_HOST="${GEO_DB_HOST:-localhost}"

# OSM 설정
OSM_CACHE_SIZE=2000
OSM_STYLE_NAME=default.style
METADATA_FILE="$OSM_ROOT/metadata.json"

echo "💧 Starting osm-features integration (SSOT Strategy: Hstore Only)"

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
# HSTORE 확장 필수 확인
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c "CREATE EXTENSION IF NOT EXISTS hstore;"

# 테이블 초기화
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

        # 다운로드 로직
        if [ ! -f "$TARGET_PBF" ]; then
            if [ -n "$DOWNLOAD_URL" ]; then
                echo "⬇️  Downloading from $DOWNLOAD_URL..."
                curl -L -o "$TARGET_PBF" "$DOWNLOAD_URL"
            else
                echo "❌ Error: File missing ($TARGET_PBF) and cannot auto-download."
                exit 1
            fi
        fi

        # 카탈로그 등록
        docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
        "INSERT INTO osm_features_catalog (region_code, data_date, file_name, status) 
         VALUES ('$REGION_CODE', '$DATA_DATE', '$FILE_NAME', 'PROCESSING')
         ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

        APPEND_FLAG=""
        if [ "$IS_FIRST_RUN" = false ]; then
            APPEND_FLAG="--append"
        fi

        STYLE_PATH="$OSM_ROOT/${OSM_STYLE_NAME:-default.style}"
        
        # 🎯 핵심: --hstore-all 옵션이 있어 tags 컬럼에 모든 데이터가 들어감
        osm2pgsql -U "$PGUSER" -d "$PGDATABASE" -H "$DB_HOST" -P "$PGPORT" \
          --slim --cache ${OSM_CACHE:-2000} \
          --hstore-all --extra-attributes \
          --multi-geometry --number-processes 4 \
          $APPEND_FLAG --style "$STYLE_PATH" "$TARGET_PBF"

        IS_FIRST_RUN=false
    done
done

# ---------------------------------------------------------
# Step 4: Migration (Hstore Only Strategy)
# ---------------------------------------------------------
if [ "$IS_FIRST_RUN" = true ]; then
    echo "⚠️  No files were processed. Exiting."
    exit 0
fi

echo "🚀 Migrating data to Single Source of Truth (osm_elements)..."

MIGRATION_SQL="$OSM_ROOT/temp_migration.sql"
cat <<EOF > "$MIGRATION_SQL"
BEGIN;

-- 1. Point Data (tags 컬럼만 가져옴 - hstore-all 덕분에 데이터 완벽함)
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

-- 3. Polygon Data (Relation ID 등 포함)
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

-- 데이터가 들어온 후 정렬해야 물리적 배치가 최적화됩니다.
-- 시간이 좀 걸릴 수 있으므로 로그를 남기거나 필요 시 주석 처리
CLUSTER osm_elements USING idx_osm_elements_way;

-- 임시 테이블 삭제 (용량 확보)
DROP TABLE IF EXISTS planet_osm_point, planet_osm_line, planet_osm_polygon, planet_osm_roads, planet_osm_nodes, planet_osm_rels, planet_osm_ways;

-- 상태 업데이트
UPDATE osm_features_catalog 
SET status = 'COMPLETED', 
    total_count = (SELECT count(*) FROM osm_elements),
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

# SQL 파일 실행
docker cp "$MIGRATION_SQL" "$DB_CONTAINER":/tmp/migration.sql
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -f /tmp/migration.sql

# 임시 파일 삭제
rm -f "$MIGRATION_SQL"
echo "✨ OSM features integration completed! (SSOT: osm_elements, General: osm_roads)"