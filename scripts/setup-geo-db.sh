#!/bin/bash

set -e
set -o pipefail

# .env 파일 로드
if [ -f .env ]; then
  export $(echo $(grep -v '^#' .env | xargs) | envsubst)
fi

# [설정] 운영 가이드 기준 변수
DB_CONTAINER="${GEO_DB_CONTAINER:-db-geo}"
DB_NETWORK="${DOCKER_NETWORK:-pixelet-network}"
PGUSER="${GEO_DB_USER:-admin}"
PGDATABASE="${GEO_DB_NAME:-geo}"
PGPASSWORD="${GEO_DB_PASSWORD}"
COUNTRY_CODE="${COUNTRY_CODE:-KR}"

# 파일 경로 설정 (호스트 기준)
OSM_DATA_DIR="$(pwd)/infra/osm-data"
PBF_RELATIVE_PATH="${COUNTRY_CODE}/latest.osm.pbf"
STYLE_PATH="/data/pixelet.style" # 컨테이너 내부 매핑 경로

echo "💧 Starting geo-data integration [Docker Native Mode]..."

# Step 2 & 3: Schema & Audit (docker exec 활용)
echo "🏗️  Initializing schema and audit..."
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
  INSERT INTO osm_data_catalog (country_code, data_date, file_name, status)
  VALUES ('$COUNTRY_CODE', CURRENT_DATE, '$PBF_RELATIVE_PATH', 'PROCESSING')
  ON CONFLICT DO NOTHING;
EOF

# Step 4: Raw Data Load (운영 가이드 방식: docker run)
echo "📥 Loading raw OSM data (osm2pgsql container)..."
docker run --rm \
  -v "$OSM_DATA_DIR":/data \
  --network="$DB_NETWORK" \
  openstreetmap/osm2pgsql:latest \
  osm2pgsql -U "$PGUSER" -d "$PGDATABASE" -H "$DB_CONTAINER" \
  --slim --cache 1000 \
  --style "$STYLE_PATH" \
  "/data/$PBF_RELATIVE_PATH"

# Step 5: Data Migration & Refinement (SRID: 3857)
echo "🚚 Migrating data to production tables..."

# 헬퍼 함수: SQL 실행용
function run_sql() {
  docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c "$1"
}

run_sql "INSERT INTO osm_buildings (id, osm_id, name, building_type, levels, geom)
SELECT osm_id, osm_id, name, building,
CASE WHEN \"building:levels\" ~ '^[0-9]+$' THEN \"building:levels\"::int ELSE 1 END,
way FROM planet_osm_polygon WHERE building IS NOT NULL ON CONFLICT (id) DO NOTHING;"

run_sql "INSERT INTO osm_roads (id, osm_id, name, highway_type, oneway, geom)
SELECT osm_id, osm_id, name, highway, CASE WHEN oneway = 'yes' THEN true ELSE false END,
way FROM planet_osm_line WHERE highway IS NOT NULL ON CONFLICT (id) DO NOTHING;"

run_sql "INSERT INTO osm_pois (id, osm_id, name, poi_type, geom)
SELECT osm_id, osm_id, name, COALESCE(amenity, shop, tourism, leisure),
way FROM planet_osm_point WHERE (amenity IS NOT NULL OR shop IS NOT NULL OR tourism IS NOT NULL OR leisure IS NOT NULL)
AND name IS NOT NULL ON CONFLICT (id) DO NOTHING;"

# Step 6: Post-Processing & Cleanup
echo "🚀 Optimizing and cleaning up..."
run_sql "VACUUM ANALYZE osm_buildings, osm_roads, osm_pois;"
run_sql "DROP TABLE IF EXISTS planet_osm_point, planet_osm_line, planet_osm_polygon, planet_osm_roads, osm2pgsql_properties;"
run_sql "UPDATE osm_data_catalog SET status = 'COMPLETED', updated_at = CURRENT_TIMESTAMP WHERE status = 'PROCESSING' AND country_code = '$COUNTRY_CODE';"

echo "✨ All processes completed successfully!"