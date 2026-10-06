# Infrastructure

Two Postgres instances, the geographic data they hold, and how to bring both
up locally and in production.

```
infra/
├── local/               the two databases for development: one compose
├── prod/                the Hetzner stack: compose, Caddy, .env.example
└── geo/                 what fills db-geo: the Overture and elevation ingests
```

The ingests live here rather than with the bakes that read what they write:
they move a provider's columns into Postgres columns and share no model with
anything, so they are scripts — `geo/overture/script/`'s five cargo scripts
and `geo/elevation/script/`'s bash and GDAL pipeline.

## 1. Databases

| Service     | Port (ext:int) | Holds             | Image          |
| :---------- | :------------- | :---------------- | :------------- |
| **db-main** | 5432:5432      | nothing yet       | PostgreSQL 16  |
| **db-geo**  | 5433:5432      | every table in §2 | PostGIS 16-3.4 |

db-main holds nothing. pxl-geo takes `DB_GEO_URL` alone; pxl-api owns db-main,
connects to it and probes it at boot, and has no route that reads it yet
([the server in Rust](../decision/20260918-the-server-in-rust-and-the-line-already-drawn.md)).

- Local access: `localhost:5433`
- Container access: `db-geo:5432`

All spatial data is **EPSG:3857**. The renderer's voxel math assumes it.

DB passwords and `ADMIN_TOKEN` come from `.env` only. `DB_GEO_USER`,
`DB_GEO_PASSWORD` and the rest must be exported before running any script.
Production's `.env` uses its own generated values, never local dev's.

## 2. Filling db-geo

Seventeen tables from Overture, GEDTM30 and OSM, and the ingests that write them,
are [infra/geo/README.md](./geo/README.md) — the sources, what each pipeline
drops before it writes, how to run one and how to check it afterward.

## 3. Production (Hetzner Cloud)

Forward search is the only feature that needs a live server. Everything
else is baked to R2. One CX23 (2 vCPU / 4 GB / 40 GB) runs both servers, both
Postgres instances and Caddy in one compose stack — `./prod/`: `api` is pxl-geo
on `/geo/*`, `app` is pxl-api on `/api/*`, and Caddy is what sends each root to
its own container. Neither server has a published host port; only Caddy does.

Production's db-geo holds **only** `overture_places`, `overture_divisions`
and `overture_division_points`, and their rows of `overture_catalog`.
Terrain's ingestion tables and the elevation raster are several GB, baked
offline, and never queried here.
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
   take the whole repo as their build context: the servers are `crates/pxl/server/geo`
   and `crates/pxl/server/api`, and their dependencies are path dependencies in the
   same Cargo workspace, so `crates/` has to be present whole for Cargo to
   resolve it. Each sits beside its own crate: `crates/pxl/server/geo/Dockerfile` and
   `crates/pxl/server/api/Dockerfile`.

6. **The production `.env`**
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

Production's db-geo is a copy, and a deploy does not carry it. `deploy-server`
rebuilds the servers from the commit (§5); the three tables stay as they were
last copied. So a re-ingest reaches production only by this copy, and an
ingest that changes a table's shape must be copied **before** the deploy that
ships a server reading the new shape — the order §5 keeps for the servers and
the client, one step further back. On 2026-10-04 the servers went out reading
`properties` from tables still in the old per-type shape, and search answered
500 until the copy below was made.

The dump comes from the **local db-geo container's** own `pg_dump`: a
host-installed one several major versions ahead writes a custom-format dump
that production's `postgis/postgis:16-3.4` cannot restore. It takes the
catalog too, so production keeps the release and the time each table was
ingested (infra/geo/README.md §4):

```sh
set -a; . ./.env; set +a
docker exec pixelet-db-geo pg_dump --format=custom --no-owner \
  -U "$DB_GEO_USER" -d "$DB_GEO_NAME" \
  -t overture_places -t overture_divisions -t overture_division_points \
  -t overture_catalog > overture.dump

scp overture.dump root@$(hcloud server ip pixelet-prod):
ssh root@$(hcloud server ip pixelet-prod)
```

Then, in the VM's shell rather than piped into `ssh`, since the first
`docker exec -i` would read the rest of a piped script as its input:

```sh
sql() { docker exec -i pixelet-db-geo sh -c 'exec "$@" -U "$POSTGRES_USER" -d "$POSTGRES_DB"' sh "$@"; }

# a table-only dump carries no CREATE EXTENSION
sql psql -v ON_ERROR_STOP=1 -c 'CREATE EXTENSION IF NOT EXISTS postgis; CREATE EXTENSION IF NOT EXISTS pg_trgm;'
# one transaction: the old tables stay unless every new one is in
sql pg_restore --no-owner --no-privileges --clean --if-exists --single-transaction < ~/overture.dump
# the catalog keeps the rows of the tables that came with it, and the planner its statistics
sql psql -v ON_ERROR_STOP=1 -c "DELETE FROM overture_catalog WHERE file_name NOT IN ('overture_places', 'overture_divisions', 'overture_division_points'); ANALYZE overture_places; ANALYZE overture_divisions; ANALYZE overture_division_points;"
rm ~/overture.dump
```

Without `--single-transaction` pg_restore goes on past an error and can leave
the tables half replaced; with it, a failure leaves them as they were, and the
whole thing can be run again. Restore writes no planner statistics, hence the
`ANALYZE` ([pg_restore](https://www.postgresql.org/docs/16/app-pgrestore.html)).
`deno task smoke https://api.plei.me --origin` checks the result.

### Updating

```sh
git pull
docker compose up -d --build
```

## 4. Cloudflare (the client, the tiles, and the relays)

### Wiring

Four roots, decided in `spec/pxl/spec/route.json` and written from there into every
place that builds a URL or reads one:

| Root      | What it is                                           | Who answers it |
| :-------- | :--------------------------------------------------- | :------------- |
| `/geo/`   | the world's data                                     | pxl-geo        |
| `/api/`   | the application                                      | pxl-api        |
| `/tile/`  | baked tiles, by kind, version and quadkey            | R2             |
| `/asset/` | models and textures, by path with a hash in the stem | R2             |

The Worker claims all four (`wrangler.jsonc`'s `run_worker_first`) and splits
them: `/tile/*` and `/asset/*` come straight out of the bucket, and `/geo/*`
and `/api/*` are reverse-proxied to `GEO_ORIGIN` and
`API_ORIGIN` with the path unchanged. An asset's key _is_ its path without the
leading slash — the root and the bucket's own prefix are the same word — where
a tile's key is its path with the tile root removed. Caddy on the VM then sends each root
to the container that answers it. Nothing is rewritten in transit: client,
relay and origin use identical paths.

No relay reads past a root or its one branch, so a new live route is a change
to the one server that answers it. Every route must be under a claimed root —
a bare path is unreachable in production, where the static-asset fallback
answers it with `index.html`, which reads as the app loading rather than as a
route that is not there. A root is reserved on the server's side because the
app routes on paths of its own; giving the app the prefix instead would put one
in every URL a user sees.

`deno task dev:local` runs one process per root plus the tiles: pxl-geo on 3000,
serve_tiles on 3001, pxl-api on 3002, and the dev proxy splits on the same
prefixes the Worker does. A deployment has no tile server at all, because the
bucket answers those two prefixes.

### Deploying the client

**A green push to `deploy` publishes it** — see §5. What follows is the same thing
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

Afterward, every path the app takes, plus the gates and the addresses that
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

## 5. CI/CD

`.github/workflows/check-and-deploy.yml` runs on every push to `deploy` and
every pull request, and not on main: main is pushed to carry work between
machines, compiling or not, and a push there is not a request to build it.
Publishing is `git push origin main:deploy`, with `--force` once main has been
rewritten; a check without a deploy is `gh workflow run check-and-deploy.yml
--ref main`. One workflow, five jobs, because a deploy must not be able to
start against a commit the gate never saw:

| Job             | When                                                                   | What                                                                                                                    |
| :-------------- | :--------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------- |
| `gate`          | always; takes a build number on deploy                                 | `deno task check`, `test`, `lint`, then `./script/build-counter.ts`                                                     |
| `deploy-web`    | deploy, gate green, and `apps/`/`crates/`/`packages/`/`spec/` changed  | `deno task deploy` under that number                                                                                    |
| `deploy-server` | deploy, gate green, and `crates/`/`infra/prod/`/the Dockerfile changed | ssh, `git reset --hard` to the run's own commit, `docker compose up -d --build --force-recreate`, then the origin smoke |
| `smoke`         | after both, if either of them deployed                                 | `deno task smoke https://plei.me`                                                                                       |
| `record`        | after both, if either of them deployed                                 | `./script/build-log.ts --record`, appending what this build carried                                                     |

**The servers go first, then the client.** A new client against an old server
is the direction that breaks; an old client against a new one is not, so long
as the server still answers the old shape for a release — which is the
discipline that makes a deploy survivable at all, since nobody's open tab
reloads on command and a session can outlive several builds. The b15 deploy
followed that order by hand; `deploy-web` now needs `deploy-server`, so a run
cannot get it wrong and a failed server deploy keeps the client back with it.

The public smoke is still a job after both rather than a step inside either:
`/geo/search` is the one route the Worker proxies to the VM, and asked from
`deploy-web` it could land while the stack was still recreating — Cloudflare
answers 521 for an origin that is merely restarting, and the run would fail
though nothing was wrong.

Both deploys are filtered on what changed since that half last went out, for
different reasons.
Reaching into the production VM over SSH is neither idempotent nor reversible,
so it runs only when an image's own tree changed. Publishing to the edge is
both — but the build number is an input to the bundle, so a docs-only push
would otherwise ship bytes identical except for a number, spend a number on
them, and make every edge cache fetch the lot again.

Both filters are whitelists rather than lists of what to skip: the set that
ships nothing is the one that keeps growing, and a root forgotten from a skip
list is a deploy nobody asked for.

What "changed" is measured against is the commit that half last went out
from, as the build log records it (`./script/build-log.ts --last web` or
`--last server`), not the push before: a run that failed published nothing,
and measured against its push the next one would see only its own change and
skip what the failed run never shipped. That happened on 2026-10-04, when
three failed runs left a fourth to see a workflow file alone. Where there is
no base to be had — an empty log, a commit a rewrite of main took, a log that
cannot be read — both deploy: a republish costs a cached build, and a skip
that should not have happened leaves production behind `deploy`.

Both halves of a build are the same commit. Every checkout pins
`ref: github.sha`, and the VM resets to that commit rather than to whatever
`origin/deploy` has reached by the time the ssh lands — otherwise a push during a
run, or a rewritten history, would have the edge serving one commit and the VM
running another under one build number. A rewrite can leave that commit
unreachable on the VM; rather than refuse the deploy and leave production on
the old code, it warns in the run and takes the tip.

`--force-recreate` is not decoration: the Caddyfile is a bind mount and a bind
mount is read at start, so an `up --build` alone leaves a stale relay in front
of a fresh server. On 2026-09-18 that looked like `/api/*` answering 404 from
the wrong process rather than like a failure.

`gate` and `deploy-web` share `concurrency: build-counter` across every ref,
because the count is one object in the bucket and the take is read-add-write:
two runs holding it together would take the same number. A deploy push therefore
queues behind the one in front of it. A hand `deno task deploy` is outside
the group and takes no number at all — it publishes an unnumbered build, which
is the fallback rather than a second deployer.

**Every build of `deploy` is numbered, and only some are published.** That is a
build lab's count: the gate takes a number once it is green, `deploy-web`
publishes under it when the client's own tree changed, and a number nobody
published is simply never seen. The counter is never reset — not from git,
where a squash or a fresh clone would renumber it, and not from the workflow's
own run number, which restarts at 1 if the workflow is recreated.

The counter says how many builds there have been and nothing about what they
were, so `record` keeps the other half: `_meta/build-log.jsonl`, a line per
published build with its number, what it was built from, which halves it
reached, and **the subjects of the commits it carried**.

The subjects rather than a tag or a stored id, because main's history is
rewritten here as a matter of course. A tag, a sha and `BUILD_ID`'s own
`r<count>-<hash>` all name something that may not exist by the time anyone
looks; text written while the history is still that history survives every
rewrite after it. The sha is kept beside the subjects anyway — it resolves
right up until it does not.

```sh
./script/build-log.ts        # every build, newest first, each with what it carried
```

The read also writes the last number into `apps/web/.build_number`, this
checkout's cache of it, so a dev build says `b<n>+` against the build that
actually went out last; `deno task deploy` writes the same file.

R2 has no append, so the write is read-add-write as the counter's is, and the
job's own `build-log` concurrency group is what keeps two of them from losing a
line. A missing log object is refused rather than taken for an empty one: the
two look alike from outside and the wrong guess would write one line over every
line there was. Starting it is a one-off, and the script prints the command.

`record` is not gated on the smoke — a build that answered badly still went
out, and what is recorded is what is live rather than what is well. A number
with no line was counted and never published. A failed write leaves a red job
after two deploys that already happened: it reports a missing record and takes
nothing back.

### Repository secrets

All of them set in GitHub's own settings and nowhere else:

| Name                    | For                                                                                                                             |
| :---------------------- | :------------------------------------------------------------------------------------------------------------------------------ |
| `CLOUDFLARE_API_TOKEN`  | wrangler: publish the Worker, read/write the build counter in R2                                                                |
| `CLOUDFLARE_ACCOUNT_ID` | which account, since a token may reach more than one                                                                            |
| `VM_HOST` / `VM_USER`   | the Hetzner box                                                                                                                 |
| `VM_SSH_KEY`            | a deploy-only private key; its public half in that user's `authorized_keys`                                                     |
| `VM_KNOWN_HOSTS`        | `ssh-keyscan <host>` run on the VM. Optional, and the deploy warns without it: the fallback accepts whatever answers on the day |

The VM key should be its own keypair rather than a personal one, so revoking
it costs nothing else.

## 6. Licensing and attribution

The notices below are required, shown in
`apps/web/src/feature/setting/section/attribution.tsx`. Check the
source license before shortening, rewording or dropping any of them.

| Source              | License                                                                                                  | Notice                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| :------------------ | :------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenStreetMap       | [ODbL](https://opendatacommons.org/licenses/odbl/)                                                       | `© OpenStreetMap contributors, available under the Open Database License` — the [OSMF Attribution Guidelines](https://osmfoundation.org/wiki/License/Attribution_Guidelines) require the credit to make clear the data is under ODbL, so the license is named rather than left to the link alone. Overture's Buildings/Transportation/Base/Divisions are substantially OSM-sourced and carry this obligation through.                                                                                                                                                                                    |
| GEDTM30             | [CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/)                                                | Creator, title, license link and an indication of change — the pipeline crops and reprojects, so it changed: `Elevation: modified GEDTM30 by OpenGeoHub, licensed under CC BY 4.0.` The author list and journal reference that used to sit here are an academic courtesy, not part of what CC BY asks, and were long enough to crowd out the Copernicus row below, which is not optional. Link the [DOI this pipeline consumes](https://doi.org/10.5281/zenodo.15689805) (`tool/elevation/metadata.json`), which is what the app's own notice and README link too — one source, not three mirrors of it. |
| Copernicus WorldDEM | [Copernicus DEM licence](https://spacedata.copernicus.eu/collections/copernicus-digital-elevation-model) | Compulsory, and the wording is dictated. GEDTM30 is a fusion whose inputs include Copernicus WorldDEM, and the license binds anyone communicating adapted data to its "produced using" form, verbatim: `produced using Copernicus WorldDEM © DLR e.V. 2010-2014 and © Airbus Defense and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved`.                                                                                                                                                                                                             |

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
