# bake

What fills db-geo, and what turns the shipped asset tree into the objects the
bucket holds. Everything here is run by hand a few times, never by a server.

```
bake/
├── overture/    ingests: Overture Buildings, Transportation, Base, Places, Divisions → db-geo
├── elevation/   ingest: GEDTM30 → db-geo (bash + GDAL)
└── asset/       asset/ tree → out/asset/{glb,png,webp}/<sha256> and the table of their keys
```

**The three page bakes are not here any more.** terrain, POI and reverse
geocode are `crates/pxl/geo-bake` since 2026-09-18 — `deno task bake:tiles`,
`bake:poi`, `bake:geocode`
([the server in Rust](../../../../issue/20260918-the-server-in-rust-and-the-line-already-drawn.md)).
The ingests above did not move and are not going to: they run a handful of
times a year, their cost is Overture's download and Postgres's `COPY`, and
they lean on the `overturemaps` Python CLI and on `sharp`.

Each tool's flags are in its own header; each `tool/` directory is the command
line, the rest of the directory is the pipeline behind it.

The procedures — what to bring up, what order, what to check, how to upload and
verify what is served — are not here:

- filling db-geo (the ingests): [infra/README.md](../../../../infra/README.md) §2–4
- baking terrain, POI, geocode: [.agents/skills/bake-tiles](../../../../.agents/skills/bake-tiles/SKILL.md)
- uploading, deploying, verifying, purging: [.agents/skills/publish-bucket](../../../../.agents/skills/publish-bucket/SKILL.md)
- the monthly POI/geocode refresh, unattended: `.github/workflows/overture-monthly-refresh.yml`
- why it is shaped this way: [issue/20260816-deployment-hosting.md](../../../../issue/20260816-deployment-hosting.md)
