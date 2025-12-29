#!/bin/bash

# 1. 환경 설정 로드 (사용자님 스크립트 스타일 계승)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")" # pixelet 루트

if [ -f "$PROJECT_ROOT/.env" ]; then
  export $(grep -v '^#' "$PROJECT_ROOT/.env" | xargs)
fi

BACKUP_DIR="$PROJECT_ROOT/backups"
mkdir -p "$BACKUP_DIR"

FILENAME="geo_db_backup_$(date +%Y%m%d_%H%M).dump"

echo "💾 Starting PostGIS database backup..."

# 2. pg_dump 실행 (-Fc: 커스텀 압축 포맷)
docker exec -t "${GEO_DB_CONTAINER}" pg_dump \
  -U "${GEO_DB_USER}" \
  -d "${GEO_DB_NAME}" \
  -Fc > "$BACKUP_DIR/$FILENAME"

echo "✅ Backup completed: $BACKUP_DIR/$FILENAME"
echo "📏 Size: $(du -h "$BACKUP_DIR/$FILENAME" | cut -f1)"

# DB 초기화 후 복구
# docker exec -i "${GEO_DB_CONTAINER}" pg_restore \
#   -U "${GEO_DB_USER}" \
#   -d "${GEO_DB_NAME}" \
#   --clean --if-exists < ./backups/파일명.dump