#!/bin/bash

# OSM's land polygons into db-geo whole, worldwide: the coast is every
# shard's, so none of it is taken in part. The download is kept under cache/
# and fetched again only when osmdata publishes a newer one.

set -e
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(cd "$ROOT/../../.." && pwd)"
DATA_DIR="$REPO_ROOT/cache/coastline"

DOTENV_PATH="$REPO_ROOT/.env"
if [ ! -f "$DOTENV_PATH" ]; then
    echo "❌ Error: .env file not found at $DOTENV_PATH"
    exit 1
fi
export $(echo $(grep -v '^#' "$DOTENV_PATH" | xargs) | envsubst)

URL=$(jq -r '.provider.url' "$ROOT/metadata.json")
TABLE=$(jq -r '.table' "$ROOT/metadata.json")
ZIP="$DATA_DIR/$(basename "$URL")"
mkdir -p "$DATA_DIR"

echo "🌊 $URL"
curl -fL -z "$ZIP" -o "$ZIP" "$URL"
SHP=$(unzip -Z1 "$ZIP" | grep '\.shp$')
unzip -o -q "$ZIP" -d "$DATA_DIR"

ogr2ogr -f PostgreSQL "PG:$DB_GEO_URL" "$DATA_DIR/$SHP" \
    -overwrite -nln "$TABLE" -nlt PROMOTE_TO_MULTI -lco GEOMETRY_NAME=geom -lco SPATIAL_INDEX=GIST -a_srs EPSG:3857
psql "$DB_GEO_URL" -v ON_ERROR_STOP=1 -c "ANALYZE $TABLE;" > /dev/null
echo "✨ $TABLE: $(psql "$DB_GEO_URL" -Atc "SELECT count(*) FROM $TABLE") polygons, $(date -r "$ZIP" +%Y-%m-%d)"
