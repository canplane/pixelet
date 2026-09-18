# tool

What fills db-geo, and what turns the shipped asset tree into the objects the
bucket holds. Everything here is run by hand a few times a year, never by a
server — which is why it is a root of its own rather than an `apps/` member:
nothing under it is deployed.

```
tool/
├── ingest/overture/   Overture Buildings, Transportation, Base, Places, Divisions → db-geo
├── ingest/asset/      asset/ tree → out/asset/{glb,png,webp}/<sha256> and the table of their keys
├── elevation/         GEDTM30 → db-geo (bash + GDAL)
└── lib/env/           reading and validating an environment variable
```

**The three page bakes are not here.** terrain, POI and reverse geocode are
`crates/pxl/geo-bake` — `deno task bake:tiles`, `bake:poi`, `bake:geocode`
([the server in Rust](../issue/20260918-the-server-in-rust-and-the-line-already-drawn.md)).
The ingests stayed in Deno and are not going to move: their cost is Overture's
download and Postgres's `COPY`, not the language, and they lean on the
`overturemaps` Python CLI and on `sharp`.

Each tool's flags are in its own header.

The procedures — what to bring up, what order, what to check, how to upload and
verify what is served — are not here:

- filling db-geo (the ingests): [infra/README.md](../infra/README.md) §2–4
- baking terrain, POI, geocode: [.agents/skills/bake-tiles](../.agents/skills/bake-tiles/SKILL.md)
- uploading, deploying, verifying, purging: [.agents/skills/publish-bucket](../.agents/skills/publish-bucket/SKILL.md)
- the monthly POI/geocode refresh, unattended: `.github/workflows/overture-monthly-refresh.yml`
- why it is shaped this way: [issue/20260816-deployment-hosting.md](../issue/20260816-deployment-hosting.md)
