#!/bin/bash

set -e
set -o pipefail

# 기본 경로 및 환경 설정
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELEV_ROOT="$(dirname "$SCRIPT_DIR")"

# .env 로드
DOTENV_PATH="$(cd "$ELEV_ROOT/../../../" && pwd)/.env"

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

# 메타데이터 파일 위치
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
        
        # 파일명 변환 로직 (4326 -> 3857)
        if [[ "$FILENAME" == *"4326"* ]]; then
            WARPED_FILENAME="${FILENAME//4326/3857}"
        else
            WARPED_FILENAME="${FILENAME%.*}_3857.tif"
        fi
        WARPED_FILE="$WARPED_DIR/$WARPED_FILENAME"
        
        echo "📂 Processing File: $FILENAME"
        mkdir -p "$(dirname "$RAW_FILE")"
        mkdir -p "$WARPED_DIR"

        if [ ! -f "$RAW_FILE" ]; then
            if [ -n "$DOWNLOAD_URL" ]; then
                echo "⬇️  Downloading from $DOWNLOAD_URL..."
                curl -L -o "$RAW_FILE" "$DOWNLOAD_URL"
            else
                echo "❌ Error: File missing ($RAW_FILE)"
                exit 1
            fi
        fi

        # 상태 등록
        docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
        "INSERT INTO esa_elevation_catalog (region_code, data_date, file_name, status) 
         VALUES ('$REGION_CODE', '$DATA_DATE', '$FILENAME', 'PROCESSING')
         ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

        # Warping 실행 (Docker)
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

# 1. 로컬 파일 리스트 생성
find "$ELEV_ROOT" -type f -path "*/warped/*_3857.tif" > "$ELEV_ROOT/warped_file_list.txt"

# 2. Docker가 인식할 수 있도록 경로 치환 (Host -> Container)
sed "s|$ELEV_ROOT|/data|g" "$ELEV_ROOT/warped_file_list.txt" > "$ELEV_ROOT/warped_file_list_docker.txt"

# 3. Docker로 VRT 생성 (Docker는 /data 경로만 기록함)
docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
    gdalbuildvrt -input_file_list /data/warped_file_list_docker.txt /data/index.vrt

# 4. 생성된 VRT 내부의 경로를 다시 로컬 경로로 환원 (Container -> Host)
# raster2pgsql이 로컬 환경에서 실제 파일을 찾을 수 있게 함
sed "s|/data|$ELEV_ROOT|g" "$ELEV_ROOT/index.vrt" > "$ELEV_ROOT/index_local.vrt"

echo "📥 Ingesting raster VRT..."
# 5. 로컬 도구로 주입 실행
raster2pgsql -d -s 3857 -I -C -M -t 256x256 "$ELEV_ROOT/index_local.vrt" public.esa_elevation_raster \
    | docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE"

# 6. 임시 아티팩트 정리
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