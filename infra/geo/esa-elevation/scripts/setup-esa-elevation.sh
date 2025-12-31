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

# 메타데이터 파일 위치 (Root 기준)
METADATA_FILE="$ELEV_ROOT/metadata.json"

# GDAL 설정
GDAL_IMAGE="ghcr.io/osgeo/gdal:ubuntu-small-latest"

echo "🏔️  Starting elevation-data integration (Driven by metadata.json)"

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
# Step 2: Schema Initialization (Elevation Only)
# ---------------------------------------------------------
echo "🧹 Initializing Database Schema for Elevation..."

# 1. 확장팩 확인
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"CREATE EXTENSION IF NOT EXISTS postgis_raster;"

# 2. 테이블 초기화
docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
"DROP TABLE IF EXISTS public.esa_elevation_raster, public.esa_elevation_catalog CASCADE;"

# 3. 스키마 주입
SCHEMA_FILE="$ELEV_ROOT/init/01_esa_elevation_schema.sql"
if [ -f "$SCHEMA_FILE" ]; then
    docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" < "$SCHEMA_FILE"
else
    echo "⚠️  Warning: Schema file not found at $SCHEMA_FILE. Assuming manual table creation."
fi

# ---------------------------------------------------------
# Step 3: Loop Processing (Download & Warp)
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
        # JSON 경로는 ELEV_ROOT 기준 (ex: region/KR/raw/file.tif)
        RAW_FILE="$ELEV_ROOT/$REL_FILE"
        FILENAME=$(basename "$RAW_FILE")
        
        # Warped 파일 경로 생성 (raw -> warped 폴더로 변경)
        # 예: region/KR/raw/dem.tif -> region/KR/warped/dem_3857.tif
        WARPED_DIR="$(dirname "$RAW_FILE")/../warped"
        
        if [[ "$FILENAME" == *"4326"* ]]; then
            WARPED_FILENAME="${FILENAME//4326/3857}"
        else
            WARPED_FILENAME="${FILENAME%.*}_3857.tif"
        fi
        
        WARPED_FILE="$WARPED_DIR/$WARPED_FILENAME"
        
        echo "---------------------------------------------------"
        echo "📂 Processing File: $FILENAME"
        echo "---------------------------------------------------"

        mkdir -p "$(dirname "$RAW_FILE")"
        mkdir -p "$WARPED_DIR"

        # 파일 다운로드 체크
        if [ -f "$RAW_FILE" ]; then
            echo "✅ Raw file exists: $RAW_FILE"
        else
            if [ -n "$DOWNLOAD_URL" ]; then
                echo "⬇️  Downloading from $DOWNLOAD_URL..."
                curl -L -o "$RAW_FILE" "$DOWNLOAD_URL"
                if [ $? -ne 0 ]; then echo "❌ Download failed."; exit 1; fi
            else
                echo "❌ Error: File missing ($RAW_FILE) and cannot auto-download."
                exit 1
            fi
        fi

        echo "📝 Registering catalog status (PROCESSING)..."
        docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" -c \
        "INSERT INTO esa_elevation_catalog (region_code, data_date, file_name, status) 
         VALUES ('$REGION_CODE', '$DATA_DATE', '$FILENAME', 'PROCESSING')
         ON CONFLICT (file_name) DO UPDATE SET status = 'PROCESSING', updated_at = CURRENT_TIMESTAMP;"

        # [Warping & Processing]
        echo "🔄 Re-projecting to 3857 (Warping)..."
        # Docker 내 경로는 /data를 ELEV_ROOT로 매핑하여 사용
        docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
            gdalwarp -s_srs EPSG:4326 -t_srs EPSG:3857 \
            -overwrite -r bilinear -of GTiff -co COMPRESS=DEFLATE \
            "/data/$REL_FILE" "/data/${REL_FILE%/*}/../warped/$WARPED_FILENAME"
        
        IS_FIRST_RUN=false
    done
done

# ---------------------------------------------------------
# Step 4: VRT & Ingestion (Batch Processing)
# ---------------------------------------------------------
if [ "$IS_FIRST_RUN" = true ]; then
    echo "⚠️  No files were processed. Exiting."
    exit 0
fi

echo "📦 Building global VRT index from all warped files..."

# 처리된 모든 region의 warped 폴더에서 tif 파일을 찾음
find "$ELEV_ROOT" -type f -path "*/warped/*_3857.tif" > "$ELEV_ROOT/warped_file_list.txt"

# VRT 생성 (Docker 내부 경로로 변환 필요: ELEV_ROOT -> /data)
# sed를 사용하여 호스트 경로를 컨테이너 경로로 치환
sed -i "s|$ELEV_ROOT|/data|g" "$ELEV_ROOT/warped_file_list.txt"

docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
    gdalbuildvrt -input_file_list /data/warped_file_list.txt /data/index.vrt

echo "📥 Ingesting raster VRT to Database..."
# raster2pgsql Options:
# -d: Drop table and create (Step 2에서 생성한 빈 테이블을 덮어쓰기 위해 필수)
# -s: SRID (3857)
# -I: Create Index
# -C: Apply Constraints
# -M: Vacuum Analyze
# -t: Tile Size (256x256)
docker run --rm -v "$ELEV_ROOT":/data "$GDAL_IMAGE" \
    raster2pgsql -d -s 3857 -I -C -M -t 256x256 /data/index.vrt public.esa_elevation_raster \
    | docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE"

# 임시 파일 정리
rm -f "$ELEV_ROOT/warped_file_list.txt" "$ELEV_ROOT/index.vrt"

# ---------------------------------------------------------
# Step 5: Optimization & Cleanup
# ---------------------------------------------------------
echo "🚀 Optimizing & Finalizing..."

docker exec -i "$DB_CONTAINER" psql -U "$PGUSER" -d "$PGDATABASE" <<EOF
ANALYZE public.esa_elevation_raster;

-- 처리된 모든 항목의 상태를 완료로 변경
UPDATE esa_elevation_catalog 
SET status = 'COMPLETED', 
    total_count = (SELECT count(*) FROM public.esa_elevation_raster), 
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'PROCESSING';
EOF

echo "✨ Elevation integration completed successfully!"