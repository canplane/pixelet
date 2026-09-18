# Infrastructure

Two Postgres instances, the geographic data they hold, and how to bring both
up locally and in production.

```
infra/
├── local/               the two databases for development: one compose
└── prod/                the Hetzner stack: compose, Caddy, .env.example
```

The ingests that fill db-geo are bake tools, not infrastructure, and live
with the rest of the bake: `apps/api/src/bake/overture/tool/ingest_*.ts`
and `apps/api/src/bake/elevation/` (a bash + GDAL pipeline). Their
procedures are documented here because they are run against these
databases.

## 1. Databases

| Service     | Port (ext:int) | Holds             | Image          |
| :---------- | :------------- | :---------------- | :------------- |
| **db-main** | 5432:5432      | nothing yet       | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | every table in §2 | PostGIS 16-3.4 |

db-main holds nothing. pxl-geo takes `DB_GEO_URL` alone; pxl-api owns db-main,
connects to it and probes it at boot, and has no route that reads it yet
([the server in Rust](../issue/20260918-the-server-in-rust-and-the-line-already-drawn.md)).

- Local access: `localhost:5433`
- Container access: `db-geo:5432`

All spatial data is **EPSG:3857**. The renderer's voxel math assumes it.

## 2. What db-geo holds

Twelve tables from two sources. Terrain, POI and geocode are baked from
these offline and published to R2; only forward search queries them at
request time.

| Source                                                                           | Tables                                                                                                                                                 | Catalog             | Ingest                                                                      |
| :------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------ | :-------------------------------------------------------------------------- |
| **Overture — Buildings, Transportation, Base**<br>terrain features and coastline | `overture_buildings`, `overture_transportation`, `overture_base_land`, `overture_base_land_use`, `overture_base_water`, `overture_base_infrastructure` | `overture_catalog`  | `apps/api/src/bake/overture/tool/ingest_{buildings,transportation,base}.ts` |
| **Overture — Places, Divisions**<br>POI, reverse geocode, forward search         | `overture_places`, `overture_divisions`, `overture_division_points`                                                                                    | `overture_catalog`  | `apps/api/src/bake/overture/tool/ingest_{places,divisions}.ts`              |
| **GEDTM30**<br>ground elevation                                                  | `elevation_raster`                                                                                                                                     | `elevation_catalog` | `apps/api/src/bake/elevation/script/setup-elevation.sh`                     |

Coastline is not its own table: `OVERTURE_COAST_SQL`
(`apps/api/src/bake/terrain/repository.ts`) carves it at bake time from
`overture_base_water`'s `ocean` subtype, and `HAS_LAND_SQL`
(`bake/terrain/tool/bake_tiles.ts`) classifies a page as open ocean when
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
> the ingested data on this machine. The sources are not in the repository
> (the warped rasters land under the gitignored `out/elevation/`). Re-ingesting
> reproduces an equivalent dataset from each source's current state, not a
> byte-for-byte copy. Drop the `down -v` line to start an already-built
> database.

```sh
docker compose -f infra/local/docker-compose.yml --env-file .env down -v
docker compose -f infra/local/docker-compose.yml --env-file .env up -d db-geo

export $(grep -v '^#' .env | xargs)

until docker exec "$DB_GEO_CONTAINER" pg_isready -U postgres; do sleep 2; done
```

### 2. Ingest

Order does not matter. Each reads the root `.env` itself.

```sh
deno run -A --env=.env apps/api/src/bake/overture/tool/ingest_buildings.ts
deno run -A --env=.env apps/api/src/bake/overture/tool/ingest_transportation.ts
deno run -A --env=.env apps/api/src/bake/overture/tool/ingest_base.ts
deno run -A --env=.env apps/api/src/bake/overture/tool/ingest_places.ts
deno run -A --env=.env apps/api/src/bake/overture/tool/ingest_divisions.ts

chmod +x apps/api/src/bake/elevation/script/setup-elevation.sh
./apps/api/src/bake/elevation/script/setup-elevation.sh
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

The one script-driven ingest, in bash + `envsubst` + GDAL, at
`apps/api/src/bake/elevation/`:

|                  |                                            |
| :--------------- | :----------------------------------------- |
| **Source**       | GEDTM30 v1.2, Cloud-Optimized GeoTIFF      |
| **Working data** | `out/elevation/region/[CODE]/warped/*.tif` |
| **Table**        | `elevation_raster`                         |
| **Catalog**      | `elevation_catalog`                        |
| **Schema**       | `01_elevation_schema.sql`                  |

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
else is baked to R2. One CX23 (2 vCPU / 4 GB / 40 GB) runs both servers, both
Postgres instances and Caddy in one compose stack — `./prod/`: `api` is pxl-geo
on `/geo/*`, `app` is pxl-api on `/api/*`, and Caddy is what sends each root to
its own container. Neither server has a published host port; only Caddy does.

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

5. **Repo** — `git clone` or `rsync` the tree onto the VM. Both Dockerfiles
   take the whole repo as their build context: the servers are `crates/pxl/geo`
   and `crates/pxl/api`, and their dependencies are path dependencies in the
   same Cargo workspace, so `crates/` has to be present whole for Cargo to
   resolve it. pxl-geo's file is still `apps/api/Dockerfile` — it kept its path
   when the server stopped being TypeScript — and builds none of apps/api;
   pxl-api's is `crates/pxl/api/Dockerfile`.

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

## 6. Cloudflare (the client, the tiles, and the relays)

### Wiring

Three roots, decided in `spec/route.json` and written from there into every
place that builds a URL or reads one:

| Root      | What it is                                        | Who answers it                            |
| :-------- | :------------------------------------------------ | :---------------------------------------- |
| `/geo/`   | the world's data                                  | R2 under `/geo/tiles/`, pxl-geo otherwise |
| `/api/`   | the application                                   | pxl-api                                   |
| `/asset/` | models and textures, by the sha256 of their bytes | R2                                        |

The Worker claims all three (`wrangler.jsonc`'s `run_worker_first`) and splits
them: `/geo/tiles/*` and `/asset/*` come straight out of the bucket, and the
rest of `/geo/*` and all of `/api/*` are reverse-proxied to `GEO_ORIGIN` and
`API_ORIGIN` with the path unchanged. An asset's key _is_ its path without the
leading slash — the root and the bucket's own prefix are the same word — where
a page's key is the tile branch removed. Caddy on the VM then sends each root
to the container that answers it. Nothing is rewritten in transit: client,
relay and origin use identical paths.

No relay reads past a root or its one branch, so a new live route is a change
to the one server that answers it. Every route must be under a claimed root —
a bare path is unreachable in production, where the static-asset fallback
answers it with `index.html`, which reads as the app loading rather than as a
route that is not there. A root is reserved on the server's side because the
app routes on paths of its own; giving the app the prefix instead would put one
in every URL a user sees.

`deno task dev` runs one process per root plus the tiles: pxl-geo on 3000,
serve_tiles on 3001, pxl-api on 3002, and the dev proxy splits on the same
prefixes the Worker does. A deployment has no tile server at all, because the
bucket answers those two prefixes.

### Deploying the client

**A green push to main publishes it** — see §7. What follows is the same thing
by hand, which is the fallback when CI is unavailable and what CI itself runs.

One Worker serves the built app, the R2 tiles, and the relay to the API, so
publishing all three is one command from the repo root:

```sh
deno task deploy
```

It builds, then runs `check:bundle`, then publishes. The middle step is the
point: a dev-only surface can be gated correctly in source and still leave
its strings in the artifact, and this fails the deploy rather than letting
it be noticed on the live site. Run it through the task rather than calling
wrangler directly, which skips that check — and which needs a Node newer
than the system may have, where Deno's compat layer satisfies it.

`wrangler login` once, per machine.

Static assets are content-hashed, so a deploy uploads only what changed and
the R2 tiles are untouched by it. Uploading tiles and assets is a separate
procedure with its own verification, in
[.agents/skills/publish-bucket/SKILL.md](../.agents/skills/publish-bucket/SKILL.md).

Afterwards, every path the app takes, plus the gates and the addresses that
must be gone:

```sh
deno task smoke https://plei.me
deno task smoke https://api.plei.me --origin   # the stack behind the edge
```

It builds the URLs from the same generated constants the client and the Worker
read, so a version bump or a moved root cannot leave it checking something
nobody serves. Both deploy jobs end with it.

What it does not check, and is worth an eye on a format change: a tile's first
two bytes must be `5854` (`XT`); `1f8b` means it is reaching the client still
gzipped, and HTML means the static-asset fallback answered a route the Worker
should have claimed. An asset answers `model/gltf-binary` or `image/png` with
no `content-encoding`.

## 7. CI/CD

`.github/workflows/check-and-deploy.yml` runs on every push to main and every
pull request. One workflow, three jobs, because a deploy must not be able to
start against a commit the gate never saw:

| Job             | When                                                                 | What                                                                                                        |
| :-------------- | :------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------- |
| `gate`          | always                                                               | `deno task check`, `test`, `lint`                                                                           |
| `deploy-web`    | main, gate green                                                     | `deno task deploy`, then `deno task smoke https://plei.me`                                                  |
| `deploy-server` | main, gate green, and `crates/`/`infra/prod/`/the Dockerfile changed | ssh, `git reset --hard origin/main`, `docker compose up -d --build --force-recreate`, then the origin smoke |

The two deploys are gated differently on purpose. Publishing to the edge is
idempotent and reversible from the Cloudflare dashboard; reaching into the
production VM over SSH is neither, so a push that could not have changed
either image does not touch it.

`--force-recreate` is not decoration: the Caddyfile is a bind mount and a bind
mount is read at start, so an `up --build` alone leaves a stale relay in front
of a fresh server. On 2026-09-18 that looked like `/api/*` answering 404 from
the wrong process rather than like a failure.

`deploy-web` is `concurrency: deploy-web` across every ref, because the build
number comes from one counter object in the bucket and two runs holding it at
once would take the same one. A hand `deno task deploy` is outside that group —
it is the fallback, not a second deployer, and running one while CI is mid-
deploy is the one way to collide.

### Secrets

Repository secrets, all of them set in GitHub's own settings and nowhere else:

| Name                    | For                                                                                                                             |
| :---------------------- | :------------------------------------------------------------------------------------------------------------------------------ |
| `CLOUDFLARE_API_TOKEN`  | wrangler: publish the Worker, read/write the build counter in R2                                                                |
| `CLOUDFLARE_ACCOUNT_ID` | which account, since a token may reach more than one                                                                            |
| `VM_HOST` / `VM_USER`   | the Hetzner box                                                                                                                 |
| `VM_SSH_KEY`            | a deploy-only private key; its public half in that user's `authorized_keys`                                                     |
| `VM_KNOWN_HOSTS`        | `ssh-keyscan <host>` run on the VM. Optional, and the deploy warns without it: the fallback accepts whatever answers on the day |
| `R2_*`                  | the monthly refresh's own, already set — see below                                                                              |

The VM key should be its own keypair rather than a personal one, so revoking
it costs nothing else.

### The monthly refresh

`.github/workflows/overture-monthly-refresh.yml`, 03:00 UTC on the 1st. It
ingests Overture Places and Divisions into a throwaway PostGIS service
container, bakes POI and reverse geocode with `--force`, and syncs the two
published prefixes in place. It needs both toolchains: the two ingests are
Deno and staying that way, the two bakes have been Rust since 2026-09-18.

It never rebuilds terrain's land/ocean classification — it fetches
`_meta/land_pages.json` from the bucket. Re-upload that object by hand (the
publish-bucket skill's terrain step) if the classification is ever rebuilt.

## 8. Licensing and attribution

The notices below are required, shown in
`apps/web/src/app/setting/section/attribution_section.tsx`. Check the
source license before shortening, rewording or dropping any of them.

| Source              | License                                                                                                  | Notice                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| :------------------ | :------------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenStreetMap       | [ODbL](https://opendatacommons.org/licenses/odbl/)                                                       | `© OpenStreetMap contributors, available under the Open Database License` — the [OSMF Attribution Guidelines](https://osmfoundation.org/wiki/Licence/Attribution_Guidelines) require the credit to make clear the data is under ODbL, so the licence is named rather than left to the link alone. Overture's Buildings/Transportation/Base/Divisions are substantially OSM-sourced and carry this obligation through.                                                                                                                                                                                                 |
| GEDTM30             | [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/)                                                | Creator, title, licence link and an indication of change — the pipeline crops and reprojects, so it changed: `Elevation: modified GEDTM30 by OpenGeoHub, licensed under CC BY 4.0.` The author list and journal reference that used to sit here are an academic courtesy, not part of what CC BY asks, and were long enough to crowd out the Copernicus row below, which is not optional. Link the [DOI this pipeline consumes](https://doi.org/10.5281/zenodo.15689805) (`apps/api/src/bake/elevation/metadata.json`), which is what the app's own notice and README link too — one source, not three mirrors of it. |
| Copernicus WorldDEM | [Copernicus DEM licence](https://spacedata.copernicus.eu/collections/copernicus-digital-elevation-model) | Compulsory, and the wording is dictated. GEDTM30 is a fusion whose inputs include Copernicus WorldDEM, and the licence binds anyone communicating adapted data to its "produced using" form, verbatim: `produced using Copernicus WorldDEM © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved`.                                                                                                                                                                                                                          |

Overture Maps Foundation itself needs no notice. Places is
CDLA-Permissive-2.0, whose only condition is shipping the license text with
the data, and the ODbL obligation on the other themes runs to OSM's
contributors rather than to Overture as redistributor. Overture's
[attribution page](https://docs.overturemaps.org/attribution/) offers a
citation a user "may choose to add" — its own wording, and the reason this is
a choice rather than an obligation. This project keeps the required minimum.

Still open: GEDTM30 also fuses JAXA's ALOS World 3D-30m. JAXA's terms page was
not readable when this was written, so no claim is made here about its exact
required wording rather than inventing one.

## 9. Secrets

DB passwords and `ADMIN_TOKEN` come from `.env` only. `DB_GEO_USER`,
`DB_GEO_PASSWORD` and the rest must be exported before running any script.
Production's `.env` uses its own generated values, never local dev's.
