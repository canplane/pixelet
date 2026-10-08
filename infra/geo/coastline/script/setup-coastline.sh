#!/bin/bash

# OSM's land polygons into db-geo whole, worldwide: the coast is every
# shard's, so none of it is taken in part. The download is kept in raw/
# and fetched again only when osmdata publishes a newer one. The table's
# identity is the download's hash, kept as its comment, which a bake's
# holdings read (`pxl-geo-bake coverage.rs`): the same download is not
# imported again, and a new one is filled beside the table and swapped in,
# so readers never see it emptied.

set -e
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(cd "$ROOT/../../.." && pwd)"
DATA_DIR="$ROOT/raw"

DOTENV_PATH="$REPO_ROOT/.env"
if [ ! -f "$DOTENV_PATH" ]; then
    echo "❌ Error: .env file not found at $DOTENV_PATH"
    exit 1
fi
export $(echo $(grep -v '^#' "$DOTENV_PATH" | xargs) | envsubst)

psql_geo() { PGPASSWORD="$DB_GEO_PASSWORD" psql -h "$DB_GEO_HOST" -p "$DB_GEO_PORT" -U "$DB_GEO_USER" -d "$DB_GEO_NAME" -v ON_ERROR_STOP=1 "$@"; }

URL=$(jq -r '.provider.url' "$ROOT/metadata.json")
TABLE=$(jq -r '.table' "$ROOT/metadata.json")
ZIP="$DATA_DIR/$(basename "$URL")"
mkdir -p "$DATA_DIR"

echo "🌊 $URL"
curl -fL -z "$ZIP" -o "$ZIP" "$URL"
SHP=$(unzip -Z1 "$ZIP" | grep '\.shp$')
unzip -o -q "$ZIP" -d "$DATA_DIR"

ID=$(shasum -a 256 "$ZIP" | cut -c1-16)
if [ "$(psql_geo -Atc "SELECT obj_description(to_regclass('$TABLE'), 'pg_class')")" = "$ID" ]; then
    echo "✨ $TABLE already holds $ZIP ($ID)"
    exit 0
fi

NEXT="${TABLE}_next"
ogr2ogr -f PostgreSQL "PG:host=$DB_GEO_HOST port=$DB_GEO_PORT user=$DB_GEO_USER dbname=$DB_GEO_NAME password=$DB_GEO_PASSWORD" "$DATA_DIR/$SHP" \
    -overwrite -nln "$NEXT" -nlt PROMOTE_TO_MULTI -lco GEOMETRY_NAME=geom -lco SPATIAL_INDEX=GIST -a_srs EPSG:3857
psql_geo -c "ANALYZE $NEXT;" > /dev/null
# One short transaction: the old table dropped, the new one and its indexes
# given its names, and its identity said.
psql_geo > /dev/null <<SQL
BEGIN;
SET LOCAL lock_timeout = '30s';
DROP TABLE IF EXISTS $TABLE;
ALTER TABLE $NEXT RENAME TO $TABLE;
DO \$\$ DECLARE i text; BEGIN
  FOR i IN SELECT indexrelid::regclass::text FROM pg_index WHERE indrelid = '$TABLE'::regclass LOOP
    IF i LIKE '${NEXT}%' THEN EXECUTE format('ALTER INDEX %I RENAME TO %I', i, '$TABLE' || substr(i, length('$NEXT') + 1)); END IF;
  END LOOP;
END \$\$;
COMMENT ON TABLE $TABLE IS '$ID';
COMMIT;
SQL
echo "✨ $TABLE: $(psql_geo -Atc "SELECT count(*) FROM $TABLE") polygons, $(date -r "$ZIP" +%Y-%m-%d)"
