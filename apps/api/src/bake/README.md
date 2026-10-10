# bake

Everything the client fetches that is not the app itself is produced here,
offline, and published to the bucket. Nothing under this directory runs in
the deployed API; the tile routes in `module/tile/` are the same code served
on demand for local development only.

```
bake/
├── overture/    ingests: Overture Buildings, Transportation, Base, Places, Divisions → db-geo
├── elevation/   ingest: GEDTM30 → db-geo (bash + GDAL)
├── terrain/     db-geo → out/terrain/<version>: one XTP page per land page, one _ocean
├── poi/         db-geo → out/poi/<version>: one XTP page per page with POI
├── geocode/     db-geo → out/geocode/<version>: one JSON page per page with a label
├── asset/       asset/ tree → out/asset/{glb,png,webp}/<sha256> and the shipped palette's table
├── bake_util.ts what every tool shares: gzip, the git sha and source catalogs a manifest records
└── wkb.ts       the WKB decoder the Overture and coast adapters read geometry through
```

Each tool's flags are in its own header; each `tool/` directory is the
command line, the rest of the directory is the pipeline behind it.

The procedures — what to bring up, what order, what to check, how to upload
and verify what is served — are not here:

- filling db-geo (the ingests): [infra/README.md](../../../../infra/README.md) §2–4
- baking terrain, POI, geocode: [.agents/skills/bake-tiles](../../../../.agents/skills/bake-tiles/SKILL.md)
- uploading, deploying, verifying, purging: [.agents/skills/publish-bucket](../../../../.agents/skills/publish-bucket/SKILL.md)
- the monthly POI/geocode refresh, unattended: `.github/workflows/overture-monthly-refresh.yml`
- why it is shaped this way: [issue/20260816-deployment-hosting.md](../../../../issue/20260816-deployment-hosting.md)
