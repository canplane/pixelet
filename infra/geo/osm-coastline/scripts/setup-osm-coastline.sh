#!/bin/bash

set -e
set -o pipefail

# 기본 경로 및 환경 설정
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COASTLINE_ROOT="$(dirname "$SCRIPT_DIR")"

# .env 로드 (대칭성 유지: Root .env 참조)
DOTENV_PATH="$(cd "$COASTLINE_ROOT/../../../" && pwd)/.env"

if [ -f "$DOTENV_PATH" ]; then
    echo "✅ Loading environment variables from $DOTENV_PATH"
    export $(echo $(grep -v '^#' "$DOTENV_PATH" | xargs) | envsubst)
else
    echo "❌ Error: .env file not found at $DOTENV_PATH"
    exit 1
fi

# 변수 가드
if [ -z "$DB_GEO_CONTAINER" ]; then
    echo "❌ Error: DB_GEO_CONTAINER is not set. Check your .env file."
    exit 1
fi

export PGPASSWORD="${DB_GEO_PASSWORD}"

# [설정] DB 연결 정보
DB_CONTAINER="${DB_GEO_CONTAINER}"
PGUSER="${DB_GEO_USER:-admin}"
PGDATABASE="${DB_GEO_NAME:-geo}"
PGPORT="${DB_GEO_PORT:-5433}"
DB_HOST="${DB_GEO_HOST:-localhost}"

# 메타데이터 파일 위치 (Root 기준)
METADATA_FILE="$COASTLINE_ROOT/metadata.json"

echo "🌊 Starting osm-coastline integration (Driven by metadata.json)"

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
# Step 2: Schema Initialization (Symmetry with Features)
# ---------------------------------------------------------
echo "🧹 Initializing Database Schema for OSM Coastline..."

# 1. 테이블 초기화 (Clean Start)
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"DROP TABLE IF EXISTS osm_coastline_polygons, osm_coastline_catalog CASCADE;"

# 2. 스키마 주입
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
    # 현재 인덱스의 객체 파싱
    ENTRY=$(jq -r ".[$i]" "$METADATA_FILE")
    
    REGION_CODE=$(echo "$ENTRY" | jq -r '.region_code')
    DATA_DATE=$(echo "$ENTRY" | jq -r '.data_date // empty')
    if [ -z "$DATA_DATE" ]; then DATA_DATE=$(date +%Y-%m-%d); fi
    
    # 해당 리전의 파일 목록 파싱
    FILES=()
    while IFS= read -r line; do
        FILES+=("$line")
    done < <(echo "$ENTRY" | jq -r '.files[]')

    echo "==================================================="
    echo "📍 Processing Region: $REGION_CODE"
    echo "==================================================="

    # Inner Loop: Files in current region
    for REL_FILE in "${FILES[@]}"; do
        # JSON 경로는 COASTLINE_ROOT 기준
        TARGET_FILE="$COASTLINE_ROOT/$REL_FILE"
        FILE_NAME=$(basename "$TARGET_FILE")
        
        # [중요] Shapefile 세트 중 .shp 파일만 처리 대상임
        if [[ "$FILE_NAME" != *.shp ]]; then
            # .dbf, .shx 등은 건너뜀 (ogr2ogr이 알아서 참조함)
            continue
        fi

        echo "---------------------------------------------------"
        echo "📂 Processing Main Shapefile: $FILE_NAME"
        echo "---------------------------------------------------"

        if [ ! -f "$TARGET_FILE" ]; then
            echo "❌ Error: Main shapefile missing at $TARGET_FILE"
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

# [수정됨] 쉘 해석 오류 방지를 위해 SQL 파일 생성 후 실행 방식 적용
TEMP_SQL="$COASTLINE_ROOT/temp_optimization.sql"

cat <<EOF > "$TEMP_SQL"
-- 1. 유효성 검사 및 보정 (Self-intersection 등 수정)
-- 해안선 데이터는 렌더링 시 오류가 치명적이므로 MakeValid 필수
UPDATE osm_coastline_polygons
SET geom = ST_MakeValid(geom)
WHERE NOT ST_IsValid(geom);

-- 2. 통계 갱신
ANALYZE osm_coastline_polygons;

-- 3. 클러스터링 (공간적 인접성 기반 물리 정렬)
CLUSTER osm_coastline_polygons USING idx_osm_coastline_geom;

-- 4. 카탈로그 완료 처리
UPDATE osm_coastline_catalog 
SET status = 'COMPLETED', 
    total_count = (SELECT count(*) FROM osm_coastline_polygons),
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

# SQL 파일 컨테이너로 복사 및 실행
docker cp "$TEMP_SQL" "$DB_CONTAINER":/tmp/optimization.sql
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -f /tmp/optimization.sql

# 임시 파일 삭제
rm -f "$TEMP_SQL"

echo "✨ OSM coastline integration completed successfully!"