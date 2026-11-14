# Infrastructure

Almost everything a player sees is made ahead of time and served as files that
never change: the tiles, the models, the textures. Only search and the
application need a live server. So production is small: one Cloudflare Worker in
front of one virtual machine, and a deploy moves the client and the servers
together from one commit.

```text
                ┌─ the app ────────────────────────────▶ its own static assets
client ─ Worker ┼─ /tile/*, /asset/* ──────────────────▶ the R2 bucket
                └─ /geo/*, /api/* ── Caddy on the VM ─┬▶ pxl-geo ── db-geo
                                                      └▶ pxl-api ── db-main
```

This file is for the person who runs that: the databases, the VM, Cloudflare,
CI, and the providers' notices.

```text
infra/
├── local/    the two databases for development
├── prod/     the VM's stack: compose and Caddy
├── geo/      what fills db-geo
└── earth/    the Earth's picture, NASA's, which the earth bake reads as files
```

## 1. Databases

| Service     | Port (ext:int) | Holds                        | Image          |
| :---------- | :------------- | :--------------------------- | :------------- |
| **db-main** | 5432:5432      | nothing yet                  | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | three sources, for the bakes | PostGIS 16-3.4 |

pxl-api owns db-main and probes it at boot, but no route reads it yet. pxl-geo
reads db-geo through `DB_GEO_URL`. All spatial data is in EPSG:3857. Locally the
passwords come from `infra/local/.env` and `ADMIN_TOKEN` from `rust/server/common/.env`;
production keeps its own in `infra/prod/.env`.

db-geo's volume is the only copy of hours of ingest, so it is never removed
with `down -v` ([geo/README.md](./geo/README.md)).

## 2. Filling db-geo

[geo/README.md](./geo/README.md) describes the sources, the tables and how to
run each ingest. One rule holds for all of them: a table is never emptied first.
A table made anew is built as `<table>_next` while readers keep using the old
one; then one short transaction drops the old and renames the new, so a fill
needs room for both at once. Nothing is dropped or truncated by hand.

## 3. Production (Hetzner Cloud)

One CX23 runs both servers, both databases and Caddy in one compose stack
(`prod/`). Caddy is the only published port; it sends `/geo/*`
to `api` (pxl-geo) and `/api/*` to `app` (pxl-api). Production's db-geo holds
only what search reads: `overture_places`, `overture_division_areas` and
`overture_divisions`.

### Setup, once

1. **The VM.**
   ```sh
   hcloud server create --name pixelet-prod --type cx23 --image ubuntu-24.04 \
     --location fsn1 --ssh-key <your-key-name>
   ```
2. **The firewall.** Ports 80 and 443 are open to Cloudflare's IP ranges alone
   (`cloudflare.com/ips-v4`, `-v6`), so the origin cannot be reached around the
   edge. SSH takes keys only.
   ```sh
   hcloud firewall create --name pixelet-prod-fw
   hcloud firewall replace-rules pixelet-prod-fw --rules-file fw_rules.json
   hcloud firewall apply-to-resource pixelet-prod-fw --type server --server pixelet-prod
   ```
3. **Docker.** `curl -fsSL https://get.docker.com | sh`.
4. **DNS.** An A record for the origin's host (`spec/spec/route.json5`
   `origins`) points at the VM and is **proxied** through Cloudflare. DNS-only
   would bypass the edge that the firewall relies on.
5. **The tree.** The repository is cloned whole onto the VM. Both servers'
   Dockerfiles (`rust/server/geo/Dockerfile`, `rust/server/api/Dockerfile`)
   build from the repository root, because their dependencies are path
   dependencies across `rust/`.
6. **Up.**
   ```sh
   cd infra/prod
   cp .env.example .env     # fill in, then chmod 600
   # bounds on the disk: container logs, BuildKit's cache by last use, and the
   # journal by size and age; a server deploy reinstalls them when they change
   install -m 644 etc/daemon.json /etc/docker/daemon.json && systemctl restart docker
   install -D -m 644 etc/journald.conf /etc/systemd/journald.conf.d/pixelet.conf && systemctl restart systemd-journald
   docker compose up -d --build
   ```
   CI deploys as root, which installing those files needs.

After setup, every deploy updates the stack with `infra/prod/deploy.sh`, sent
from a checkout and run on the VM. By hand it is the same script, for a commit
`deploy` already holds, since the VM fetches it from there:

```sh
ssh root@$(hcloud server ip pixelet-prod) bash -eu -s -- "$(git rev-parse origin/deploy)" < infra/prod/deploy.sh
```

### Copying the search tables

A deploy does not carry db-geo, so production's three tables change only when
they are copied over. When a copy changes a table's shape, the server that reads
it goes out in the same step: stop `api` (`docker compose stop api`), restore,
then deploy the server.

Dump with the local container's own `pg_dump`, since a newer one on the host
writes a format that production's PostgreSQL 16 cannot restore:

```sh
set -a; . ./infra/local/.env; set +a
docker exec pixelet-db-geo pg_dump --format=custom --no-owner \
  -U "$DB_GEO_USER" -d "$DB_GEO_NAME" \
  -t overture_places -t overture_division_areas -t overture_divisions > overture.dump

scp overture.dump root@$(hcloud server ip pixelet-prod):
ssh root@$(hcloud server ip pixelet-prod)
```

Then run the rest in the VM's own shell. Piped into `ssh`, the first
`docker exec -i` would swallow the commands after it.

```sh
sql() { docker exec -i pixelet-db-geo sh -c 'exec "$@" -U "$POSTGRES_USER" -d "$POSTGRES_DB"' sh "$@"; }

# a table-only dump carries no CREATE EXTENSION
sql psql -v ON_ERROR_STOP=1 -c 'CREATE EXTENSION IF NOT EXISTS postgis; CREATE EXTENSION IF NOT EXISTS pg_trgm;'
# one transaction: a failure leaves the old tables as they were
sql pg_restore --no-owner --no-privileges --clean --if-exists --single-transaction < ~/overture.dump
# the planner's statistics and the visibility map, which a restore does not
# write; one worker, since parallel workers outgrow Docker's 64 MB /dev/shm
sql psql -v ON_ERROR_STOP=1 -c "VACUUM (ANALYZE, PARALLEL 0) overture_places" -c "VACUUM (ANALYZE, PARALLEL 0) overture_division_areas" -c "VACUUM (ANALYZE, PARALLEL 0) overture_divisions"
rm ~/overture.dump
```

`./script/smoke.py --origin` checks the result.

## 4. Cloudflare

Every URL the client reads falls under one of four roots, written once in
`spec/spec/route.json5` and read from there by everything that builds or answers
a URL:

| Root      | What it is                                           | Answered by |
| :-------- | :--------------------------------------------------- | :---------- |
| `/geo/`   | the world's data                                     | pxl-geo     |
| `/api/`   | the application                                      | pxl-api     |
| `/tile/`  | baked tiles, by kind, version and name               | R2          |
| `/asset/` | models and textures, by path with a hash in the stem | R2          |

**The bucket is private, and the Worker is the only way into it.** The Worker
runs on the free plan, which fails closed at 100k requests a day, so no read can
cost money. A paid Workers plan, the bucket's `r2.dev` URL or a custom domain on
the bucket would break that. Writes are outside the bound and no spending cap is
set: a publish is a whole pyramid of Class A operations, so check the month's
count before one.

The bucket, `pixelet-bucket`, holds everything the client fetches that is not
the app itself:

| Prefix                  | Object                                          | Named by                 | Policy                                                           |
| ----------------------- | ----------------------------------------------- | ------------------------ | ---------------------------------------------------------------- |
| `terrain/<version>/`    | packed XTP tile                                 | name, 4–15               | immutable; a re-bake is a new version                            |
| `place/<version>/`      | XTP count tile                                  | name, 7–15               | immutable; a re-bake is a new version                            |
| `division/<version>/`   | XTP division tile                               | name, 4–12               | immutable; a re-bake is a new version                            |
| `earth/<version>/`      | packed earth tile; `sphere`, its picture whole  | name, 4                  | immutable; a re-bake is a new version                            |
| `asset/`                | glb, png, webp, not packed                      | the SHA-256 of its bytes | immutable by construction; unreferenced keys deleted at any time |
| `source`                | the source's declaration, JSON                  | fixed name               | mutable; written by `publish`, `no-cache`, revalidated each boot |
| `_meta/build-counter`   | the official build count                        | fixed name               | mutable; taken and written by CI's number job, never by hand     |
| `_meta/build-log.jsonl` | one line for each published build               | fixed name               | appended by CI's `record` (`script/build-log.py`)                |
| `_meta/paths/<n>.json`  | build `#n`'s source paths, to decode its panics | build number             | written by `make deploy` under a build number; never served      |

A tile's name is its Morton key, so the same name under two kinds is the same
ground, and one prefix is one region across every kind and every zoom.

The Worker claims all four roots (`wrangler.jsonc` `run_worker_first`). The
bucket answers `/tile/*` under its path less the root, and `/asset/*` under its
path; `/geo/*` and `/api/*` go to `GEO_ORIGIN` and `API_ORIGIN` unchanged. No
path is rewritten anywhere. A route outside the roots cannot be reached in
production: the app's fallback answers it with `index.html`, which looks like
the app loading rather than a missing route.

**Deploying the client** is CI's job (§5); by hand it is the script CI runs.
Neither checks nor tests anything, so `make -j8 check` and `make -j8 test` must
pass over the commit first, and the servers it talks to must already be deployed
(§3), since a new client against an old server is the direction that breaks. A
deploy by hand takes no build number.

```sh
wrangler login           # once a machine
make -j8 check && make -j8 test
make deploy              # build and publish
./script/smoke.py
```

A deploy uploads only the app's changed static assets. Tiles go into the bucket
with `./script/publish.py <kind>`, which uploads a new version and then writes
`source`; models and textures by the sync in
[asset/README.md](../asset/README.md).

## 5. CI/CD

`.github/workflows/deploy.yml` runs on a push to `deploy` and on a dispatch,
never on main. Main carries work between machines, whether or not it compiles.
A push that changes only markdown runs nothing. **Publishing is
`git push origin main:deploy`**, forced once main has been rewritten, after
`make -j8 check` and `make -j8 test` pass here, because CI checks and tests
nothing.

| Job             | When                  | What                                                                                                    |
| :-------------- | :-------------------- | :------------------------------------------------------------------------------------------------------ |
| `number`        | every push to deploy  | takes a build number                                                                                    |
| `deploy-server` | after `number`        | ssh, `infra/prod/deploy.sh`: reset to the run's commit, `docker compose up -d --build --force-recreate` |
| `deploy-web`    | after `deploy-server` | `make deploy` under the build number: builds, keeps the panic paths, publishes                          |
| `smoke`         | after both deployed   | `./script/smoke.py`                                                                                     |
| `record`        | after either deployed | appends the build to the log                                                                            |

A deploy is whole: the client and the servers go out from the same commit every
time, so they never run different ones. The servers go first, because an old
client against a new server is safe and the reverse is not.

Every build of `deploy` takes a number from a counter in the bucket, which is
never reset. A published build is one line in `_meta/build-log.jsonl`, with the
subjects of the commits it carried, because main's commit ids do not survive
its rewrites; `./script/build-log.py` reads it, newest first. A deployment
states its own number at `/build.json`.

The secrets are GitHub's alone: `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID` for wrangler; `R2_ACCESS_KEY_ID` and
`R2_SECRET_ACCESS_KEY`, held to the bucket, for the counter, the log and the
panic paths; `VM_HOST`, `VM_USER` and `VM_SSH_KEY`, a keypair used only for
deploys, for the VM; and `VM_KNOWN_HOSTS`,
which is optional, and without which the deploy warns and trusts whatever host
answers.

## 6. Licensing and attribution

The app shows each provider's notice in its own words
(`ts/client/src/ui/feature/setting/info/attribution.tsx`). Check the provider's source
before shortening, rewording or dropping one.

| Provider                    | License                                                          | Notice                                                                                                                                                                                                                                                                          |
| :-------------------------- | :--------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Overture Maps               | per theme: ODbL, CC BY 4.0, CDLA Permissive 2.0, Apache 2.0, CC0 | As Overture's [Explorer](https://github.com/OvertureMaps/explore-site) gives it: `© OpenStreetMap contributors, Overture Maps Foundation`, `modified`, and a link to the [attribution guide](https://docs.overturemaps.org/attribution/), which names each source per theme.    |
| OpenStreetMap Land Polygons | [ODbL](https://opendatacommons.org/licenses/odbl/)               | The [package](https://osmdata.openstreetmap.de/data/land-polygons.html)'s own line: `This data is Copyright 2026 OpenStreetMap contributors. It is available under the Open Database License (ODbL).` The year is the data's, and moves with `geo/coastline/source.json`'s pin. |
| GEDTM30                     | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)        | CC BY's elements, since it gives no line: `Modified GEDTM30 by Yufeng Ho and Tom Hengl (OpenGeoHub), licensed under CC BY 4.0`, linking v1.2's record ([10.5281/zenodo.18887460](https://doi.org/10.5281/zenodo.18887460)).                                                    |
| Arnis                       | [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0)        | Code, not data: the piers are ported from it ([THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)), so the app shows its NOTICE's `Copyright (c) 2022-2026 Louis Erbkamm (louis-e)`, as Apache 2.0 §4(d) asks.                                                                  |
