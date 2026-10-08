#!/bin/bash

# OSM's land polygons into db-geo whole, worldwide: the coast is every
# shard's, so none of it is taken in part. The download is pinned, its hash
# in source.json's `pin`: the zip kept in raw/ is used while it is the
# pinned one, and osmdata, which serves only its latest, is asked only when
# it is not, which fails unless osmdata still serves the pin. Each run
# imports it, filled beside the table and swapped in, so readers never see it
# emptied.

set -e
set -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../../.." && pwd)"
DATA_DIR="$ROOT/raw"

set -a; . "$REPO_ROOT/.env"; set +a
psql_geo() { psql "$DB_GEO_URL" -v ON_ERROR_STOP=1 "$@"; }

URL=$(jq -r '.provider.url' "$ROOT/source.json")
TABLE=osm_land_polygons
PIN=$(jq -r '.pin.version' "$ROOT/source.json")
ACCESSED_AT=$(jq -r '.pin.accessed_at' "$ROOT/source.json")
ZIP="$DATA_DIR/$(basename "$URL")"
mkdir -p "$DATA_DIR"
hash_of() { shasum -a 256 "$1" | cut -c1-16; }

if [ ! -f "$ZIP" ] || [ "$(hash_of "$ZIP")" != "$PIN" ]; then
    echo "[coastline] $URL"
    curl -fL -o "$ZIP.part" "$URL"
    GOT=$(hash_of "$ZIP.part")
    if [ "$GOT" != "$PIN" ]; then
        echo "[coastline] osmdata serves $GOT ($(date -r "$ZIP.part" +%Y-%m-%d)), not the pinned $PIN: to take it, set source.json's pin to it; it is kept as $ZIP.part" >&2
        exit 1
    fi
    mv "$ZIP.part" "$ZIP"
fi
SHP=$(unzip -Z1 "$ZIP" | grep '\.shp$')
unzip -o -q "$ZIP" -d "$DATA_DIR"

NEXT="${TABLE}_next"
ogr2ogr -f PostgreSQL "PG:$DB_GEO_URL" "$DATA_DIR/$SHP" \
    -overwrite -nln "$NEXT" -nlt PROMOTE_TO_MULTI -lco GEOMETRY_NAME=geom -lco SPATIAL_INDEX=GIST -a_srs EPSG:3857
psql_geo -c "ANALYZE $NEXT;" > /dev/null
# One short transaction: the old table dropped, the new one and its indexes
# given its names.
psql_geo > /dev/null <<SQL
BEGIN;
SET LOCAL lock_timeout = '30s';
DROP TABLE IF EXISTS $TABLE;
ALTER TABLE $NEXT RENAME TO $TABLE;
COMMENT ON TABLE $TABLE IS '$PIN $ACCESSED_AT';
DO \$\$ DECLARE i text; BEGIN
  FOR i IN SELECT indexrelid::regclass::text FROM pg_index WHERE indrelid = '$TABLE'::regclass LOOP
    IF i LIKE '${NEXT}%' THEN EXECUTE format('ALTER INDEX %I RENAME TO %I', i, '$TABLE' || substr(i, length('$NEXT') + 1)); END IF;
  END LOOP;
END \$\$;
COMMIT;
SQL
echo "[coastline] $TABLE: $(psql_geo -Atc "SELECT count(*) FROM $TABLE") polygons, $(date -r "$ZIP" +%Y-%m-%d)"
