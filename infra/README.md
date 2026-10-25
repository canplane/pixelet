# Infrastructure

Almost everything a player sees is baked ahead and served from a bucket;
only search and the application need a live server. So production is one
Cloudflare Worker in front of one small VM:

```
                ┌─ the app ────────────────────────────▶ its own static assets
client ─ Worker ┼─ /tile/*, /asset/* ──────────────────▶ R2 bucket
                └─ /geo/*, /api/* ── Caddy on the VM ─┬▶ pxl-geo ── db-geo
                                                      └▶ pxl-api ── db-main
```

Locally the same two databases run from one compose file, and the tile
server stands in for the bucket.

```
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

pxl-api owns db-main and probes it at boot, but no route reads it yet
([the server in Rust](../doc/minutes/20260918-the-server-in-rust-and-the-line-already-drawn.md));
pxl-geo reads db-geo through `DB_GEO_URL`. All spatial data is EPSG:3857.
Passwords and `ADMIN_TOKEN` come from `.env` only, and production's are its
own.

**Never `down -v` db-geo** (`pixelet-db-geo`): its volume is a
multi-gigabyte PostGIS dataset, hours of ingest to fill again.

## 2. Filling db-geo

[geo/README.md](./geo/README.md) is the sources, the tables and how to run
each ingest. One rule holds for all of them: **a table is filled beside
itself and swapped in, never emptied first.** The new one is built as
`<table>_next` while readers keep the old, then one short transaction, its
`lock_timeout` bounded, drops the old and renames the new. Nothing is dropped
or truncated by hand, and a fill needs room for both tables at once.

## 3. Production (Hetzner Cloud)

One CX23 (2 vCPU, 4 GB, 40 GB) runs both servers, both databases and Caddy
in one compose stack (`prod/`). Caddy is the only published port and sends
`/geo/*` to `api` (pxl-geo) and `/api/*` to `app` (pxl-api). Production's
db-geo holds only what search reads: `overture_places`,
`overture_division_areas` and `overture_divisions`.

### Setup, once

1. **VM.**
   ```sh
   hcloud server create --name pixelet-prod --type cx23 --image ubuntu-24.04 \
     --location fsn1 --ssh-key <your-key-name>
   ```
2. **Firewall.** 80 and 443 open to Cloudflare's IP ranges alone
   (`cloudflare.com/ips-v4`, `-v6`), so the origin cannot be reached around
   the edge; SSH is key-only.
   ```sh
   hcloud firewall create --name pixelet-prod-fw
   hcloud firewall replace-rules pixelet-prod-fw --rules-file fw_rules.json
   hcloud firewall apply-to-resource pixelet-prod-fw --type server --server pixelet-prod
   ```
3. **Docker.** `curl -fsSL https://get.docker.com | sh`.
4. **DNS.** An A record for `api.plei.me` to the VM, **proxied** through
   Cloudflare; DNS-only would bypass the edge the firewall relies on.
5. **The tree.** Cloned whole onto the VM: both servers' Dockerfiles
   (`rust/server/geo/Dockerfile`, `rust/server/api/Dockerfile`) build
   from the repository root, since their dependencies are path dependencies
   across `rust/`.
6. **Up.**
   ```sh
   cd infra/prod
   cp .env.example .env     # fill in, then chmod 600
   # the disk's bounds: container logs, BuildKit's cache by last use, the
   # journal by size and a day; a server deploy reinstalls them when changed
   install -m 644 etc/daemon.json /etc/docker/daemon.json && systemctl restart docker
   install -D -m 644 etc/journald.conf /etc/systemd/journald.conf.d/pixelet.conf && systemctl restart systemd-journald
   docker compose up -d --build
   ```

   CI's deploy user is root, which installing them needs.

A CI deploy updates the stack after that (§5) with `infra/prod/deploy.sh`,
sent from a checkout and run on the VM: `git reset --hard` to the commit,
install a changed `infra/prod/etc/` file and restart what reads it, then in
`infra/prod` `docker compose up -d --build --force-recreate` and
`docker image prune -f`. By hand it is the same script, for a commit `deploy`
already holds, since the VM fetches it from there:

```sh
ssh root@$(hcloud server ip pixelet-prod) bash -eu -s -- "$(git rev-parse origin/deploy)" < infra/prod/deploy.sh
```

### Copying the search tables

A deploy does not carry db-geo: production's three tables change only when
they are copied over. When the copy changes a table's shape, the server that
reads it goes out with it, as one step: stop `api`
(`docker compose stop api`), restore, then deploy the server.

Dump with the local container's own `pg_dump`, since a newer host one writes
a format production's Postgres 16 cannot restore:

```sh
set -a; . ./.env; set +a
docker exec pixelet-db-geo pg_dump --format=custom --no-owner \
  -U "$DB_GEO_USER" -d "$DB_GEO_NAME" \
  -t overture_places -t overture_division_areas -t overture_divisions > overture.dump

scp overture.dump root@$(hcloud server ip pixelet-prod):
ssh root@$(hcloud server ip pixelet-prod)
```

Then in the VM's own shell — piped into `ssh`, the first `docker exec -i`
would swallow the rest:

```sh
sql() { docker exec -i pixelet-db-geo sh -c 'exec "$@" -U "$POSTGRES_USER" -d "$POSTGRES_DB"' sh "$@"; }

# a table-only dump carries no CREATE EXTENSION
sql psql -v ON_ERROR_STOP=1 -c 'CREATE EXTENSION IF NOT EXISTS postgis; CREATE EXTENSION IF NOT EXISTS pg_trgm;'
# one transaction: a failure leaves the old tables as they were
sql pg_restore --no-owner --no-privileges --clean --if-exists --single-transaction < ~/overture.dump
# the planner's statistics, which restore does not write
sql psql -v ON_ERROR_STOP=1 -c "ANALYZE overture_places; ANALYZE overture_division_areas; ANALYZE overture_divisions;"
rm ~/overture.dump
```

`deno task smoke https://api.plei.me --origin` checks the result.

## 4. Cloudflare

Four roots, written once in `spec/spec/route.json` and read from there by
everything that builds or answers a URL:

| Root      | What it is                                           | Answered by |
| :-------- | :--------------------------------------------------- | :---------- |
| `/geo/`   | the world's data                                     | pxl-geo     |
| `/api/`   | the application                                      | pxl-api     |
| `/tile/`  | baked tiles, by kind, version and name               | R2          |
| `/asset/` | models and textures, by path with a hash in the stem | R2          |

**The bucket is private and the Worker is the only way into it**, on the
free plan, which fails closed at 100k requests a day, so no read can bill.
A paid Workers plan, the `r2.dev` URL or a custom domain on the bucket breaks
that ([deployment hosting](../doc/minutes/20260816-deployment-hosting.md)).
Writes are outside it, and nothing configures a spending cap: a publish is a
pyramid's worth of Class A operations, so `deno task usage` comes before one.

The bucket, `pixelet-tiles`, holds everything the client fetches that is not the app itself:

| Prefix                  | Object                                          | Named by                 | Policy                                                           |
| ----------------------- | ----------------------------------------------- | ------------------------ | ---------------------------------------------------------------- |
| `terrain/<version>/`    | packed XTP tile                                 | name, 4–15               | immutable; a re-bake is a new version                            |
| `place/<version>/`      | XTP count tile                                  | name, 7–15               | immutable; a re-bake is a new version                            |
| `division/<version>/`   | XTP division tile                               | name, 4–12               | immutable; a re-bake is a new version                            |
| `earth/<version>/`      | packed earth tile; `sphere`, its picture whole  | name, 4                  | immutable; a re-bake is a new version                            |
| `asset/`                | glb, png, webp — not packed                     | the SHA-256 of its bytes | immutable by construction; unreferenced keys deleted whenever    |
| `source`                | the source's declaration, JSON                  | fixed name               | mutable; written by `publish`, `no-cache`, revalidated each boot |
| `_meta/build-counter`   | the official build count                        | fixed name               | mutable; taken and written by CI's number job, never by hand     |
| `_meta/build-log.jsonl` | a line a published build                        | fixed name               | appended by CI's `record` (`script/build-log.ts`)                |
| `_meta/paths/<n>.json`  | build `#n`'s source paths, to decode its panics | build number             | written by `deno task deploy` under a build number; never served |

A tile's name is its Morton key, so the same name under two kinds is the same ground, and one prefix is one region across every kind at every zoom. Every object is uploaded `public, max-age=31536000, immutable`, so a new version is a new prefix and no cache needs purging.

The Worker claims all four (`wrangler.jsonc` `run_worker_first`): the bucket
answers `/tile/*`, under its path less the root, and `/asset/*`, under its
path; `/geo/*` and `/api/*` go to `GEO_ORIGIN` and `API_ORIGIN` unchanged.
No path is rewritten anywhere. A route outside a root is unreachable in
production — the app's fallback answers it with `index.html`, which looks
like the app loading, not like a missing route. Locally `deno task dev:oven`
runs pxl-geo on 3000, the tile server on 3001 and pxl-api on 3002, and the
dev proxy splits on the same roots.

**Deploying the client** is CI's (§5); by hand it is the script CI runs.
Neither checks or tests anything, so first `deno task check` and
`deno task test` pass over the commit
([the deploy skill](../.agents/skills/deploy/SKILL.md)), and the servers it
talks to are already deployed (§3), since a new client against an old server
is the direction that breaks. A hand deploy takes no number and goes out
unnumbered.

```sh
wrangler login           # once a machine
deno task check && deno task test
deno task deploy         # build, publish
deno task smoke https://plei.me
```

A deploy uploads only
changed static assets and leaves the bucket alone: uploading tiles and
assets is [the publish-bucket skill](../.agents/skills/publish-bucket/SKILL.md).

The smoke does not check a tile's or an asset's served bytes; that is the
skill's "Verify what is served".

## 5. CI/CD

`.github/workflows/deploy.yml` runs on a push to `deploy` and on a dispatch,
never on main: main carries work between machines, compiling or not. A push
that changes only markdown runs nothing, and a dispatch against any other ref
does nothing. **Publishing is `git push origin main:deploy`**, forced once
main has been rewritten, after `deno task check` and `deno task test` pass
here: CI checks and tests nothing
([the deploy skill](../.agents/skills/deploy/SKILL.md)). Why each job is as
it is lives in the workflow's comments.

| Job             | When                  | What                                                                                                    |
| :-------------- | :-------------------- | :------------------------------------------------------------------------------------------------------ |
| `number`        | every push to deploy  | takes a build number                                                                                    |
| `deploy-server` | after `number`        | ssh, `infra/prod/deploy.sh`: reset to the run's commit, `docker compose up -d --build --force-recreate` |
| `deploy-web`    | after `deploy-server` | `deno task deploy` under the build number: builds, keeps the panic paths, publishes                     |
| `smoke`         | after both deployed   | `deno task smoke https://plei.me`                                                                       |
| `record`        | after either deployed | appends the build to the log                                                                            |

A deploy is whole: the client and the servers go out from the one commit,
every time, so they never run different ones. The servers go first because an
old client against a new server is safe and the reverse is not.

Every build of `deploy` takes a number from a counter in
the bucket, never reset; a published one is a line in `_meta/build-log.jsonl`
with the subjects of the commits it carried, since main's shas do not
survive its rewrites. `./script/build-log.ts` reads it, newest first, and
writes the last number into `ts/client/.build_number` for dev builds.

The secrets are GitHub's alone: `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID` for wrangler and the counter; `VM_HOST`, `VM_USER`
and `VM_SSH_KEY`, a deploy-only keypair, for the VM; and `VM_KNOWN_HOSTS`,
optional, without which the deploy warns and trusts whatever answers.

## 6. Licensing and attribution

The app shows each provider's notice in its own words
(`ts/client/src/shell/setting/section/attribution.tsx`). Check the source
before shortening, rewording or dropping one.

| Provider                    | License                                                          | Notice                                                                                                                                                                                                                                                                          |
| :-------------------------- | :--------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Overture Maps               | per theme: ODbL, CC BY 4.0, CDLA Permissive 2.0, Apache 2.0, CC0 | As Overture's [Explorer](https://github.com/OvertureMaps/explore-site) gives it: `© OpenStreetMap contributors, Overture Maps Foundation`, `modified`, and a link to the [attribution guide](https://docs.overturemaps.org/attribution/), which names each source per theme.    |
| OpenStreetMap Land Polygons | [ODbL](https://opendatacommons.org/licenses/odbl/)               | The [package](https://osmdata.openstreetmap.de/data/land-polygons.html)'s own line: `This data is Copyright 2026 OpenStreetMap contributors. It is available under the Open Database License (ODbL).` The year is the data's, and moves with `geo/coastline/source.json`'s pin. |
| GEDTM30                     | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)        | CC BY's elements, as it gives no line: `Modified GEDTM30 by Yufeng Ho and Tom Hengl (OpenGeoHub), licensed under CC BY 4.0`, linking v1.2's record ([10.5281/zenodo.18887460](https://doi.org/10.5281/zenodo.18887460)).                                                        |
| Arnis                       | [Apache 2.0](https://www.apache.org/licenses/LICENSE-2.0)        | Code, not data: the piers are ported from it ([THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md)), so the app shows its NOTICE's `Copyright (c) 2022-2026 Louis Erbkamm (louis-e)`, as Apache 2.0 §4(d) asks.                                                                  |

Whether a published version, fetchable tile by tile, needs a notice of its
own beyond the app's is open (doc/backlog.md).
