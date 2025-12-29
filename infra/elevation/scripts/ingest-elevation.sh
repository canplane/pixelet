#!/bin/bash

set -e
set -o pipefail

# 1. 경로 자동 계산
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELEV_ROOT="$(dirname "$SCRIPT_DIR")"

# 2. .env 파일 로드
DOTENV_PATH="$(cd "$ELEV_ROOT/../../" && pwd)/.env"

if [ -f "$DOTENV_PATH" ]; then
    echo "✅ Loading environment variables from $DOTENV_PATH"
    export $(grep -v '^#' "$DOTENV_PATH" | xargs)
else
    echo "❌ Error: .env file not found at $DOTENV_PATH"
    exit 1
fi

# 변수가 정상적으로 로드되었는지 확인하는 가드(Guard)
if [ -z "$GEO_DB_CONTAINER" ]; then
    echo "❌ Error: GEO_DB_CONTAINER is not set. Check your .env file."
    exit 1
fi

export PGPASSWORD="${GEO_DB_PASSWORD}"

# [설정]
DB_CONTAINER="${GEO_DB_CONTAINER}"
PGUSER="${GEO_DB_USER:-admin}"
PGDATABASE="${GEO_DB_NAME:-geo}"
COUNTRY_CODE="${COUNTRY_CODE:-KR}"
COUNTRY_DIR="countries/${COUNTRY_CODE}"

mkdir -p "$ELEV_ROOT/$COUNTRY_DIR/warped"

# 🛠️ 수정 포인트: 사용할 도커 이미지 태그 (공식 이미지로 변경)
GDAL_IMAGE="ghcr.io/osgeo/gdal:ubuntu-small-latest"

echo "🏔️ Starting pixelet elevation-data integration for [$COUNTRY_CODE]..."

# Step 1: metadata.json 기반 변환
FILES=$(jq -r '.files[]' "$ELEV_ROOT/$COUNTRY_DIR/metadata.json")

for RAW_RELATIVE_PATH in $FILES; do
    FILENAME=$(basename "$RAW_RELATIVE_PATH")
    WARPED_FILE="${FILENAME//4326/3857}"
    
    echo "🔄 Re-projecting: $FILENAME -> $WARPED_FILE"
    
    # GDAL Warp 실행
    docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
        gdalwarp -s_srs EPSG:4326 -t_srs EPSG:3857 \
        -overwrite \
        -r bilinear -of GTiff -co COMPRESS=DEFLATE \
        "/data/$COUNTRY_DIR/$RAW_RELATIVE_PATH" \
        "/data/$COUNTRY_DIR/warped/$WARPED_FILE"
done

# Step 2: Virtual Dataset 생성
echo "📦 Building VRT index..."
docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
    sh -c "gdalbuildvrt /data/$COUNTRY_DIR/index.vrt /data/$COUNTRY_DIR/warped/*.tif"

# Step 3: DB Clean start & Extension Check
echo "🧹 Initializing database environment..."

# 래스터 타입 사용을 위해 postgis_raster 확장이 반드시 필요함
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"CREATE EXTENSION IF NOT EXISTS postgis_raster;"

# 기존 테이블 소거
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"DROP TABLE IF EXISTS public.srtm_elevation_3857 CASCADE;"

# Step 4: PostGIS Raster Ingestion
echo "📥 Ingesting raster tiles to DB..."
raster2pgsql -s 3857 -I -C -M -t 256x256 "$ELEV_ROOT/$COUNTRY_DIR/index.vrt" public.srtm_elevation_3857 \
    | docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE"

# Step 5: Optimization
echo "🚀 Optimizing raster table..."
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"ANALYZE public.srtm_elevation_3857;"

echo "✨ Elevation integration completed successfully!"