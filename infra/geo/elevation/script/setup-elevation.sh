#!/bin/bash

# The elevation into db-geo, a shard at a time: each shard the coverage
# needs (`elevation_shards`), its apron of whole blocks included, warped out
# of the remote COG onto the one grid the bake samples, and its blocks put
# under the names of the tiles they are (`elevation_shards --blocks`),
# written over where a neighbour's apron put them already: the same samples.
# The tables are filled beside the live ones and swapped in once every shard
# is whole, so readers never see them emptied; a run stopped partway resumes
# from the shards its tables beside hold. A new version (metadata.json's
# `pin`) is taken by running again.

set -e
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELEV_ROOT="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(cd "$ELEV_ROOT/../../.." && pwd)"
# Gigabytes, gitignored (infra/geo/.gitignore) and outside the Docker build context.
WARPED_DIR="$ELEV_ROOT/warped"

DOTENV_PATH="$REPO_ROOT/.env"
if [ ! -f "$DOTENV_PATH" ]; then
    echo "❌ Error: .env file not found at $DOTENV_PATH"
    exit 1
fi
export $(echo $(grep -v '^#' "$DOTENV_PATH" | xargs) | envsubst)
if [ -z "$DB_GEO_CONTAINER" ]; then
    echo "❌ Error: DB_GEO_CONTAINER is not set. Check your .env file."
    exit 1
fi
PGUSER="${DB_GEO_USER:-admin}"
PGDATABASE="${DB_GEO_NAME:-geo}"
psql_geo() { docker exec -i "$DB_GEO_CONTAINER" psql -v ON_ERROR_STOP=1 -U "$PGUSER" -d "$PGDATABASE" "$@"; }

METADATA_FILE="$ELEV_ROOT/metadata.json"
DOWNLOAD_URL=$(jq -r '.[0].provider.url' "$METADATA_FILE")
VERSION=$(jq -r '.[0].pin.version' "$METADATA_FILE")
GDAL_IMAGE="ghcr.io/osgeo/gdal:ubuntu-small-latest"

psql_geo < "$ELEV_ROOT/init/01_elevation_schema.sql"
# The tables beside, from the same definitions under their `_next` names.
perl -pe 's/\belevation_(raster|catalog)\b/elevation_${1}_next/g; s/\bidx_elevation_raster_/idx_elevation_raster_next_/g' \
    "$ELEV_ROOT/init/01_elevation_schema.sql" | psql_geo > /dev/null

mkdir -p "$WARPED_DIR"
BAKE_BIN="$REPO_ROOT/crates/target/server/elevation_shards"
cargo build -q --manifest-path "$REPO_ROOT/crates/Cargo.toml" --profile server -p pxl-geo-bake --bin elevation_shards
SHARDS=$("$BAKE_BIN")

# On its own descriptor: `docker exec -i` would read the list as its input.
while read -r NAME MINX MINY MAXX MAXY STRIDE BLOCK BLOCK_M <&3; do
    if [ -n "$(psql_geo -Atc "SELECT 1 FROM elevation_catalog_next WHERE shard = '$NAME'")" ]; then
        echo "⏭️  $NAME already"
        continue
    fi
    FILE="$NAME.tif"
    echo "🔄 $NAME"
    psql_geo -c "TRUNCATE elevation_stage;" > /dev/null
    # The shard's apron's edges and the bake's stride, so every shard's pixels are one grid.
    docker run --rm -v "$WARPED_DIR":/data "$GDAL_IMAGE" \
        gdalwarp -t_srs EPSG:3857 -te "$MINX" "$MINY" "$MAXX" "$MAXY" -tr "$STRIDE" "$STRIDE" \
        -overwrite -r bilinear -of GTiff -co COMPRESS=DEFLATE \
        "/vsicurl/$DOWNLOAD_URL" "/data/$FILE"
    (cd "$WARPED_DIR" && raster2pgsql -a -s 3857 -F -t "${BLOCK}x${BLOCK}" "$FILE" public.elevation_stage) | psql_geo > /dev/null
    # A block's place on the grid is its west and north edges over a block's side; its tile is the one there.
    BLOCKS=$("$BAKE_BIN" --blocks "$NAME" | awk '{ printf "%s(%s, %s, '\''%s'\'')", (NR > 1 ? ", " : ""), $1, $2, $3 }')
    PLACE="round(ST_UpperLeftX(s.rast) / $BLOCK_M)::int = b.bx AND round(ST_UpperLeftY(s.rast) / $BLOCK_M)::int = b.by"
    UNPLACED=$(psql_geo -Atc "SELECT count(*) FROM elevation_stage s WHERE NOT EXISTS (SELECT 1 FROM (VALUES $BLOCKS) b(bx, by, tile) WHERE $PLACE)")
    if [ "$UNPLACED" != "0" ]; then
        echo "❌ $NAME: $UNPLACED blocks fall on no tile of the shard"
        exit 1
    fi
    psql_geo -c "INSERT INTO elevation_raster_next (tile, rast)
            SELECT b.tile, s.rast FROM elevation_stage s JOIN (VALUES $BLOCKS) b(bx, by, tile) ON $PLACE
            ON CONFLICT (tile) DO UPDATE SET rast = EXCLUDED.rast;
        INSERT INTO elevation_catalog_next (shard, version, total_count)
            SELECT '$NAME', '$VERSION', count(*) FROM elevation_stage;
        TRUNCATE elevation_stage;" > /dev/null
done 3<<< "$SHARDS"

psql_geo -c "ANALYZE elevation_raster_next;" > /dev/null
# One short transaction: the live tables dropped, those beside given their names and their indexes'.
psql_geo > /dev/null <<SQL
BEGIN;
SET LOCAL lock_timeout = '30s';
DROP TABLE IF EXISTS elevation_raster, elevation_catalog;
ALTER TABLE elevation_raster_next RENAME TO elevation_raster;
ALTER TABLE elevation_catalog_next RENAME TO elevation_catalog;
DO \$\$ DECLARE i text; BEGIN
  FOR i IN SELECT indexrelid::regclass::text FROM pg_index WHERE indrelid IN ('elevation_raster'::regclass, 'elevation_catalog'::regclass) LOOP
    IF i LIKE '%_next%' THEN EXECUTE format('ALTER INDEX %I RENAME TO %I', i, replace(i, '_next', '')); END IF;
  END LOOP;
END \$\$;
COMMIT;
SQL
echo "✨ Elevation: $(psql_geo -Atc "SELECT count(*) FROM elevation_catalog") shards"
