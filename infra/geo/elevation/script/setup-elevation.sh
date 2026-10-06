#!/bin/bash

# The elevation into db-geo, a shard at a time: each shard the coverage
# needs (`elevation_shards`) warped out of the remote COG onto the one grid
# the bake samples, and loaded with its name. A shard whose catalog row is
# this source's date is kept, so a run stopped partway resumes.

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
DATA_DATE=$(jq -r '.[0].data_date' "$METADATA_FILE")
GDAL_IMAGE="ghcr.io/osgeo/gdal:ubuntu-small-latest"

psql_geo < "$ELEV_ROOT/init/01_elevation_schema.sql"
if [ -z "$(psql_geo -Atc "SELECT 1 FROM information_schema.columns WHERE table_name = 'elevation_catalog' AND column_name = 'shard'")" ]; then
    echo "❌ elevation_catalog predates shards: drop elevation_raster and elevation_catalog, and run again"
    exit 1
fi

mkdir -p "$WARPED_DIR"
SHARDS=$(cargo run -q --manifest-path "$REPO_ROOT/crates/Cargo.toml" -p pxl-geo-bake --bin elevation_shards)

# On its own descriptor: `docker exec -i` would read the list as its input.
while read -r NAME MINX MINY MAXX MAXY STRIDE <&3; do
    if [ -n "$(psql_geo -Atc "SELECT 1 FROM elevation_catalog WHERE shard = '$NAME' AND status = 'COMPLETED' AND data_date = '$DATA_DATE'")" ]; then
        echo "⏭️  $NAME already $DATA_DATE"
        continue
    fi
    FILE="$NAME.tif"
    echo "🔄 $NAME"
    psql_geo -c "DELETE FROM elevation_catalog WHERE shard = '$NAME'; DELETE FROM elevation_raster WHERE filename = '$FILE';" > /dev/null
    # The shard's edges and the bake's stride, so every shard's pixels are one grid.
    docker run --rm -v "$WARPED_DIR":/data "$GDAL_IMAGE" \
        gdalwarp -t_srs EPSG:3857 -te "$MINX" "$MINY" "$MAXX" "$MAXY" -tr "$STRIDE" "$STRIDE" \
        -overwrite -r bilinear -of GTiff -co COMPRESS=DEFLATE \
        "/vsicurl/$DOWNLOAD_URL" "/data/$FILE"
    (cd "$WARPED_DIR" && raster2pgsql -a -s 3857 -F -t 256x256 "$FILE" public.elevation_raster) | psql_geo > /dev/null
    psql_geo -c "INSERT INTO elevation_catalog (shard, data_date, file_name, status, total_count)
        SELECT '$NAME', '$DATA_DATE', '$FILE', 'COMPLETED', count(*) FROM elevation_raster WHERE filename = '$FILE';" > /dev/null
done 3<<< "$SHARDS"

psql_geo -c "ANALYZE public.elevation_raster;" > /dev/null
echo "✨ Elevation: $(psql_geo -Atc "SELECT count(*) FROM elevation_catalog WHERE status = 'COMPLETED'") shards"
