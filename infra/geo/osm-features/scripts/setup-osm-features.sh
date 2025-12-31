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

# 메타데이터 파일 위치 (Root 기준)
METADATA_FILE="$OSM_ROOT/metadata.json"

echo "💧 Starting osm-features integration (Driven by metadata.json)"

# ---------------------------------------------------------
# Step 1: Metadata Parsing & Preparation
# ---------------------------------------------------------
if [ ! -f "$METADATA_FILE" ]; then
  echo "❌ Error: Metadata file not found at $METADATA_FILE"
  exit 1
fi

# 메타데이터 엔트리 개수 확인
ENTRY_COUNT=$(jq '. | length' "$METADATA_FILE")

if [ "$ENTRY_COUNT" -eq 0 ]; then
  echo "❌ Error: No entries found in metadata.json"
  exit 1
fi

echo "🔍 Found $ENTRY_COUNT region(s) in metadata.json"

# ---------------------------------------------------------
# Step 2: Schema Initialization (OSM Only)
# ---------------------------------------------------------
echo "🧹 Initializing Database Schema for OSM Features..."

# 1. (확장팩은 SQL 안에 포함되어 있으므로 생략 가능하나 대칭성을 위해 주석 유지)

# 2. 테이블 초기화
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"DROP TABLE IF EXISTS osm_elements, osm_roads, osm_features_catalog CASCADE;"

# 3. 스키마 주입
SCHEMA_FILE="$OSM_ROOT/init/01_osm_features_schema.sql"
if [ -f "$SCHEMA_FILE" ]; then
    docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" < "$SCHEMA_FILE"
else
    echo "⚠️  Warning: Schema file not found at $SCHEMA_FILE. Assuming manual table creation."
fi

# ---------------------------------------------------------
# Step 3: Loop Processing (Iterate Regions -> Files)
# ---------------------------------------------------------
IS_FIRST_RUN=true

# Outer Loop: Regions defined in metadata
for ((i=0; i<ENTRY_COUNT; i++)); do
    # 현재 인덱스의 객체 파싱
    ENTRY=$(jq -r ".[$i]" "$METADATA_FILE")
    
    REGION_CODE=$(echo "$ENTRY" | jq -r '.region_code')
    DOWNLOAD_URL=$(echo "$ENTRY" | jq -r '.provider.url // empty')
    DATA_DATE=$(echo "$ENTRY" | jq -r '.data_date // empty')
    
    if [ -z "$DATA_DATE" ]; then DATA_DATE=$(date +%Y-%m-%d); fi
    
    # 해당 리전의 파일 목록 파싱
    mapfile -t FILES < <(echo "$ENTRY" | jq -r '.files[]')

    echo "==================================================="
    echo "📍 Processing Region: $REGION_CODE"
    echo "==================================================="

    # Inner Loop: Files in current region
    for REL_FILE in "${FILES[@]}"; do
        # JSON 경로는 OSM_ROOT 기준
        TARGET_PBF="$OSM_ROOT/$REL_FILE"
        FILE_NAME=$(basename "$TARGET_PBF")

        echo "---------------------------------------------------"
        echo "📂 Processing File: $FILE_NAME"
        echo "---------------------------------------------------"

        mkdir -p "$(dirname "$TARGET_PBF")"

        # 파일 다운로드 체크
        if [ -f "$TARGET_PBF" ]; then
            echo "✅ File exists: $TARGET_PBF"
        else
            if [ -n "$DOWNLOAD_URL" ]; then
                echo "⬇️  Downloading from $DOWNLOAD_URL..."
                curl -L -o "$TARGET_PBF" "$DOWNLOAD_URL"
                if [ $? -ne 0 ]; then echo "❌ Download failed."; exit 1; fi
            else
                echo "❌ Error: File missing ($TARGET_PBF) and cannot auto-download."
                exit 1
            fi
        fi

        echo "📝 Registering catalog status (PROCESSING)..."
        docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
        "INSERT INTO osm_features_catalog (region_code, data_date, file_name, status) 
         VALUES ('$REGION_CODE', '$DATA_DATE', '$FILE_NAME', 'PROCESSING')
         ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

        # [Raw Ingestion]
        # 전체 실행 중 첫 번째 파일만 Create, 그 외 모든 리전의 모든 파일은 Append
        APPEND_FLAG=""
        if [ "$IS_FIRST_RUN" = false ]; then
            APPEND_FLAG="--append"
            echo "🔗 Appending to existing temporary tables..."
        else
            echo "🚀 Creating temporary tables (osm2pgsql)..."
        fi

        # OSM_STYLE_NAME이 없으면 default.style을 기본값으로 사용
        STYLE_PATH="$OSM_ROOT/${OSM_STYLE_NAME:-default.style}"

        osm2pgsql -U "$PGUSER" -d "$PGDATABASE" -H "$DB_HOST" -P "$PGPORT" \
          --slim --cache ${OSM_CACHE:-2000} --hstore-all --extra-attributes \
          --multi-geometry \
          --number-processes 4 \
          $APPEND_FLAG \
          --style "$STYLE_PATH" \
          "$TARGET_PBF"

        # 한 번이라도 실행되었으면 이후는 무조건 Append 모드
        IS_FIRST_RUN=false
    done
done

# ---------------------------------------------------------
# Step 4: Migration (Separating SSOT & Low-zoom Roads)
# ---------------------------------------------------------
if [ "$IS_FIRST_RUN" = true ]; then
    echo "⚠️  No files were processed. Exiting."
    exit 0
fi

echo "Migrating all raw tables to osm_elements (SSOT) and osm_roads (Generalized)..."

docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
-- 1. Main Data (SSOT: High-fidelity) with Type Discrimination
INSERT INTO osm_elements (osm_id, type, tags, way) SELECT osm_id, 'point', tags, way FROM planet_osm_point ON CONFLICT (osm_id, type) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;
INSERT INTO osm_elements (osm_id, type, tags, way) SELECT osm_id, 'line', tags, way FROM planet_osm_line ON CONFLICT (osm_id, type) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;
INSERT INTO osm_elements (osm_id, type, tags, way) SELECT osm_id, 'polygon', tags, way FROM planet_osm_polygon ON CONFLICT (osm_id, type) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 2. Generalized Data (osm_roads)
-- Derived directly from planet_osm_roads for low-zoom optimization
INSERT INTO osm_roads (osm_id, tags, way) 
SELECT DISTINCT ON (osm_id) osm_id, tags, way 
FROM planet_osm_roads 
ORDER BY osm_id 
ON CONFLICT (osm_id) DO NOTHING;
EOF

# ---------------------------------------------------------
# Step 5: Optimization & Cleanup
# ---------------------------------------------------------
echo "🚀 Optimizing & Finalizing..."

docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
ANALYZE osm_elements;
ANALYZE osm_roads;

CLUSTER osm_elements USING idx_osm_elements_way;
CLUSTER osm_roads USING idx_osm_roads_way;

-- 임시 테이블 삭제
DROP TABLE IF EXISTS planet_osm_point, planet_osm_line, planet_osm_polygon, planet_osm_roads, planet_osm_nodes, planet_osm_rels, planet_osm_ways, osm2pgsql_properties;

-- 처리된 모든 항목의 상태를 완료로 변경
UPDATE osm_features_catalog 
SET status = 'COMPLETED', 
    total_count = (SELECT count(*) FROM osm_elements),
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

echo "✨ Osm-features integration completed successfully!"