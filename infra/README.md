# Infrastructure

Two Postgres instances, the geographic data they hold, and how to bring both
up locally and in production.

```
infra/
├── docker-compose.yml   local db-main + db-geo
├── elevation/           the one script-driven ingest pipeline
├── main/                db-main init scripts
└── prod/                Hetzner stack: compose, Caddy, .env.example
```

## 1. Databases

| Service     | Port (ext:int) | Holds                       | Image          |
| :---------- | :------------- | :-------------------------- | :------------- |
| **db-main** | 5432:5432      | nothing yet                 | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | every table in §2           | PostGIS 16-3.4 |

`apps/api/src/app/resources.ts` probes db-main at startup, so `DB_MAIN_URL`
is required even though no query reads it.

- Local access: `localhost:5433`
- Container access: `db-geo:5432`

All spatial data is **EPSG:3857**. The renderer's voxel math assumes it.

## 2. What db-geo holds

Twelve tables from two sources. Terrain, POI and geocode are baked from
these offline and published to R2; only forward search queries them at
request time.

| Source                                                  | Tables                                                                                                | Catalog             | Ingest                                                                                                                |
| :------------------------------------------------------ | :---------------------------------------------------------------------------------------------------- | :------------------ | :-------------------------------------------------------------------------------------------------------------------- |
| **Overture — Buildings, Transportation, Base**<br>terrain features and coastline | `overture_buildings`, `overture_transportation`, `overture_base_land`, `overture_base_land_use`, `overture_base_water`, `overture_base_infrastructure` | `overture_catalog`  | `apps/api/src/bake/overture/tools/ingest_{buildings,transportation,base}.ts`                                          |
| **Overture — Places, Divisions**<br>POI, reverse geocode, forward search        | `overture_places`, `overture_divisions`, `overture_division_points`                                     | `overture_catalog`  | `apps/api/src/bake/overture/tools/ingest_{places,divisions}.ts`                                                        |
| **GEDTM30**<br>ground elevation                          | `elevation_raster`                                                                                     | `elevation_catalog` | `./elevation/scripts/setup-elevation.sh`                                                                              |

Coastline is not its own table: `OVERTURE_COAST_SQL`
(`apps/api/src/bake/terrain/repository.ts`) carves it at bake time from
`overture_base_water`'s `ocean` subtype, and `HAS_LAND_SQL`
(`bake/terrain/tools/bake_tiles.ts`) classifies a page as open ocean when
no `overture_base_land` row touches its bbox.

Both catalogs record one row per table with the source's own as-of date.
`read_source_catalogs()` (`bake/bake_util.ts`) copies them into every baked
tile version's `manifest.json`.

Every pipeline is **drop-and-recreate**: re-running one never duplicates or
conflicts.

### Sources

**Overture Maps** — public S3 release, no key or portal step. Ingested
through the official `overturemaps` Python CLI, auto-installed into a
`.venv` beside the tools on first run (gitignored). `--bbox` defaults to
South Korea, `124,33,132,39`.

**GEDTM30 v1.2** (OpenGeoHub / OpenLandMap) — one global Cloud-Optimized
GeoTIFF, cropped to each region's `bbox_4326` straight out of the remote
file over HTTP range requests. No raw-file staging step. A bare-earth DTM:
ground elevation carries no building or canopy height. Real elevation
exists only inside `metadata.json`'s `bbox_4326`; outside it, nothing.

## 3. Local build

### Prerequisites (macOS)

```sh
brew install gdal postgis jq gettext
brew link --force gettext   # puts envsubst on PATH
```

`gdal` for `gdalwarp`/`raster2pgsql`, `postgis` for the ingestion
utilities, `jq` for `metadata.json`, `envsubst` for the shell scripts.

### 1. Bring up db-geo

Run from the project root.

> `down -v` destroys the data volume, and that volume is the only copy of
> the ingested data on this machine. The raw sources are not in the
> repository and `region/*/raw/` holds only `.gitkeep`. Re-ingesting
> reproduces an equivalent dataset from each source's current state, not a
> byte-for-byte copy. Drop the `down -v` line to start an already-built
> database.

```sh
docker compose -f infra/docker-compose.yml --env-file .env down -v
docker compose -f infra/docker-compose.yml --env-file .env up -d db-geo

export $(grep -v '^#' .env | xargs)

until docker exec "$DB_GEO_CONTAINER" pg_isready -U postgres; do sleep 2; done
```

### 2. Ingest

Order does not matter. Each reads the root `.env` itself.

```sh
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_buildings.ts
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_transportation.ts
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_base.ts
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_places.ts
deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_divisions.ts

chmod +x infra/elevation/scripts/setup-elevation.sh
./infra/elevation/scripts/setup-elevation.sh
```

Each Overture tool's own top comment is the source of truth for its
mechanics — which `--type` values it pulls, which subtypes it skips.

### 3. Verify

```sql
SELECT 'overture_buildings' AS t, count(*) FROM overture_buildings
UNION ALL SELECT 'overture_transportation', count(*) FROM overture_transportation
UNION ALL SELECT 'overture_base_land', count(*) FROM overture_base_land
UNION ALL SELECT 'overture_base_land_use', count(*) FROM overture_base_land_use
UNION ALL SELECT 'overture_base_water', count(*) FROM overture_base_water
UNION ALL SELECT 'overture_base_infrastructure', count(*) FROM overture_base_infrastructure
UNION ALL SELECT 'overture_places', count(*) FROM overture_places
UNION ALL SELECT 'overture_divisions', count(*) FROM overture_divisions
UNION ALL SELECT 'overture_division_points', count(*) FROM overture_division_points;

SELECT r_table_name, srid, scale_x, scale_y FROM raster_columns
WHERE r_table_name = 'elevation_raster';
```

After a large ingest, `VACUUM ANALYZE` (or `VACUUM FULL ANALYZE` when
downtime is acceptable) reclaims what drop-and-recreate leaves behind.

## 4. The elevation pipeline

The one pipeline this directory runs directly, in bash + `envsubst` + GDAL.
`./elevation/`:

| | |
| :-------------- | :------------------------------------ |
| **Source**      | GEDTM30 v1.2, Cloud-Optimized GeoTIFF |
| **Raw data**    | `./elevation/region/[CODE]/raw/*.tif` |
| **Table**       | `elevation_raster`                    |
| **Catalog**     | `elevation_catalog`                   |
| **Schema**      | `01_elevation_schema.sql`             |

1. **Schema** — drops `elevation_raster`/`elevation_catalog`, runs
   `init/01_elevation_schema.sql`. The raster table itself is created by
   `raster2pgsql`.
2. **Crop and reproject** — `gdalwarp` inside the container reads the
   remote COG through `/vsicurl/`, crops to `bbox_4326` (`-te`, source
   CRS), writes EPSG:3857 with DEFLATE.
3. **Ingest** — `raster2pgsql -t 256x256` piped into `psql`, one row per
   region file. The tile size matches the voxel grid's page size.
4. **Finalize** — `ANALYZE`, then `elevation_catalog.status = COMPLETED`.

### Backup

```sh
docker exec -t $DB_GEO_CONTAINER pg_dump -U $PGUSER -d $PGDATABASE \
  -t public.elevation_raster -t public.elevation_catalog | gzip > elevation_backup.sql.gz

gunzip -c elevation_backup.sql.gz | docker exec -i $DB_GEO_CONTAINER psql -U $PGUSER -d $PGDATABASE
```

The Overture tables need no backup: re-running their ingest tools rebuilds
them from the public release.

## 5. Production (Hetzner Cloud)

Forward search is the only feature that needs a live server. Everything
else is baked to R2. One CX23 (2 vCPU / 4 GB / 40 GB) runs the API, both
Postgres instances and Caddy in one compose stack — `./prod/`.

Production's db-geo holds **only** `overture_places`, `overture_divisions`
and `overture_division_points`. Terrain's ingestion tables and the
elevation raster are several GB, baked offline, and never queried here.
The `tiger`/`topology` schemas that ship with the `postgis/postgis` image
are unrelated to this project.

### One-time setup

1. **VM** — Hetzner Cloud Console for the account and billing, then:
   ```sh
   hcloud server create --name pixelet-prod --type cx23 --image ubuntu-24.04 \
     --location fsn1 --ssh-key <your-key-name>
   ```

2. **Firewall** — 80/443 restricted to Cloudflare's published IP ranges
   (`cloudflare.com/ips-v4`, `-v6`), so an origin IP found through
   historical DNS or cert-transparency logs cannot bypass the edge. SSH
   stays open; key-only auth is the gate there.
   ```sh
   hcloud firewall create --name pixelet-prod-fw
   hcloud firewall replace-rules pixelet-prod-fw --rules-file fw_rules.json
   hcloud firewall apply-to-resource pixelet-prod-fw --type server --server pixelet-prod
   ```

3. **Docker** — `curl -fsSL https://get.docker.com | sh`

4. **DNS** — an A record for `api.plei.me` to the VM's public IP, **with
   Cloudflare's proxy on**. DNS-only skips the edge's DDoS mitigation and
   makes the firewall above pointless. Caddy's ACME challenge still
   reaches the origin through a proxied record.

5. **Repo** — `git clone` or `rsync` the tree onto the VM. The Dockerfile's
   build context is the whole repo, not just `apps/api/`.

6. **Secrets**
   ```sh
   cd infra/prod
   cp .env.example .env     # fill in, then chmod 600
   ```

7. **Up**
   ```sh
   docker compose up -d --build
   ```
   The first boot logs connection-refused retries while Postgres runs its
   own init; `restart: unless-stopped` resolves them.

### Data migration

Only the three Places/Divisions tables, and through the **local db-geo
container's** own `pg_dump` — a host-installed `pg_dump` several major
versions ahead writes a custom-format dump that production's
`postgis/postgis:16-3.4` cannot restore.

```sh
docker exec pixelet-db-geo pg_dump --format=custom --no-owner \
  -U "$DB_GEO_USER" -d "$DB_GEO_NAME" \
  -t overture_places -t overture_divisions -t overture_division_points \
  > overture.dump

# a table-only dump carries no CREATE EXTENSION
ssh user@<vm-ip> 'docker exec pixelet-db-geo psql -U <DB_GEO_USER> -d <DB_GEO_NAME> \
  -c "CREATE EXTENSION IF NOT EXISTS postgis; CREATE EXTENSION IF NOT EXISTS pg_trgm;"'

scp overture.dump user@<vm-ip>:~/
ssh user@<vm-ip> 'docker exec -i pixelet-db-geo pg_restore --no-owner --no-privileges \
  -U <DB_GEO_USER> -d <DB_GEO_NAME> --clean --if-exists < ~/overture.dump'
```

`--clean --if-exists` makes this safe to re-run for a monthly refresh.

### Updating

```sh
git pull
docker compose up -d --build
```

## 6. Cloudflare Worker wiring

Everything the server answers is under `/api`. The Worker claims
`/api/tiles/*` and reads it out of R2; everything else under that root is
reverse-proxied whole to `API_ORIGIN` (`apps/web/wrangler.jsonc`'s `vars`,
`https://api.plei.me`), path unchanged. Vite's dev proxy splits the same
root the same way, both from `apps/web/src/lib/env/routes.ts`. Nothing is
rewritten in transit: client, relay and origin use identical paths.

Neither relay reads past the root, so a new live route is a change to
`apps/api` alone. Every route must be mounted under `/api`
(`apps/api/src/app/mount.ts`) — a bare path is unreachable in production,
where the static-asset fallback answers it with `index.html`.

The root is reserved on the server's side because the app routes on paths
of its own; giving the app the prefix instead would put one in every URL a
user sees.

## 7. Licensing and attribution

Two notices are required, shown in
`apps/web/src/app/settings/sections/attribution_section.tsx`. Check the
source license before shortening, rewording or dropping either.

| Source     | License                                                   | Notice                                                                                                                                                                                                                                                                    |
| :--------- | :-------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| OpenStreetMap | ODbL                                                    | `© OpenStreetMap contributors` — the historical form listed as acceptable by the [OSMF Attribution Guidelines](https://osmfoundation.org/wiki/Licence/Attribution_Guidelines). Overture's Buildings/Transportation/Base/Divisions are substantially OSM-sourced and carry this obligation through. |
| GEDTM30    | [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/) | The pipeline crops and reprojects the raw DTM, so CC-BY requires noting the change: `Contains modified GEDTM30 data. Ho Y, Grohmann CH, Lindsay J, Reuter HI, Parente L, Witjes M, Hengl T (2025). GEDTM30: global ensemble digital terrain model at 30 m and derived multiscale terrain variables. PeerJ 13:e19673.` |

Overture Maps Foundation itself needs no notice. Places is
CDLA-Permissive-2.0, whose only condition is shipping the license text with
the data, and the ODbL obligation on the other themes runs to OSM's
contributors rather than to Overture as redistributor. Overture's
[attribution page](https://docs.overturemaps.org/attribution/) recommends a
combined notice as best practice; this project keeps the legally required
minimum.

## 8. Secrets

DB passwords and `ADMIN_TOKEN` come from `.env` only. `DB_GEO_USER`,
`DB_GEO_PASSWORD` and the rest must be exported before running any script.
Production's `.env` uses its own generated values, never local dev's.
