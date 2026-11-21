#!/bin/bash

# The elevation into db-geo, a shard at a time: each shard named, or the
# coverage's (`elevation_shard`), its apron of whole blocks included, warped
# out of the remote COG onto the one grid the bake samples, and its blocks put
# under the names of the tiles they are (`elevation_shard --blocks`). A shard
# is written into the live tables as they are, its blocks and its catalog row
# in one transaction, and takes the version source.json pins (`pin`); a
# block a neighbor's apron shares is the same block, stored once. With
# `--whole`, the tables are made anew from every shard beside the live ones
# and swapped in once every shard is whole, so readers never see them
# emptied; such a run stopped partway resumes from the shards its tables
# beside hold, and refuses ones of another version rather than mix them.
#
#   ./infra/geo/elevation/ingest.sh [--shard <name>]… [--whole]

set -e
set -o pipefail

ELEV_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$ELEV_ROOT/../../.." && pwd)"
# Gigabytes, gitignored and dockerignored.
WARPED_DIR="$ELEV_ROOT/warped"
TOOLS="$REPO_ROOT/infra/geo/tool/run"

set -a; . "$REPO_ROOT/infra/local/.env"; set +a
psql_geo() { "$TOOLS" psql "$DB_GEO_URL" -v ON_ERROR_STOP=1 "$@"; }

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

# `--shard <name>`, repeated, for those shards alone; `--whole` for the tables
# made anew from every shard.
NAMED=()
WHOLE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --shard)
            [ -n "$2" ] || { echo "--shard names a shard" >&2; exit 1; }
            NAMED+=("$2")
            shift 2
            ;;
        --whole) WHOLE=1; shift ;;
        *) echo "unknown flag: $1" >&2; exit 1 ;;
    esac
done
if [ "$WHOLE" = 1 ] && [ ${#NAMED[@]} -gt 0 ]; then
    echo "--whole makes the tables from every shard; it takes no --shard" >&2
    exit 1
fi

mkdir -p "$WARPED_DIR"
BAKE_BIN="${CARGO_TARGET_DIR:-$REPO_ROOT/rust/target}/server/elevation_shard"
(cd "$REPO_ROOT/rust" && cargo build -q --profile server -p pxl-geo-bake --bin elevation_shard)
SHARDS=$("$BAKE_BIN")
# Whether the name `$1` is among `$2`'s, one a line, its first word, compared
# as strings: awk compares two numeric-looking fields as numbers.
among() { awk -v n="$1" '$1 "" == n "" { found = 1 } END { exit !found }' <<< "$2"; }
for N in "${NAMED[@]}"; do
    if ! among "$N" "$SHARDS"; then
        echo "$N: no shard of the coverage (infra/geo/coverage.json)" >&2
        exit 1
    fi
done
NAMES=$(printf '%s\n' "${NAMED[@]}")

psql_geo -c "CREATE EXTENSION IF NOT EXISTS postgis_raster CASCADE;" > /dev/null
if [ "$WHOLE" = 1 ]; then
    RASTER=elevation_raster_next CATALOG=elevation_catalog_next
else
    RASTER=elevation_raster CATALOG=elevation_catalog
fi
tables "$RASTER" "$CATALOG" | psql_geo > /dev/null
if [ "$WHOLE" = 0 ]; then
    # A shard is added to the raster as it is; one on another grid is made anew only on --whole.
    read -r STRIDE BLOCK <<< "$(awk 'NR == 1 { print $6, $7 }' <<< "$SHARDS")"
    OFF=$(psql_geo -Atc "SELECT count(*) FROM (SELECT rast FROM $RASTER LIMIT 1) r
        WHERE abs(ST_ScaleX(rast) - $STRIDE) > 1e-9 * $STRIDE OR ST_Width(rast) <> $BLOCK")
    if [ "$OFF" != "0" ]; then
        echo "[elevation] $RASTER is on another grid than the stride $STRIDE and the block $BLOCK; run with --whole" >&2
        exit 1
    fi
else
    OTHER=$(psql_geo -Atc "SELECT count(*) FROM $CATALOG WHERE version <> '$VERSION'")
    if [ "$OTHER" != "0" ]; then
        echo "[elevation] the tables beside hold $OTHER shards of another version: drop $RASTER and $CATALOG first" >&2
        exit 1
    fi
fi

# On its own descriptor: psql would read the list as its input.
while read -r NAME MINX MINY MAXX MAXY STRIDE BLOCK BLOCK_MERC <&3; do
    if [ ${#NAMED[@]} -gt 0 ] && ! among "$NAME" "$NAMES"; then
        continue
    fi
    if [ "$WHOLE" = 1 ] && [ -n "$(psql_geo -Atc "SELECT 1 FROM $CATALOG WHERE shard = '$NAME'")" ]; then
        echo "[elevation/$NAME] already held"
        continue
    fi
    FILE="$NAME.tif"
    echo "[elevation/$NAME] warping"
    # The shard's apron's edges and the bake's stride, so every shard's pixels are one grid.
    "$TOOLS" gdalwarp -q -t_srs EPSG:3857 -te "$MINX" "$MINY" "$MAXX" "$MAXY" -tr "$STRIDE" "$STRIDE" \
        -overwrite -r bilinear -of GTiff -co COMPRESS=DEFLATE \
        "/vsicurl/$DOWNLOAD_URL" "$WARPED_DIR/$FILE"
    (cd "$WARPED_DIR" && "$TOOLS" raster2pgsql -d -s 3857 -t "${BLOCK}x${BLOCK}" "$FILE" elevation_stage) | psql_geo > /dev/null
    # A block's place on the grid is its west and north edges over a block's side; its tile is the one there.
    BLOCKS=$("$BAKE_BIN" --blocks "$NAME" | awk '{ printf "%s(%s, %s, '\''%s'\'')", (NR > 1 ? ", " : ""), $1, $2, $3 }')
    PLACE="round(ST_UpperLeftX(s.rast) / $BLOCK_MERC)::int = b.bx AND round(ST_UpperLeftY(s.rast) / $BLOCK_MERC)::int = b.by"
    UNPLACED=$(psql_geo -Atc "SELECT count(*) FROM elevation_stage s WHERE NOT EXISTS (SELECT 1 FROM (VALUES $BLOCKS) b(bx, by, tile) WHERE $PLACE)")
    if [ "$UNPLACED" != "0" ]; then
        echo "[elevation/$NAME] $UNPLACED blocks fall on no tile of the shard" >&2
        exit 1
    fi
    # One transaction: the shard's blocks and its catalog row, whole or absent.
    psql_geo > /dev/null <<SQL
BEGIN;
INSERT INTO $RASTER (tile, rast)
    SELECT b.tile, s.rast FROM elevation_stage s JOIN (VALUES $BLOCKS) b(bx, by, tile) ON $PLACE
    ON CONFLICT (tile) DO UPDATE SET rast = EXCLUDED.rast;
INSERT INTO $CATALOG (shard, version, accessed_at, total_count)
    SELECT '$NAME', '$VERSION', '$ACCESSED_AT', count(*) FROM elevation_stage
    ON CONFLICT (shard) DO UPDATE SET version = EXCLUDED.version,
        accessed_at = EXCLUDED.accessed_at, total_count = EXCLUDED.total_count;
COMMIT;
SQL
done 3<<< "$SHARDS"

psql_geo -c "DROP TABLE IF EXISTS elevation_stage; ANALYZE $RASTER;" > /dev/null
if [ "$WHOLE" = 1 ]; then
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
fi
echo "[elevation] $(psql_geo -Atc "SELECT count(*) FROM elevation_catalog") shards"
