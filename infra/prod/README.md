# apps/api's production deployment (Hetzner Cloud)

Forward search (`apps/api/src/modules/search/`) is the one feature that
needs a live, always-on server — POI/geocode/terrain stay static-bake-to-R2
and don't need any of this (see discussions/20260908-overture-poi-adoption.md).
Everything here exists only to make that one feature reachable in production.

**Chosen over Fly.io/AWS**: one Hetzner CX22 (2 vCPU / 4 GB / 40 GB, ~€4.50/mo,
20 TB pooled traffic) running app + both Postgres instances + a Caddy
reverse proxy in one docker-compose stack. Fly.io would split app/DB across
two billed resources for roughly 2-3x the cost at this scale; AWS's
equivalent (RDS + EC2/Fargate) commonly lands $40-60+/mo once NAT
gateway/storage/backup charges are counted in, for a workload this small
that never touches AWS's actual strengths (autoscaling fleets, multi-region).

## What's on this VM

Same `DB_MAIN`/`DB_GEO` split local dev already uses (`infra/docker-compose.yml`),
not consolidated into one Postgres server — same env var shape, same mental
model, and two idle Postgres containers cost nothing real on a 4 GB box.

**`db-geo` holds exactly three tables**: `overture_places`,
`overture_divisions`, `overture_division_points` — never terrain's own
ingestion tables (`overture_buildings`/`overture_transportation`/
`overture_base_*`/`esa_elevation_raster`/`osm_coastline_polygons`, several
GB combined, baked offline and already static on R2 — this server never
queries them) and never the
`tiger`/`topology` schemas (PostGIS's own bundled US Census geocoder data,
unrelated to this project, just ships with the `postgis/postgis` image by
default).

`db-main` exists only because `apps/api/src/app/resources.ts` probes it at
startup (`DB_MAIN_URL` is a required env var) — nothing queries it yet.

## One-time server setup

1. **Provision the VM** (from a machine with `hcloud` configured against a
   real API token — Hetzner account/billing setup itself has to happen in
   the Hetzner Cloud Console, not from a script). `cx22` was renamed
   `cx23` at some point (same 2 vCPU / 4 GB / 40 GB spec, ~€6.49/mo net as
   of this deploy — confirmed live, a little above the ~€4.50 quoted
   earlier in discussion):
   ```
   hcloud server create --name pixelet-prod --type cx23 --image ubuntu-24.04 \
     --location fsn1 --ssh-key <your-key-name>
   ```

2. **Firewall**: restrict 80/443 to Cloudflare's own published IP ranges
   (https://www.cloudflare.com/ips-v4, `-v6`) so an attacker who finds the
   origin IP by some other means (historical DNS, cert-transparency logs)
   can't bypass Cloudflare's own edge protection and hit the VM directly.
   SSH stays open to all (key-only auth is the real gate there):
   ```
   hcloud firewall create --name pixelet-prod-fw
   # build a rules JSON (direction/protocol/port/source_ips per rule —
   # 22 open to 0.0.0.0/0 and ::/0, 80/443 restricted to the Cloudflare
   # ranges above) and apply it:
   hcloud firewall replace-rules pixelet-prod-fw --rules-file fw_rules.json
   hcloud firewall apply-to-resource pixelet-prod-fw --type server --server pixelet-prod
   ```

3. **Install Docker** on the VM (Ubuntu):
   ```
   curl -fsSL https://get.docker.com | sh
   ```

4. **DNS**: add an A record for `api.plei.me` → the VM's public IP, **with
   Cloudflare's proxy (orange cloud) turned on** — plain DNS-only would
   both skip Cloudflare's own DDoS mitigation and defeat the firewall
   above (the origin would then be reachable directly by design). Caddy's
   automatic HTTPS (Caddyfile) needs this record to resolve before it can
   complete its ACME challenge; Let's Encrypt's own HTTP-01 challenge
   request still reaches the origin fine through a proxied record.

5. **Copy this repo** to the VM (`git clone` over SSH if a deploy key is
   set up, or `rsync` for a first pass — either is fine, the Dockerfile
   only needs the repo tree itself, see its own top comment on why the
   build context is the whole repo, not just `apps/api/`).

6. **Configure secrets**:
   ```
   cd infra/prod
   cp .env.example .env
   # fill in DB_MAIN_PASSWORD / DB_GEO_PASSWORD / ADMIN_TOKEN with fresh
   # values (openssl rand -base64 32 each) — never reuse local dev's own
   chmod 600 .env
   ```

7. **Bring the stack up**:
   ```
   docker compose up -d --build
   ```
   First run: `db-geo` starts empty (no tables yet), and the very first
   boot can log a handful of connection-refused retries while Postgres
   finishes its own first-time init — both resolve on their own (Docker's
   `restart: unless-stopped` retries the API container automatically) once
   Postgres is actually ready. See "Data migration" below before expecting
   `/api/search` to return anything real regardless.

## Data migration (local dev → production, one-time + whenever re-run)

Only the three tables above, straight from the same local Postgres this
session's own `ingest_places.ts`/`ingest_divisions.ts` already populate —
not a `pg_dumpall`, not the whole `pixelet_geo` database:

```sh
# Through the LOCAL db-geo container's own pg_dump, not the host's — a
# newer host-installed pg_dump (e.g. Homebrew's, often several major
# versions ahead) writes a custom-format dump version production's own
# postgis/postgis:16-3.4 pg_restore can't read ("unsupported version in
# file header", confirmed live). Both ends being the identical image
# guarantees format compatibility.
docker exec pixelet-db-geo pg_dump --format=custom --no-owner \
  -U "$DB_GEO_USER" -d "$DB_GEO_NAME" \
  -t overture_places -t overture_divisions -t overture_division_points \
  > overture.dump

# The production db-geo needs postgis/pg_trgm created once, up front —
# a table-only dump carries no CREATE EXTENSION statements:
ssh user@<vm-ip> 'docker exec pixelet-db-geo psql -U <DB_GEO_USER> -d <DB_GEO_NAME> \
  -c "CREATE EXTENSION IF NOT EXISTS postgis; CREATE EXTENSION IF NOT EXISTS pg_trgm;"'

# Copy to the VM, then restore into production's db-geo:
scp overture.dump user@<vm-ip>:~/
ssh user@<vm-ip> 'docker exec -i pixelet-db-geo pg_restore --no-owner --no-privileges \
  -U <DB_GEO_USER> -d <DB_GEO_NAME> --clean --if-exists < ~/overture.dump'
```

`--clean --if-exists` makes this safe to re-run wholesale on a monthly
refresh (drops and recreates just these three tables, same "full replace,
not an incremental diff" policy `ingest_places.ts`'s own top comment
already documents for the source data itself).

## Cloudflare Worker wiring

`apps/web`'s production Worker reverse-proxies `/search` to `API_ORIGIN`'s
`/api/search` (`wrangler.jsonc`'s own `vars`, `https://api.plei.me` — see
`worker/index.ts`), matching Vite's dev-time proxy
(`vite.config.ts`'s own `proxy: { "/search": ... }`).

## Backups

`db-geo`'s three tables are fully reproducible from Overture's own public
release (re-run `ingest_places.ts`/`ingest_divisions.ts` against this VM's
`DB_GEO_URL`) — a real backup matters less here than for genuinely
original data, but a simple cron'd `pg_dump` piped to the same R2 bucket
this project already pays for costs nothing extra to add later.

## Updating

```sh
git pull
docker compose up -d --build
```
