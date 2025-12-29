#!/bin/bash

set -e
set -o pipefail

# 1. 경로 자동 계산
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OSM_ROOT="$(dirname "$SCRIPT_DIR")"

# .env 파일 로드
if [ -f "$OSM_ROOT/../../.env" ]; then
  export $(echo $(grep -v '^#' "$OSM_ROOT/../../.env" | xargs) | envsubst)
elif [ -f "$OSM_ROOT/../.env" ]; then
  export $(echo $(grep -v '^#' "$OSM_ROOT/../.env" | xargs) | envsubst)
fi

export PGPASSWORD="${GEO_DB_PASSWORD}"

# [설정] 환경 변수 및 DB 정보
DB_CONTAINER="${GEO_DB_CONTAINER}"
DB_NETWORK="${DOCKER_NETWORK:-pixelet-network}"
PGUSER="${GEO_DB_USER:-admin}"
PGDATABASE="${GEO_DB_NAME:-geo}"
PGPORT="${GEO_DB_PORT:-5433}"
DB_HOST="${GEO_DB_HOST:-localhost}"

COUNTRY_CODE="${COUNTRY_CODE:-KR}"
COUNTRY_DIR="countries/${COUNTRY_CODE}"
PBF_RELATIVE_PATH="${COUNTRY_DIR}/raw/latest.osm.pbf"

echo "💧 Starting pixelet geo-data integration..."

# Step 2: Fresh Start
echo "🧹 Initializing osm_elements..."
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c "DROP TABLE IF EXISTS osm_elements, osm_data_catalog CASCADE;"

# Step 3: Schema Init
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" < "$OSM_ROOT/init/01_osm_schema.sql"

# Step 4: Raw Ingestion
osm2pgsql -U "$PGUSER" -d "$PGDATABASE" -H "$DB_HOST" -P "$PGPORT" \
  --slim --cache 2000 --hstore-all --extra-attributes \
  --multi-geometry \
  --number-processes 4 \
  --style "$OSM_ROOT/default.style" \
  "$OSM_ROOT/$PBF_RELATIVE_PATH"

docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"UPDATE osm_data_catalog SET status = 'PROCESSING' WHERE status = 'READY' AND country_code = '$COUNTRY_CODE';"

# Step 5: Migration (DISTINCT ON을 통한 단일 트랜잭션 내 ID 충돌 방지)
# 소스 데이터셋 내 중복 ID가 존재할 경우 ON CONFLICT UPDATE가 실패하므로 쿼리 단계에서 그룹화 수행
echo "Migrating all raw tables to osm_elements (Sequential Merge with ID Grouping)..."
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
-- 1. planet_osm_roads: 지형 뼈대 우선 주입
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_roads 
ORDER BY osm_id 
ON CONFLICT (osm_id) DO NOTHING;

-- 2. planet_osm_polygon: 면 데이터 주입 및 기존 태그 병합
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_polygon 
ORDER BY osm_id 
ON CONFLICT (osm_id) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 3. planet_osm_line: 선 데이터 주입 및 기존 태그 병합
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_line 
ORDER BY osm_id 
ON CONFLICT (osm_id) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;

-- 4. planet_osm_point: 점 데이터 주입 및 최종 태그 속성 보강
INSERT INTO osm_elements (osm_id, tags, way)
SELECT DISTINCT ON (osm_id) osm_id, tags, way FROM planet_osm_point 
ORDER BY osm_id 
ON CONFLICT (osm_id) DO UPDATE SET tags = osm_elements.tags || EXCLUDED.tags;
EOF

# Step 6: Cleanup
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
ANALYZE osm_elements;
CLUSTER osm_elements USING idx_osm_elements_way;
DROP TABLE IF EXISTS planet_osm_point, planet_osm_line, planet_osm_polygon, planet_osm_roads, planet_osm_nodes, planet_osm_rels, planet_osm_ways, osm2pgsql_properties;

UPDATE osm_data_catalog SET status = 'COMPLETED', total_count = (SELECT count(*) FROM osm_elements)
WHERE status = 'PROCESSING' AND country_code = '$COUNTRY_CODE';
EOF

echo "✨ Integration completed successfully!"