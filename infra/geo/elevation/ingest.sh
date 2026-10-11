#!/bin/bash

# The elevation into db-geo, a shard at a time: each shard the coverage
# needs (`elevation_shards`), its apron of whole blocks included, warped out
# of the remote COG onto the one grid the bake samples, and its blocks put
# under the names of the tiles they are (`elevation_shards --blocks`). The
# tables are filled beside the live ones and swapped in once every shard is
# whole, so readers never see them emptied; a run stopped partway resumes
# from the shards its tables beside hold, and refuses ones of another
# version (source.json's `pin`) rather than mix them.

set -e
set -o pipefail

ELEV_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ELEV_ROOT/../../.." && pwd)"
# Gigabytes, gitignored and dockerignored.
WARPED_DIR="$ELEV_ROOT/warped"

set -a; . "$REPO_ROOT/.env"; set +a
psql_geo() { psql "$DB_GEO_URL" -v ON_ERROR_STOP=1 "$@"; }

SOURCE="$ELEV_ROOT/source.json"
DOWNLOAD_URL=$(jq -r '.provider.url' "$SOURCE")
VERSION=$(jq -r '.pin.version' "$SOURCE")
ACCESSED_AT=$(jq -r '.pin.accessed_at' "$SOURCE")

# The tables under the names given: the raster's blocks, each under the name
# of the tile it is, and the catalog's shards.
tables() {
    cat <<SQL
CREATE TABLE IF NOT EXISTS $1 (tile TEXT PRIMARY KEY, rast RASTER NOT NULL);
CREATE INDEX IF NOT EXISTS $1_rast ON $1 USING GIST (ST_ConvexHull(rast));
CREATE TABLE IF NOT EXISTS $2 (shard TEXT PRIMARY KEY, version TEXT NOT NULL, accessed_at TIMESTAMPTZ NOT NULL, total_count BIGINT NOT NULL);
SQL
}
psql_geo -c "CREATE EXTENSION IF NOT EXISTS postgis_raster CASCADE;" > /dev/null
tables elevation_raster_next elevation_catalog_next | psql_geo > /dev/null
OTHER=$(psql_geo -Atc "SELECT count(*) FROM elevation_catalog_next WHERE version <> '$VERSION'")
if [ "$OTHER" != "0" ]; then
    echo "[elevation] the tables beside hold $OTHER shards of another version: drop elevation_raster_next and elevation_catalog_next first" >&2
    exit 1
fi

mkdir -p "$WARPED_DIR"
BAKE_BIN="${CARGO_TARGET_DIR:-$REPO_ROOT/crates/target}/server/elevation_shards"
cargo build -q --manifest-path "$REPO_ROOT/crates/Cargo.toml" --profile server -p pxl-geo-bake --bin elevation_shards
SHARDS=$("$BAKE_BIN")

# On its own descriptor: psql would read the list as its input.
while read -r NAME MINX MINY MAXX MAXY STRIDE BLOCK BLOCK_M <&3; do
    if [ -n "$(psql_geo -Atc "SELECT 1 FROM elevation_catalog_next WHERE shard = '$NAME'")" ]; then
        echo "[elevation/$NAME] already held"
        continue
    fi
    FILE="$NAME.tif"
    echo "[elevation/$NAME] warping"
    # The shard's apron's edges and the bake's stride, so every shard's pixels are one grid.
    gdalwarp -q -t_srs EPSG:3857 -te "$MINX" "$MINY" "$MAXX" "$MAXY" -tr "$STRIDE" "$STRIDE" \
        -overwrite -r bilinear -of GTiff -co COMPRESS=DEFLATE \
        "/vsicurl/$DOWNLOAD_URL" "$WARPED_DIR/$FILE"
    (cd "$WARPED_DIR" && raster2pgsql -d -s 3857 -t "${BLOCK}x${BLOCK}" "$FILE" elevation_stage) | psql_geo > /dev/null
    # A block's place on the grid is its west and north edges over a block's side; its tile is the one there.
    BLOCKS=$("$BAKE_BIN" --blocks "$NAME" | awk '{ printf "%s(%s, %s, '\''%s'\'')", (NR > 1 ? ", " : ""), $1, $2, $3 }')
    PLACE="round(ST_UpperLeftX(s.rast) / $BLOCK_M)::int = b.bx AND round(ST_UpperLeftY(s.rast) / $BLOCK_M)::int = b.by"
    UNPLACED=$(psql_geo -Atc "SELECT count(*) FROM elevation_stage s WHERE NOT EXISTS (SELECT 1 FROM (VALUES $BLOCKS) b(bx, by, tile) WHERE $PLACE)")
    if [ "$UNPLACED" != "0" ]; then
        echo "[elevation/$NAME] $UNPLACED blocks fall on no tile of the shard" >&2
        exit 1
    fi
    psql_geo -c "INSERT INTO elevation_raster_next (tile, rast)
            SELECT b.tile, s.rast FROM elevation_stage s JOIN (VALUES $BLOCKS) b(bx, by, tile) ON $PLACE
            ON CONFLICT (tile) DO UPDATE SET rast = EXCLUDED.rast;
        INSERT INTO elevation_catalog_next (shard, version, accessed_at, total_count)
            SELECT '$NAME', '$VERSION', '$ACCESSED_AT', count(*) FROM elevation_stage;" > /dev/null
done 3<<< "$SHARDS"

psql_geo -c "DROP TABLE IF EXISTS elevation_stage; ANALYZE elevation_raster_next;" > /dev/null
# One short transaction: the live tables dropped, those beside given their names and their indexes'.
psql_geo > /dev/null <<SQL
BEGIN;
SET LOCAL lock_timeout = '30s';
DROP TABLE IF EXISTS elevation_raster, elevation_catalog;
ALTER TABLE elevation_raster_next RENAME TO elevation_raster;
ALTER TABLE elevation_catalog_next RENAME TO elevation_catalog;
DO \$\$ DECLARE i text; BEGIN
  FOR i IN SELECT indexrelid::regclass::text FROM pg_index WHERE indrelid IN ('elevation_raster'::regclass, 'elevation_catalog'::regclass) LOOP
    IF i LIKE '%\_next%' THEN EXECUTE format('ALTER INDEX %I RENAME TO %I', i, replace(i, '_next', '')); END IF;
  END LOOP;
END \$\$;
COMMIT;
SQL
echo "[elevation] $(psql_geo -Atc "SELECT count(*) FROM elevation_catalog") shards"
