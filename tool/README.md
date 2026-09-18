# tool

What puts the elevation raster into db-geo. Run by hand a few times a year,
never by a server — which is why this is a root of its own rather than an
`apps/` member: nothing under it is deployed.

```
tool/
└── elevation/   GEDTM30 → db-geo (bash + GDAL)
```

**What was here is Rust now.** The asset tree's own tooling — the ingest that
hashes every file into the bucket's shape and writes the catalogs' tables, the
texture pack's normalizer, the brand's icon renderer — is `crates/pxl/asset-bake`:
`deno task ingest:asset`, `download:pack`, `render:brand`. What fills db-geo from
Overture, and the three page bakes, are `crates/pxl/geo-bake`'s
([the server in Rust](../issue/20260918-the-server-in-rust-and-the-line-already-drawn.md)).

What is left is bash and GDAL, which was never TypeScript and has no reason to
become Rust: the work is `gdalwarp`'s, and this is the command line that drives
it.

Each tool's flags are in its own header.

The procedures — what to bring up, what order, what to check, how to upload and
verify what is served — are not here:

- filling db-geo (the ingests): [infra/README.md](../infra/README.md) §2–4
- baking terrain, POI, geocode: [.agents/skills/bake-tiles](../.agents/skills/bake-tiles/SKILL.md)
- uploading, deploying, verifying, purging: [.agents/skills/publish-bucket](../.agents/skills/publish-bucket/SKILL.md)
- the monthly POI/geocode refresh, unattended: `.github/workflows/overture-monthly-refresh.yml`
- why it is shaped this way: [issue/20260816-deployment-hosting.md](../issue/20260816-deployment-hosting.md)
